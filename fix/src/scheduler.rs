//use alloc::collections::VecDeque;
use alloc::sync::Arc;
use fix::Handle;
extern crate alloc;
use core::sync::atomic::{AtomicBool, Ordering};
//use kernel::kthread;
use kernel::kthread::KMutex;

use crossbeam_queue::SegQueue;

//task that must be added
pub struct Task {
    handle: Handle,
    result: KMutex<Option<Handle>>,
    done: AtomicBool,

}
impl Task {
   
    fn new (value: Handle) -> Self{
        Self {
            handle: value,
            result: KMutex::new(None),
            done: AtomicBool::new(false),
        }
    }
    
    pub fn task_complete (&self, value: Handle) {
        *self.result.lock() = Some(value);
        self.done.store(true, Ordering::Release);
    }
    
    pub fn get_handle(&self) -> Handle {
        self.handle
    }

    pub fn is_complete(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }

    pub fn take_result(&self) -> Handle {
        self.result.lock().take().expect("task marked complete without a result")
    }
}
pub struct Scheduler {
    global: SegQueue<Arc<Task>>,
    //locals: Vec<Arc<KMutex<VecDeque<task>>>>,
    //results: ,
    //num_work_left:,
    //live_worker_count:, scheduler calls into executer??? scheduler thread, thread is like arca user pipe opens user programarca blob

}

impl Scheduler {
    pub const fn new () -> Self {
        Self { global: SegQueue::new() }
    }

    pub fn get_work (self: &Self) -> Option<Arc<Task>> {
        self.global.pop()
    }

    pub fn push_work(&self, handle: Handle)-> Arc<Task> {
        let task = Arc::new(Task::new(handle));
        self.global.push(Arc::clone(&task));
        task    
    }

}