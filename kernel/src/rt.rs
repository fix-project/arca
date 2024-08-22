use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Poll},
};

use alloc::{boxed::Box, collections::vec_deque::VecDeque, sync::Arc, task::Wake};

use crate::spinlock::SpinLock;

pub struct Executor {
    tasks: Arc<SpinLock<VecDeque<Arc<Task>>>>,
    sleeping: Arc<AtomicUsize>,
}

type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

struct Task {
    future: SpinLock<Option<BoxFuture>>,
    tasks: Arc<SpinLock<VecDeque<Arc<Task>>>>,
    sleeping: Arc<AtomicUsize>,
}

impl Executor {
    pub fn new() -> Executor {
        Executor {
            tasks: Arc::new(SpinLock::new(VecDeque::new())),
            sleeping: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn spawn<F>(&mut self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let mut tasks = self.tasks.lock();
        tasks.push_back(Arc::new(Task {
            future: SpinLock::new(Some(Box::pin(future))),
            tasks: self.tasks.clone(),
            sleeping: self.sleeping.clone(),
        }))
    }

    pub fn run(&mut self) {
        loop {
            if let Some(task) = {
                let mut tasks = self.tasks.lock();
                tasks.pop_front()
            } {
                let task2 = task.clone();
                let mut future_slot = task.future.lock();
                if let Some(mut future) = future_slot.take() {
                    let waker = task2.into();
                    let mut cx = Context::from_waker(&waker);
                    self.sleeping.fetch_add(1, Ordering::SeqCst);
                    if future.as_mut().poll(&mut cx).is_pending() {
                        future_slot.replace(future);
                    } else {
                        self.sleeping.fetch_sub(1, Ordering::SeqCst);
                    }
                }
            } else if self.sleeping.load(Ordering::SeqCst) == 0 {
                break;
            } else {
                crate::pause();
            }
        }
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Wake for Task {
    fn wake(self: alloc::sync::Arc<Self>) {
        let other = self.clone();
        let mut tasks = other.tasks.lock();
        self.sleeping.fetch_sub(1, Ordering::SeqCst);
        tasks.push_back(self);
    }
}

struct Yield(AtomicBool);

impl Future for Yield {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let done = self.0.load(Ordering::Acquire);
        match done {
            true => Poll::Ready(()),
            false => {
                self.0.store(true, Ordering::Release);
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

pub fn yield_now() -> impl Future<Output = ()> {
    Yield(AtomicBool::new(false))
}
