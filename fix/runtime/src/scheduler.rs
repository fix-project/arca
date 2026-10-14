use crate::Handle;
use alloc::{collections::VecDeque, sync::Arc};
extern crate alloc;
use core::sync::atomic::{AtomicBool, Ordering};
use kernel::kthread::{KCondvar, KMutex};

pub struct Task {
    handle: Handle,
    result: KMutex<Option<Handle>>,
    done: AtomicBool,
    completed: KCondvar,
}
impl Task {
    fn new(value: Handle) -> Self {
        Self {
            handle: value,
            result: KMutex::new(None),
            done: AtomicBool::new(false),
            completed: KCondvar::new(),
        }
    }

    pub fn task_complete(&self, value: Handle) {
        let mut result = self.result.lock();
        *result = Some(value);
        self.done.store(true, Ordering::Release);
        self.completed.notify_one();
    }

    pub fn get_handle(&self) -> Handle {
        self.handle
    }

    pub fn is_complete(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }

    pub fn take_result(&self) -> Handle {
        let mut result = self.result.lock();
        while !self.done.load(Ordering::Acquire) {
            result = self.completed.wait(result);
        }
        result
            .take()
            .expect("task marked complete without a result")
    }
}
pub struct Scheduler {
    global: KMutex<VecDeque<Arc<Task>>>,
    available: KCondvar,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            global: KMutex::new(VecDeque::new()),
            available: KCondvar::new(),
        }
    }

    pub fn get_work(&self) -> Option<Arc<Task>> {
        self.global.lock().pop_front()
    }

    pub fn wait_for_work(&self, ready: impl Fn() -> bool) -> Option<Arc<Task>> {
        let mut queue = self.global.lock();
        while queue.is_empty() && !ready() {
            queue = self.available.wait(queue);
        }
        if ready() { None } else { queue.pop_front() }
    }

    pub fn wake_workers(&self) {
        let _queue = self.global.lock();
        self.available.notify_all();
    }

    pub fn push_work(&self, handle: Handle) -> Arc<Task> {
        let task = Arc::new(Task::new(handle));
        self.global.lock().push_back(Arc::clone(&task));
        self.available.notify_one();
        task
    }
}
