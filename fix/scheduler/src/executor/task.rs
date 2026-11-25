use alloc::{
    boxed::Box,
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    task::Wake,
};
use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll, Waker},
};
use spin::Mutex;

#[derive(Default)]
struct ReadyQueue {
    indices: Mutex<VecDeque<usize>>,
}

impl ReadyQueue {
    fn ready(&self) -> usize {
        self.indices.lock().len()
    }

    fn push(&self, index: usize) {
        self.indices.lock().push_back(index);
    }

    fn pop(&self) -> Option<usize> {
        self.indices.lock().pop_front()
    }
}

struct Ready {
    runnable: AtomicBool,
    parent: Option<Waker>,
    queue: Option<(Arc<ReadyQueue>, usize)>,
}
impl Wake for Ready {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        if self.runnable.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some((queue, index)) = &self.queue {
            queue.push(*index);
        }
        if let Some(parent) = &self.parent {
            parent.wake_by_ref();
        }
    }
}

/// A pinned local future with readiness tracking for the event loop.
pub struct Task<F> {
    future: Pin<Box<F>>,
    ready: Arc<Ready>,
    waker: Waker,
}
impl<F: Future> Task<F> {
    pub fn new(future: F) -> Self {
        Self::with_parent(future, None)
    }

    pub(crate) fn with_parent(future: F, parent: Option<Waker>) -> Self {
        Self::with_readiness(future, parent, None)
    }

    fn with_queue(future: F, parent: Option<Waker>, queue: &Arc<ReadyQueue>, index: usize) -> Self {
        queue.push(index);
        Self::with_readiness(future, parent, Some((queue.clone(), index)))
    }

    fn with_readiness(
        future: F,
        parent: Option<Waker>,
        queue: Option<(Arc<ReadyQueue>, usize)>,
    ) -> Self {
        let ready = Arc::new(Ready {
            runnable: AtomicBool::new(true),
            parent,
            queue,
        });
        Self {
            future: Box::pin(future),
            waker: Waker::from(ready.clone()),
            ready,
        }
    }

    /// Poll once. A wake during polling keeps the event loop runnable.
    pub fn poll(&mut self) -> Poll<F::Output> {
        if !self.ready.runnable.swap(false, Ordering::AcqRel) {
            return Poll::Pending;
        }
        self.future
            .as_mut()
            .poll(&mut Context::from_waker(&self.waker))
    }

    /// Check for runnable work before putting the worker to sleep.
    pub fn is_ready(&self) -> bool {
        self.ready.runnable.load(Ordering::Acquire)
    }
}

/// Start all supplied futures, preserving input order in the results.
/// On an error, drop remaining futures; this cancels waiters, not shared evaluations.
pub async fn try_join_all<F, T, E>(
    futures: impl IntoIterator<Item = F>,
) -> Result<alloc::vec::Vec<T>, E>
where
    F: Future<Output = Result<T, E>>,
{
    let parent = core::future::poll_fn(|cx| Poll::Ready(cx.waker().clone())).await;
    let queue = Arc::new(ReadyQueue::default());
    let mut pending: alloc::vec::Vec<_> = futures
        .into_iter()
        .enumerate()
        .map(|(index, future)| {
            Some(Task::with_queue(
                future,
                Some(parent.clone()),
                &queue,
                index,
            ))
        })
        .collect();
    let mut remaining = pending.len();
    let mut results: alloc::vec::Vec<Option<T>> = (0..pending.len()).map(|_| None).collect();
    core::future::poll_fn(|cx| {
        for _ in 0..queue.ready() {
            let index = queue.pop().unwrap();
            let Some(active) = &mut pending[index] else {
                continue;
            };
            match active.poll() {
                Poll::Ready(Ok(value)) => {
                    results[index] = Some(value);
                    pending[index] = None;
                    remaining -= 1;
                }
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => {}
            }
        }
        if remaining != 0 {
            if queue.ready() != 0 {
                cx.waker().wake_by_ref();
            }
            Poll::Pending
        } else {
            Poll::Ready(Ok(results
                .iter_mut()
                .map(|result| result.take().unwrap())
                .collect()))
        }
    })
    .await
}

type BoxedTask<'a, T> = Task<Pin<Box<dyn Future<Output = T> + 'a>>>;

/// A worker-local set of independent tasks. Dropping the set drops its futures.
pub struct Tasks<'a, T> {
    tasks: BTreeMap<usize, BoxedTask<'a, T>>,
    queue: Arc<ReadyQueue>,
    next_index: usize,
    parent: Option<Waker>,
}
impl<T> Default for Tasks<'_, T> {
    fn default() -> Self {
        Self {
            tasks: BTreeMap::new(),
            queue: Arc::new(ReadyQueue::default()),
            next_index: 0,
            parent: None,
        }
    }
}
impl<'a, T> Tasks<'a, T> {
    pub fn spawn(&mut self, future: impl Future<Output = T> + 'a) {
        let index = self.next_index;
        self.next_index = index.checked_add(1).expect("task indices exhausted");
        let future: Pin<Box<dyn Future<Output = T> + 'a>> = Box::pin(future);
        self.tasks.insert(
            index,
            Task::with_queue(future, self.parent.clone(), &self.queue, index),
        );
    }
    pub(crate) fn with_parent(parent: Waker) -> Self {
        Self {
            parent: Some(parent),
            ..Self::default()
        }
    }
    /// Poll tasks once, reporting readiness and collecting completions.
    pub fn poll(&mut self) -> (bool, alloc::vec::Vec<T>) {
        let mut progress = false;
        let mut completed = alloc::vec::Vec::new();
        for _ in 0..self.queue.ready() {
            let index = self.queue.pop().unwrap();
            let Some(task) = self.tasks.get_mut(&index) else {
                continue;
            };
            progress = true;
            if let Poll::Ready(value) = task.poll() {
                completed.push(value);
                self.tasks.remove(&index);
            }
        }
        (progress, completed)
    }
    pub fn is_ready(&self) -> bool {
        self.queue.ready() != 0
    }
}
