use alloc::{boxed::Box, sync::Arc, task::Wake};
use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll, Waker},
};

struct Ready(AtomicBool);
impl Wake for Ready {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.store(true, Ordering::Release);
    }
}

/// A pinned local future, polled only initially or after its waker fires.
pub struct Task<F> {
    future: Pin<Box<F>>,
    ready: Arc<Ready>,
    waker: Waker,
}
impl<F: Future> Task<F> {
    pub fn new(future: F) -> Self {
        let ready = Arc::new(Ready(AtomicBool::new(true)));
        Self {
            future: Box::pin(future),
            waker: Waker::from(ready.clone()),
            ready,
        }
    }

    /// Poll a ready task once. A wake during polling keeps it ready for the next turn.
    pub fn poll(&mut self) -> Poll<F::Output> {
        if !self.ready.0.swap(false, Ordering::AcqRel) {
            return Poll::Pending;
        }
        self.future
            .as_mut()
            .poll(&mut Context::from_waker(&self.waker))
    }

    /// Check for runnable work before putting the worker to sleep.
    pub fn is_ready(&self) -> bool {
        self.ready.0.load(Ordering::Acquire)
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
    let mut pending: alloc::vec::Vec<_> = futures
        .into_iter()
        .map(|future| Some(Box::pin(future)))
        .collect();
    let mut results: alloc::vec::Vec<Option<T>> = (0..pending.len()).map(|_| None).collect();
    core::future::poll_fn(|cx| {
        let mut waiting = false;
        for (future, result) in pending.iter_mut().zip(&mut results) {
            let Some(active) = future else {
                continue;
            };
            match active.as_mut().poll(cx) {
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
}
impl<T> Default for Tasks<'_, T> {
    fn default() -> Self {
        Self {
            tasks: alloc::vec::Vec::new(),
        }
    }
}
impl<'a, T> Tasks<'a, T> {
    pub fn spawn(&mut self, future: impl Future<Output = T> + 'a) {
        self.tasks.push(Task::new(Box::pin(future)));
    }
    /// Poll ready tasks once, reporting whether any task ran and collecting completions.
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
