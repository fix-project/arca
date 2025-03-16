use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
    time::Duration,
};

use alloc::{
    boxed::Box,
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    sync::Arc,
    task::Wake,
};
use common::util::spinlock::SpinLockGuard;
use time::OffsetDateTime;

use crate::{kvmclock, prelude::*};

pub static EXECUTOR: LazyLock<Executor> = LazyLock::new(Executor::new);

pub struct Executor {
    pending: Arc<SpinLock<VecDeque<Arc<Task>>>>,
    sleeping: SpinLock<BTreeMap<OffsetDateTime, Waker>>,
    active: AtomicUsize,
}

type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

impl Executor {
    fn new() -> Executor {
        Executor {
            pending: Arc::new(SpinLock::new(VecDeque::new())),
            sleeping: SpinLock::new(BTreeMap::new()),
            active: AtomicUsize::new(0),
        }
    }

    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let task = Arc::new(Task {
            future: SpinLock::new(Box::pin(future)),
            pending: self.pending.clone(),
        });
        let mut tasks = self.pending.lock();
        self.active.fetch_add(1, Ordering::AcqRel);
        tasks.push_back(task)
    }

    #[inline(never)]
    fn wake_sleeping(&self) -> bool {
        let mut sleeping = self.sleeping.lock();
        let mut anything = false;
        while let Some(first) = sleeping.first_entry() {
            let now = kvmclock::now();
            if &now >= first.key() {
                let waker = first.remove();
                waker.wake_by_ref();
                anything = true;
            } else {
                break;
            }
        }
        anything
    }

    #[inline(never)]
    fn run_pending(&self) -> bool {
        let mut tasks = self.pending.lock();
        let Some(task) = tasks.pop_front() else {
            return false;
        };
        SpinLockGuard::unlock(tasks);
        if task.poll().is_ready() {
            self.active.fetch_sub(1, Ordering::AcqRel);
        }
        true
    }

    #[inline(never)]
    fn sleep(&self) {
        unsafe {
            core::arch::asm!("hlt");
        }
    }

    pub fn run(&self) {
        while self.active.load(Ordering::Acquire) != 0 {
            let mut anything = false;
            anything |= self.wake_sleeping();
            anything |= self.run_pending();
            if !anything {
                self.sleep();
            }
        }
    }
}

pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    EXECUTOR.spawn(future);
}

pub fn run() {
    EXECUTOR.run();
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

struct Task {
    future: SpinLock<BoxFuture>,
    pending: Arc<SpinLock<VecDeque<Arc<Task>>>>,
}

impl Task {
    #[inline(never)]
    fn poll(self: Arc<Self>) -> Poll<()> {
        let waker = self.clone().into();
        let mut cx = Context::from_waker(&waker);
        let mut task_future = self.future.lock();
        task_future.as_mut().poll(&mut cx)
    }
}

impl Wake for Task {
    #[inline(never)]
    fn wake(self: Arc<Self>) {
        let other = self.clone();
        let mut tasks = other.pending.lock();
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

struct Delay(OffsetDateTime);

impl Future for Delay {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let now = kvmclock::now();
        if now >= self.0 {
            return Poll::Ready(());
        }
        let waker = cx.waker().clone();
        let mut sleeping = EXECUTOR.sleeping.lock();
        sleeping.insert(self.0, waker);
        Poll::Pending
    }
}

pub fn delay_for(duration: Duration) -> impl Future<Output = ()> {
    delay_until(kvmclock::now() + duration)
}

pub fn delay_until(time: OffsetDateTime) -> impl Future<Output = ()> {
    Delay(time)
}
