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
    let procedure = runtime.storage().add_blob(&elf);
    let combination = runtime.storage().add_tree(&[procedure.into(), input]);

    let shared = Arc::new(Shared {
        queues: KMutex::new(WorkQueues::new(
            workers,
            MAX_QUEUE_CAPACITY,
            MAX_QUEUES,
        )),
        outcome: KMutex::new(None),
        remaining: AtomicUsize::new(workers),
        executed: (0..workers).map(|_| AtomicUsize::new(0)).collect(),
    });

    for worker in 0..workers {
        let runtime = runtime.clone();
        let shared = shared.clone();
        let elf = elf.clone();

        spawn(move || {
            let result = drive(
                &runtime,
                &shared,
                worker,
                &elf,
                combination,
            );

            if let Err(error) = result {
                shared.finish(Err(error));
            }

            let _ = shared.queues.lock().close(worker);
            shared.remaining.fetch_sub(1, Ordering::Release);
        });
    }

    while shared.remaining.load(Ordering::Acquire) != 0 { kthread::yield_now(); }

    let result = shared.outcome.lock().as_ref().copied().ok_or("no scheduler result")??;

    Ok(Report {
        result,
        executed: shared
            .executed
            .iter()
            .map(|count| count.load(Ordering::Relaxed))
            .collect(),
    })
}

// need function to run the scheduler, receive effects, and resume its continuation
fn run(runtime: &FixOnArca, shared: &Shared, worker: usize, elf: &[u8], combination: Tree,) {

}

// need another function to perform the requested queue, execution, or completion operation
fn execute_task(runtime: &FixOnArca, shared: &Shared, worker: usize, name: &[u8], args: &[Value],) {

}

fn spawn(f: impl FnOnce() + Send + 'static) {
    kthread::spawn(f);
}