// driver that will initialize run() and set up of when user scheduler is called
use crate::runtime::arca::{FixOnArca, decode_handle, pack_handle};
use crate::user_queues::{Error, WorkQueues};
use crate::{Handle, Runtime, Storage, Tree};
use core::sync::atomic::{AtomicUsize, Ordering};
use fixhandle::Name;
use kernel::{kthread, prelude::*};

const MAX_QUEUE_CAPACITY: usize = 65536;
const MAX_QUEUES: usize = 1;

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
        self.outcome.lock().get_or_insert(result);
    }
}

pub struct Report {
    pub result: Handle,
    pub executed: Vec<usize>,
}

//check to make sure its not a local handle
fn read_handle(value: &Blob) -> Result<Handle, Error> {
    let handle = decode_handle(value).map_err(|_| Error::Invalid)?;
    if matches!(handle.name(), Name::Local { .. }) {
        return Err(Error::Invalid);
    }
    Ok(handle)
}
// accepts a root handle, just as a starting point, they can then figure out what to do with it
// or to add new functions, which these helper functions will be defined in the SDK we implement
pub fn start<S: Storage + Send + Sync + 'static>(
    runtime: Arc<FixOnArca<S>>,
    elf: Arc<[u8]>,
    root: Handle,
    workers: usize,
) -> Result<Report, &'static str> {
    if !(1..=256).contains(&workers) {
        return Err("worker count must be 1..=256");
    }
    // spawn the differne thread instnaces copying over the elf program to run
    // wait for eveyrhting to finish, and get all results into one?
    // return the result/ report
    let procedure = runtime.storage().add_blob(&elf);
    let combination = runtime.storage().add_tree(&[procedure.into(), root]);

    let shared = Arc::new(Shared {
        queues: KMutex::new(WorkQueues::new(workers, MAX_QUEUE_CAPACITY, MAX_QUEUES)),
        outcome: KMutex::new(None),
        remaining: AtomicUsize::new(workers),
        executed: (0..workers).map(|_| AtomicUsize::new(0)).collect(),
    });

    for worker in 0..workers {
        let runtime = runtime.clone();
        let shared = shared.clone();
        let elf = elf.clone();

        spawn(move || {
            let result = run(&runtime, &shared, worker, &elf, combination);

            if let Err(error) = result {
                shared.finish(Err(error));
            }

            let _ = shared.queues.lock().close(worker);
            shared.remaining.fetch_sub(1, Ordering::Release);
        });
    }

    while shared.remaining.load(Ordering::Acquire) != 0 {
        kthread::yield_now();
    }

    let result = shared
        .outcome
        .lock()
        .as_ref()
        .copied()
        .ok_or("no scheduler result")??;

    Ok(Report {
        result,
        executed: shared
            .executed
            .iter()
            .map(|count| count.load(Ordering::Relaxed))
            .collect(),
    })
}

// need function to run each instance, receive effects
fn run<S: Storage>(
    runtime: &FixOnArca<S>,
    shared: &Shared,
    worker: usize,
    elf: &[u8],
    combination: Tree,
) -> Result<(), &'static str> {
    let mut f: Function = common::elfloader::load_elf(elf).map_err(|_| "invalid scheduler ELF")?;
    f = f.apply(pack_handle(combination));

    loop {
        if shared.stopped() {
            return Ok(());
        }

        let Value::Function(effect) = f.force() else {
            return Err("scheduler returned or trapped before finish");
        };

        let Value::Tuple(mut data) = effect.into_inner().read() else {
            return Err("invalid scheduler effect");
        };

        if data.len() != 3 {
            return Err("invalid scheduler effect");
        }

        let tag: Blob = data.take(0).try_into().map_err(|_| "invalid effect tag")?;

        if &*tag != b"Symbolic" {
            return Err("invalid effect tag");
        }

        let name: Blob = data.take(1).try_into().map_err(|_| "invalid effect name")?;
        let args: Tuple = data
            .take(2)
            .try_into()
            .map_err(|_| "invalid effect arguments")?;
        let mut args: Vec<Value> = args.into_iter().collect();

        let Some(Value::Function(k)) = args.pop() else {
            return Err("missing scheduler continuation");
        };

        if !name.starts_with(b"scheduler_") {
            f = runtime
                .handle_effect(&name, args, k)
                .ok_or("unknown scheduler object effect")?;
            continue;
        }

        let (status, value) = match execute_task(runtime, shared, worker, &name, &args) {
            Ok(value) => (0, value),
            Err(error) => (
                match error {
                    Error::Empty => 1,
                    Error::Full => 2,
                    Error::Closed => 3,
                    Error::Unauthorized => 4,
                    Error::Invalid => 5,
                },
                Word::new(0).into(),
            ),
        };

        f = k.apply(Tuple::from((Word::new(status), value)));
    }
}

// need another function to perform the requested queue, execution, or completion operation
fn execute_task<S: Storage>(
    runtime: &FixOnArca<S>,
    shared: &Shared,
    worker: usize,
    name: &[u8],
    args: &[Value],
) -> Result<Value, Error> {
    match (name, args) {
        // maybe have one so people can query what the scheduler fields are?
        (b"scheduler_config", [Value::Word(field)]) => {
            let value = match field.read() {
                0 => worker as u64,
                1 => shared.executed.len() as u64,
                2 => u64::from(shared.stopped()),
                _ => return Err(Error::Invalid),
            };
            Ok(Word::new(value).into())
        }
        (b"scheduler_create_queue", [Value::Word(capacity)]) => {
            let capacity = usize::try_from(capacity.read()).map_err(|_| Error::Invalid)?;
            let queue = shared.queues.lock().create_queue(worker, capacity)?;
            Ok(Word::new(queue as u64).into())
        }

        (b"scheduler_queue_count", []) => {
            let count = shared.queues.lock().queue_count(worker)?;
            Ok(Word::new(count as u64).into())
        }

        (b"scheduler_pop", [Value::Word(queue)]) => {
            let queue = usize::try_from(queue.read()).map_err(|_| Error::Invalid)?;
            let work = shared.queues.lock().pop(worker, queue)?;
            Ok(pack_handle(work).into())
        }

        (b"scheduler_push", [Value::Word(queue), Value::Blob(value)]) => {
            let queue = usize::try_from(queue.read()).map_err(|_| Error::Invalid)?;
            let work = read_handle(value)?;
            shared.queues.lock().push(worker, queue, work)?;
            Ok(Word::new(0).into())
        }

        (b"scheduler_execute", [Value::Blob(value)]) => {
            let combination = Tree::try_from(read_handle(value)?).map_err(|_| Error::Invalid)?;
            let result = runtime.execute(combination);
            shared.executed[worker].fetch_add(1, Ordering::Relaxed);
            Ok(pack_handle(result).into())
        }

        (b"scheduler_finish", [Value::Blob(value)]) => {
            shared.finish(Ok(read_handle(value)?));
            Ok(Word::new(0).into())
        }

        (b"scheduler_yield", []) => {
            kthread::yield_now();
            Ok(Word::new(0).into())
        }

        _ => Err(Error::Invalid),
    }
}

fn spawn(f: impl FnOnce() + Send + 'static) {
    kthread::spawn(f);
}
