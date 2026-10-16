// driver that will initialize run() and set up of when user scheduler is called
use crate::runtime::arca::{FixOnArca};
use crate::user_queues::{Error, WorkQueues};
use crate::{Handle, Runtime, Tree};
use core::sync::atomic::{AtomicUsize, Ordering};
use fixhandle::Name;
use kernel::{kthread, prelude::*};

struct Shared {
    queues: KMutex<WorkQueues<Handle>>,
    outcome: KMutex<Option<Result<Handle, &'static str>>>,
    remaining: AtomicUsize,
    executed: Vec<AtomicUsize>,
}

impl Shared {
    // needed to signal this whole run is completed
    fn stopped(&self) -> bool {
        self.outcome.lock().is_some()
    }

    fn finish(&self, result: Result<Handle, &'static str>) {
    }
}

pub struct Report {
    pub result: Handle,
    pub executed: Vec<usize>,
}

pub fn start(runtime: Arc<FixOnArca>, elf: Arc<[u8]>, input: Handle, workers: usize) -> Result<Report, &'static str> {
    if !(1..=256).contains(&workers) {
        return Err("worker count must be 1..=256");
    }
    // spawn the differne thread instnaces copying over the elf program to run
    // wait for eveyrhting to finish, and get all results into one?
    // return the result/ report
    spawn(| ) //move it )
}

// need function to run the scheduler, receive effects, and resume its continuation
fn run() {

}

// need another function to perform the requested queue, execution, or completion operation
fn execute_task() {

}

fn spawn(f: impl FnOnce() + Send + 'static) {
    kthread::spawn(f);
}