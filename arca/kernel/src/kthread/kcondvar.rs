use super::{KMutexGuard, Thread, current, park};
use alloc::{collections::VecDeque, sync::Arc};
use common::util::spinlock::SpinLock;
use core::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct KCondvar {
    waiters: SpinLock<VecDeque<Arc<Waiter>>>,
}

struct Waiter {
    thread: Arc<Thread>,
    notified: AtomicBool,
}

impl Waiter {
    fn notify(&self) {
        self.notified.store(true, Ordering::Release);
        self.thread.unpark();
    }
}

impl KCondvar {
    pub const fn new() -> Self {
        Self {
            waiters: SpinLock::new(VecDeque::new()),
        }
    }

    pub fn wait<'a, T: ?Sized>(&self, guard: KMutexGuard<'a, T>) -> KMutexGuard<'a, T> {
        let waiter = Arc::new(Waiter {
            thread: current(),
            notified: AtomicBool::new(false),
        });
        self.waiters.lock().push_back(waiter.clone());
        let mutex = guard.lock;
        drop(guard);
        while !waiter.notified.load(Ordering::Acquire) {
            park();
        }
        mutex.lock()
    }

    pub fn notify_one(&self) {
        let waiter = self.waiters.lock().pop_front();
        if let Some(waiter) = waiter {
            waiter.notify();
        }
    }

    pub fn notify_all(&self) {
        let waiters = core::mem::take(&mut *self.waiters.lock());
        for waiter in waiters {
            waiter.notify();
        }
    }
}
