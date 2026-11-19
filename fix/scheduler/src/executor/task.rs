use alloc::{boxed::Box, sync::Arc, task::Wake};
use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll, Waker},
};

struct Ready {
    runnable: AtomicBool,
    parent: Option<Waker>,
}
impl Wake for Ready {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        if !self.runnable.swap(true, Ordering::AcqRel)
            && let Some(parent) = &self.parent
        {
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
        let ready = Arc::new(Ready {
            runnable: AtomicBool::new(true),
            parent,
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
    let mut pending: alloc::vec::Vec<_> = futures
        .into_iter()
        .map(|future| Some(Task::with_parent(future, Some(parent.clone()))))
        .collect();
    let mut results: alloc::vec::Vec<Option<T>> = (0..pending.len()).map(|_| None).collect();
    core::future::poll_fn(|_| {
        let mut waiting = false;
        for (future, result) in pending.iter_mut().zip(&mut results) {
            let Some(active) = future else {
                continue;
            };
            match active.poll() {
                Poll::Ready(Ok(value)) => {
                    *result = Some(value);
                    *future = None;
                }
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => waiting = true,
            }
        }
        if waiting {
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

/// A worker-local set of independent tasks. Dropping the set drops its futures.
pub struct Tasks<'a, T> {
    tasks: alloc::vec::Vec<Task<Pin<Box<dyn Future<Output = T> + 'a>>>>,
    parent: Option<Waker>,
}
impl<T> Default for Tasks<'_, T> {
    fn default() -> Self {
        Self {
            tasks: alloc::vec::Vec::new(),
            parent: None,
        }
    }
}
impl<'a, T> Tasks<'a, T> {
    pub fn spawn(&mut self, future: impl Future<Output = T> + 'a) {
        self.tasks
            .push(Task::with_parent(Box::pin(future), self.parent.clone()));
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
        let mut index = 0;
        while index < self.tasks.len() {
            progress |= self.tasks[index].is_ready();
            if let Poll::Ready(value) = self.tasks[index].poll() {
                completed.push(value);
                drop(self.tasks.swap_remove(index));
            } else {
                index += 1;
            }
        }
        (progress, completed)
    }
    pub fn is_ready(&self) -> bool {
        self.tasks.iter().any(Task::is_ready)
    }
}
