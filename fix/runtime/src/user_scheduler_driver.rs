extern crate alloc;

use crate::runtime::arca::{EffectAction, Execution, FixOnArca, SavedClaims, Suspended};
use crate::user_maps::Maps;
use crate::user_queues::{Error, WorkQueues};
use crate::{
    Blob, EncodeView, Handle, HandleView, InvalidApplication, Object, ObjectView, Ref, Runtime,
    Storage, ThunkView, Tree,
};
use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::ToString;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use core::time::Duration;
use kernel::{
    kthread::{self, KCondvar},
    prelude::{Arc, Box, KMutex, Vec},
};

use common::protocol::control::{
    EvaluationOperation, EvaluationSnapshot, EvaluationStep, EvaluationTask, MachineProgress,
    RemoteAvailability, RuntimeStats,
};

const MAX_QUEUE_CAPACITY: usize = 65536;
const STREAM_READ_CHUNK: usize = 65536;

pub trait Services: Send + Sync {
    fn put(&self, value: Handle) -> Result<Handle, alloc::string::String>;
    fn get_todo(&self, reference: Ref) -> Result<Option<Handle>, &'static str>;
    fn set_todo(&self, reference: Ref, value: Handle) -> Result<(), &'static str>;
    fn remotes(&self) -> &[fixremote::Remote] {
        &[]
    }
    fn labels(
        &self,
    ) -> Result<Vec<(alloc::string::String, [u8; 32])>, fixrepository::RepositoryError> {
        Err(fixrepository::RepositoryError::InvalidRepository)
    }
    fn export_object(&self, _: [u8; 32], _: bool) -> Result<Vec<u8>, fixremote::Error> {
        Err(fixremote::Error::Invalid)
    }
    fn read_backing(&self, _: [u8; 32], _: u64, _: usize) -> Result<Vec<u8>, fixremote::Error> {
        Err(fixremote::Error::Invalid)
    }
    fn update_label(
        &self,
        _: &str,
        _: Option<[u8; 32]>,
        _: [u8; 32],
    ) -> Result<(), fixrepository::RepositoryError> {
        Err(fixrepository::RepositoryError::InvalidRepository)
    }
    fn information(
        &self,
        _: Handle,
        _: u32,
        _: u64,
    ) -> Result<fixremote::protocol::Information, alloc::string::String> {
        Err("object inventory is unsupported".into())
    }
    fn file_information(
        &self,
        _: &str,
        _: Handle,
    ) -> Result<fixremote::protocol::Information, alloc::string::String> {
        Err("file remote inventory is unsupported".into())
    }
    fn trusted_keys(&self) -> Result<Vec<alloc::string::String>, fixremote::Error> {
        Err(fixremote::Error::Invalid)
    }
    fn certify(
        &self,
        _: [u8; 32],
        _: &[alloc::string::String],
    ) -> Result<
        (
            fixremote::protocol::CertificateRef,
            fixremote::protocol::Record,
        ),
        fixremote::Error,
    > {
        Err(fixremote::Error::Invalid)
    }
    fn missing(&self, _: &[fixremote::protocol::Offer]) -> Result<Vec<[u8; 32]>, fixremote::Error> {
        Err(fixremote::Error::Invalid)
    }
    fn import(&self, _: &[u8]) -> Result<Vec<Handle>, fixremote::Error> {
        Err(fixremote::Error::Invalid)
    }
}

pub struct PersistenceOnly<F>(pub F);
impl<F: Fn(Handle) -> Result<Handle, &'static str> + Send + Sync> Services for PersistenceOnly<F> {
    fn put(&self, value: Handle) -> Result<Handle, alloc::string::String> {
        (self.0)(value).map_err(Into::into)
    }
    fn get_todo(&self, _: Ref) -> Result<Option<Handle>, &'static str> {
        Err("todo metadata is unsupported")
    }
    fn set_todo(&self, _: Ref, _: Handle) -> Result<(), &'static str> {
        Err("todo metadata is unsupported")
    }
}

pub struct RepositoryServices {
    pub storage: Arc<crate::storage::repository::RepositoryStorage>,
    pub todos: Vec<(Ref, Blob)>,
    pub remotes: Arc<fixremote::Config>,
}
impl Services for RepositoryServices {
    fn file_information(
        &self,
        address: &str,
        collector: Handle,
    ) -> Result<fixremote::protocol::Information, alloc::string::String> {
        let path = address
            .strip_prefix("file://")
            .ok_or("expected a file remote")?;
        let repository =
            fixrepository::Repository::open(path).map_err(|error| error.to_string())?;
        fixremote::information(&repository, self.put(collector)?, 0, None)
            .map_err(|error| error.to_string())
    }
    fn export_object(&self, name: [u8; 32], backing: bool) -> Result<Vec<u8>, fixremote::Error> {
        let repository = self.storage.repository();
        let root = repository.lookup(&name)?;
        let (transfer, backing) = fixremote::propose_export(repository, root, backing)?;
        postcard::to_allocvec(&(transfer, backing)).map_err(|_| fixremote::Error::Invalid)
    }
    fn read_backing(
        &self,
        name: [u8; 32],
        offset: u64,
        length: usize,
    ) -> Result<Vec<u8>, fixremote::Error> {
        let repository = self.storage.repository();
        let object =
            Object::try_from(repository.lookup(&name)?).map_err(|_| fixremote::Error::Invalid)?;
        repository
            .read_backing_range(object, offset, length)
            .map_err(Into::into)
    }
    fn update_label(
        &self,
        label: &str,
        expected: Option<[u8; 32]>,
        value: [u8; 32],
    ) -> Result<(), fixrepository::RepositoryError> {
        let repository = self.storage.repository();
        let value = repository.lookup(&value)?;
        repository.compare_and_write_label(label, expected, value)
    }
    fn labels(
        &self,
    ) -> Result<Vec<(alloc::string::String, [u8; 32])>, fixrepository::RepositoryError> {
        let repository = self.storage.repository();
        repository
            .labels()?
            .into_iter()
            .map(|label| {
                let name = repository.label_name(&label)?;
                Ok((label, name))
            })
            .collect()
    }
    fn remotes(&self) -> &[fixremote::Remote] {
        &self.remotes.remotes
    }
    fn information(
        &self,
        collector: Handle,
        parallelism: u32,
        memory: u64,
    ) -> Result<fixremote::protocol::Information, alloc::string::String> {
        let collector = self.put(collector)?;
        fixremote::information(
            self.storage.repository(),
            collector,
            parallelism,
            Some(memory),
        )
        .map_err(|error| error.to_string())
    }
    fn trusted_keys(&self) -> Result<Vec<alloc::string::String>, fixremote::Error> {
        fixremote::trusted_keys(&self.remotes)
    }
    fn certify(
        &self,
        name: [u8; 32],
        keys: &[alloc::string::String],
    ) -> Result<
        (
            fixremote::protocol::CertificateRef,
            fixremote::protocol::Record,
        ),
        fixremote::Error,
    > {
        fixremote::certify(
            self.storage.repository(),
            name,
            keys,
            &self.remotes.policy.identities,
            |identity, namespace, message| {
                kernel::host::os::ssh_sign(identity, namespace, message)
                    .map_err(fixcertificates::Error::Signing)
            },
        )
    }
    fn missing(
        &self,
        offers: &[fixremote::protocol::Offer],
    ) -> Result<Vec<[u8; 32]>, fixremote::Error> {
        let mut missing = Vec::new();
        for name in fixremote::missing(self.storage.repository(), offers)? {
            if !self.storage.persist_available(&name)? {
                missing.push(name);
            }
        }
        Ok(missing)
    }
    fn import(&self, bytes: &[u8]) -> Result<Vec<Handle>, fixremote::Error> {
        fixremote::import(self.storage.repository(), bytes, &self.remotes)
    }
    fn put(&self, value: Handle) -> Result<Handle, alloc::string::String> {
        self.storage.save(value).map_err(|error| match error {
            fixrepository::RepositoryError::Certificate(error) => {
                alloc::format!("could not persist scheduler value: {error}")
            }
            error => alloc::format!("could not persist scheduler value: {error}"),
        })
    }
    fn get_todo(&self, reference: Ref) -> Result<Option<Handle>, &'static str> {
        let stored = self
            .storage
            .repository()
            .get_todo(reference)
            .map_err(|_| "could not read todo metadata")?;
        let extra: Vec<_> = self
            .todos
            .iter()
            .filter(|(key, _)| key.as_handle().same_encoding(reference.as_handle()))
            .map(|(_, uri)| Handle::from(*uri))
            .collect();
        if extra.is_empty() {
            return Ok(stored);
        }
        let mut entries = extra;
        if let Some(stored) = stored {
            let tree = Tree::try_from(stored).map_err(|_| "invalid todo metadata")?;
            for uri in self
                .storage
                .get_tree(tree)
                .map_err(|_| "could not read todo metadata")?
            {
                if !entries.iter().any(|entry| entry.same_encoding(&uri)) {
                    entries.push(uri);
                }
            }
        }
        self.storage
            .repository()
            .create_tree(&entries)
            .map(|tree| Some(tree.into()))
            .map_err(|_| "could not construct todo metadata")
    }
    fn set_todo(&self, reference: Ref, value: Handle) -> Result<(), &'static str> {
        let value = self
            .put(value)
            .map_err(|_| "could not persist todo metadata")?;
        self.storage
            .repository()
            .set_todo(reference, value)
            .map_err(|_| "could not save todo metadata")
    }
}

#[derive(Default)]
struct WorkerEvent {
    state: KMutex<EventState>,
    changed: KCondvar,
}

#[derive(Default)]
struct EventState {
    epoch: u64,
    closed: bool,
}

impl WorkerEvent {
    fn listen(&self) -> u64 {
        self.state.lock().epoch
    }

    fn wait(&self, epoch: u64) -> Result<(), Error> {
        let mut state = self.state.lock();
        while !state.closed && state.epoch == epoch {
            state = self.changed.wait(state);
        }
        if state.closed {
            Err(Error::Closed)
        } else {
            Ok(())
        }
    }

    fn wake(&self, all: bool) -> Result<(), Error> {
        let mut state = self.state.lock();
        if state.closed {
            return Err(Error::Closed);
        }
        state.epoch = state.epoch.checked_add(1).ok_or(Error::Invalid)?;
        if all {
            self.changed.notify_all();
        } else {
            self.changed.notify_one();
        }
        Ok(())
    }

    fn close(&self) {
        self.state.lock().closed = true;
        self.changed.notify_all();
    }
}

/// Coupons waiting to be persisted, so that saving never blocks a worker.
#[derive(Default)]
struct Saver {
    state: KMutex<SaverState>,
    changed: KCondvar,
}

#[derive(Default)]
struct SaverState {
    queue: alloc::collections::VecDeque<Handle>,
    closed: bool,
    error: Option<alloc::string::String>,
}

impl Saver {
    fn push(&self, value: Handle) {
        self.state.lock().queue.push_back(value);
        self.changed.notify_one();
    }

    fn pop(&self) -> Option<Handle> {
        let mut state = self.state.lock();
        loop {
            if let Some(value) = state.queue.pop_front() {
                return Some(value);
            }
            if state.closed {
                return None;
            }
            state = self.changed.wait(state);
        }
    }

    fn close(&self) {
        self.state.lock().closed = true;
        self.changed.notify_all();
    }
}

#[derive(Default)]
struct Applications {
    peak_queued: AtomicUsize,
    state: KMutex<ApplicationsState>,
    changed: KCondvar,
}
#[derive(Default)]
struct ApplicationsState {
    queue: alloc::collections::VecDeque<(usize, &'static [u8], Handle)>,
    closed: bool,
}
impl Applications {
    fn push(&self, id: usize, operation: &'static [u8], argument: Handle) {
        let mut state = self.state.lock();
        state.queue.push_back((id, operation, argument));
        self.peak_queued
            .fetch_max(state.queue.len(), Ordering::Relaxed);
        drop(state);
        self.changed.notify_one();
    }
    fn pop(&self) -> Option<(usize, &'static [u8], Handle)> {
        let mut state = self.state.lock();
        loop {
            if state.closed {
                return None;
            }
            if let Some(job) = state.queue.pop_front() {
                return Some(job);
            }
            state = self.changed.wait(state);
        }
    }
    fn close(&self) {
        let mut state = self.state.lock();
        state.closed = true;
        state.queue.clear();
        self.changed.notify_all();
    }
}

fn discard_advice(storage: &dyn Storage, shared: &Shared, result: EffectResult) {
    let _ = (|| {
        let reply = storage.get_tree(Tree::try_from(result.ok()?).ok()?).ok()?;
        if integer(storage, *reply.first()?).ok()? == 2 {
            let event = storage
                .get_tree(Tree::try_from(*reply.get(1)?).ok()?)
                .ok()?;
            shared
                .suspended
                .lock()
                .remove(&integer(storage, *event.first()?).ok()?);
        }
        Some(())
    })();
}

type SharedStream = Arc<KMutex<kernel::host::net::Stream>>;
type SharedListener = Arc<kernel::host::net::TcpListener>;
type EffectResult = Result<Handle, &'static str>;
type TaskKey = ([u8; 32], Option<EvaluationOperation>);

struct PendingEffect {
    invocation: bool,
    result: Option<EffectResult>,
}

#[derive(Clone, Copy)]
struct TreeProgress {
    task: [u8; 32],
    updated: Duration,
}

struct ActiveTask {
    progress: EvaluationTask,
    started: Duration,
    scoped: bool,
}

impl ActiveTask {
    fn new(progress: EvaluationTask, scoped: bool) -> Self {
        Self {
            progress,
            started: kernel::tsc::read(),
            scoped,
        }
    }
}

#[derive(Default)]
struct ProgressObservers {
    next_id: usize,
    next_event: usize,
    cursors: BTreeMap<usize, usize>,
    steps: VecDeque<(usize, EvaluationStep)>,
}

impl ProgressObservers {
    fn subscribe(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.cursors.insert(id, self.next_event);
        id
    }

    fn record(&mut self, step: EvaluationStep) {
        if !self.cursors.is_empty() {
            self.steps.push_back((self.next_event, step));
            self.next_event += 1;
        }
    }

    fn read(&mut self, id: usize) -> Result<Vec<EvaluationStep>, Error> {
        let cursor = self.cursors.get_mut(&id).ok_or(Error::Invalid)?;
        let steps = self
            .steps
            .iter()
            .filter(|(event, _)| event >= cursor)
            .map(|(_, step)| step.clone())
            .collect();
        *cursor = self.next_event;
        self.collect();
        Ok(steps)
    }

    fn collect(&mut self) {
        let oldest = self
            .cursors
            .values()
            .copied()
            .min()
            .unwrap_or(self.next_event);
        while self.steps.front().is_some_and(|(event, _)| *event < oldest) {
            self.steps.pop_front();
        }
    }
}

struct Shared {
    advised: KMutex<BTreeMap<TaskKey, ActiveTask>>,
    observers: KMutex<ProgressObservers>,
    remote_progress: KMutex<BTreeMap<alloc::string::String, EvaluationSnapshot>>,
    labels: Vec<([u8; 32], alloc::string::String)>,
    started: Duration,
    busy: AtomicUsize,
    peak_busy: AtomicUsize,
    privileged_completed: AtomicUsize,
    cpu_started: Option<u64>,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    progress_trees: KMutex<BTreeMap<[u8; 32], TreeProgress>>,
    scheduler: Handle,
    collector: Handle,
    saved_claims: SavedClaims,
    suspended: KMutex<BTreeMap<usize, (bool, Suspended)>>,
    next_suspended: AtomicUsize,
    applications: Applications,
    saver: Saver,
    saving: AtomicBool,
    queues: KMutex<WorkQueues<Handle>>,
    maps: KMutex<Maps<Handle, Handle>>,
    event: WorkerEvent,
    streams: KMutex<Vec<Option<SharedStream>>>,
    listeners: KMutex<Vec<Option<SharedListener>>>,
    pending_effects: KMutex<BTreeMap<usize, PendingEffect>>,
    next_effect: AtomicUsize,
    completed_effects: KMutex<Vec<usize>>,
    services: Box<dyn Services>,
    outcome: KMutex<Option<Result<Outcome, &'static str>>>,
    stopped: AtomicBool,
    remaining: AtomicUsize,
    coordination_calls: AtomicUsize,
    executed: Vec<AtomicUsize>,
    profile: Option<KMutex<BTreeMap<Vec<u8>, EffectStats>>>,
}

#[derive(Default)]
struct EffectStats {
    calls: u64,
    elapsed: Duration,
    returned_claims: u64,
    nonempty_claims: u64,
}

impl Shared {
    fn record_step(&self, step: EvaluationStep) {
        self.observers.lock().record(step.clone());
        kernel::host::os::evaluation_step(step);
    }

    fn finish_task(&self, key: TaskKey, outcome: &str) {
        let task = self.advised.lock().remove(&key);
        kernel::host::os::task_finished(key.0, key.1);
        if let Some(task) = task {
            let mut name = task.progress.id;
            name[31] &= 0xe0;
            let named = self.labels.iter().any(|(label, _)| {
                let mut label = *label;
                label[31] &= 0xe0;
                label == name
            });
            if outcome != "cancelled"
                && outcome != "cached"
                && (named
                    || !task.progress.title.is_empty()
                    || key.1 == Some(EvaluationOperation::Apply))
            {
                self.record_step(EvaluationStep {
                    location: Vec::new(),
                    elapsed_ns: kernel::tsc::read().saturating_sub(task.started).as_nanos() as u64,
                    task: task.progress,
                    success: outcome == "completed",
                });
            }
        }
    }

    fn snapshot(&self) -> EvaluationSnapshot {
        let local = MachineProgress {
            location: Vec::new(),
            availability: None,
            used: true,
            runtime: Some(RuntimeStats {
                elapsed_ns: kernel::tsc::read().saturating_sub(self.started).as_nanos() as u64,
                workers: self.executed.len() as u64,
                busy_workers: self.busy.load(Ordering::Relaxed) as u64,
                queued: self.applications.state.lock().queue.len() as u64,
                peak_busy_workers: self.peak_busy.load(Ordering::Relaxed) as u64,
                peak_queued: self.applications.peak_queued.load(Ordering::Relaxed) as u64,
                completed: self
                    .executed
                    .iter()
                    .map(|count| count.load(Ordering::Relaxed) as u64)
                    .sum(),
                privileged_completed: self.privileged_completed.load(Ordering::Relaxed) as u64,
                cpu_ns: self
                    .cpu_started
                    .zip(kernel::host::os::cpu_time())
                    .and_then(|(start, now)| now.checked_sub(start)),
                bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
                bytes_received: self.bytes_received.load(Ordering::Relaxed),
                peak_memory: common::BuddyAllocator.peak_size() as u64,
                memory_used: common::BuddyAllocator.used_size() as u64,
                memory_total: common::BuddyAllocator.total_size() as u64,
            }),
            tasks: self
                .advised
                .lock()
                .values()
                .map(|task| task.progress.clone())
                .collect(),
            labels: self.labels.clone(),
        };
        let mut machines = alloc::vec![local];
        for snapshot in self.remote_progress.lock().values() {
            machines.extend(snapshot.machines.iter().cloned());
        }
        EvaluationSnapshot {
            machines,
            steps: Vec::new(),
        }
    }
    // needed to signal this whole run is completed
    fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    fn finish(&self, result: Result<Outcome, &'static str>) {
        self.outcome.lock().get_or_insert(result);
        self.stopped.store(true, Ordering::Release);
        self.event.close();
        self.applications.close();
    }

    /// Ends the run because a combination it needed is not an application.
    fn fail(&self, storage: &dyn Storage, combination: Handle) {
        self.finish(Ok(Err(InvalidApplication::request(storage, combination))));
    }
}

/// A run's result, or the `Fix.error` request that ended it without one.
pub type Outcome = Result<Handle, Tree>;

pub struct Report {
    pub result: Outcome,
    pub executed: Vec<usize>,
}

pub fn start<S: Storage + AsRef<crate::storage::memory::MemoryStorage> + Send + Sync + 'static>(
    runtime: Arc<FixOnArca<S>>,
    combination: Tree,
    workers: usize,
    services: impl Services + 'static,
) -> Result<Report, alloc::string::String> {
    start_profiled(runtime, combination, workers, services, false)
}

pub fn start_profiled<
    S: Storage + AsRef<crate::storage::memory::MemoryStorage> + Send + Sync + 'static,
>(
    runtime: Arc<FixOnArca<S>>,
    combination: Tree,
    workers: usize,
    services: impl Services + 'static,
    profile: bool,
) -> Result<Report, alloc::string::String> {
    if workers > 256 {
        return Err("worker count must be 0..=256".into());
    }
    if combination.len() < 2 {
        return Err("scheduler combination needs metadata and a callable".into());
    }
    let fields = runtime
        .storage()
        .get_tree(combination)
        .map_err(|error| alloc::format!("invalid scheduler combination: {error:?}"))?;
    let scheduler = *fields
        .get(1)
        .ok_or("scheduler combination needs a callable")?;
    let collector = *fields
        .get(3)
        .ok_or("scheduler combination needs a collector")?;
    if profile {
        runtime.enable_profile();
    }
    let initial_captures = profile.then(|| runtime.capture_count());
    let initial_elf_loads = profile.then(|| runtime.elf_loads());
    let initial_kernel_calls = profile.then(|| runtime.kernel_calls());
    let initial_call_times = profile.then(|| runtime.call_times());
    let initial_other_calls = profile.then(|| runtime.other_calls());
    let initial_applications: BTreeMap<_, _> = if profile {
        runtime.application_profiles().into_iter().collect()
    } else {
        BTreeMap::new()
    };

    let labels = services
        .labels()
        .unwrap_or_default()
        .into_iter()
        .map(|(label, name)| (name, label))
        .collect();
    let shared = Arc::new(Shared {
        advised: KMutex::new(BTreeMap::new()),
        observers: KMutex::new(ProgressObservers::default()),
        remote_progress: KMutex::new(
            services
                .remotes()
                .iter()
                .map(|remote| {
                    (
                        remote.name.clone(),
                        EvaluationSnapshot {
                            machines: alloc::vec![MachineProgress {
                                location: alloc::vec![remote.name.clone()],
                                availability: Some(RemoteAvailability::Configured),
                                ..MachineProgress::default()
                            }],
                            steps: Vec::new(),
                        },
                    )
                })
                .collect(),
        ),
        labels,
        started: kernel::tsc::read(),
        busy: AtomicUsize::new(0),
        peak_busy: AtomicUsize::new(0),
        privileged_completed: AtomicUsize::new(0),
        cpu_started: kernel::host::os::cpu_time(),
        bytes_sent: AtomicU64::new(0),
        bytes_received: AtomicU64::new(0),
        progress_trees: KMutex::new(BTreeMap::new()),
        scheduler,
        collector,
        saved_claims: SavedClaims::with_profile(profile),
        suspended: KMutex::new(BTreeMap::new()),
        pending_effects: KMutex::new(BTreeMap::new()),
        next_effect: AtomicUsize::new(0),
        completed_effects: KMutex::new(Vec::new()),
        next_suspended: AtomicUsize::new(0),
        applications: Applications::default(),
        saver: Saver::default(),
        saving: AtomicBool::new(true),
        queues: KMutex::new(WorkQueues::new(
            workers.max(1),
            MAX_QUEUE_CAPACITY,
            workers + 1,
        )),
        maps: KMutex::new(Maps::new()),
        event: WorkerEvent::default(),
        services: Box::new(services),
        streams: KMutex::new(Vec::new()),
        listeners: KMutex::new(Vec::new()),
        outcome: KMutex::new(None),
        stopped: AtomicBool::new(false),
        remaining: AtomicUsize::new(workers + 1),
        coordination_calls: AtomicUsize::new(0),
        executed: (0..workers).map(|_| AtomicUsize::new(0)).collect(),
        profile: profile.then(|| KMutex::new(BTreeMap::new())),
    });

    let coordinator = kthread::current();
    let observing = kernel::host::os::observe_evaluation();
    if observing {
        kernel::host::os::evaluation_progress(shared.snapshot());
        let shared = shared.clone();
        spawn(move || {
            while !shared.stopped() {
                kernel::host::os::sleep(250);
                if !shared.stopped() {
                    kernel::host::os::evaluation_progress(shared.snapshot());
                }
            }
        });
    }
    {
        let coordinator = coordinator.clone();
        let runtime = runtime.clone();
        let shared = shared.clone();
        spawn(move || {
            while let Some(value) = shared.saver.pop() {
                let saved = shared.services.put(value).and_then(|saved| {
                    runtime
                        .index_claim(saved)
                        .map_err(|_| "could not index a saved coupon".into())
                });
                if let Err(error) = saved {
                    shared.saver.state.lock().error.get_or_insert(error);
                }
            }
            shared.saving.store(false, Ordering::Release);
            coordinator.unpark();
        });
    }
    for worker in 0..workers {
        let coordinator = coordinator.clone();
        let runtime = runtime.clone();
        let shared = shared.clone();
        spawn(move || {
            while let Some((id, operation, argument)) = shared.applications.pop() {
                if !shared.pending_effects.lock().contains_key(&id) {
                    if operation == b"callable_resume"
                        && let Ok(token) = integer(runtime.storage(), argument)
                    {
                        shared.suspended.lock().remove(&token);
                    }
                    continue;
                }
                let busy = shared.busy.fetch_add(1, Ordering::Relaxed) + 1;
                shared.peak_busy.fetch_max(busy, Ordering::Relaxed);
                let result = handle_task(&runtime, &shared, worker, operation, &[argument], false)
                    .map(|(status, value)| {
                        let storage = runtime.storage();
                        storage
                            .add_tree(&[storage.add_blob(&status.to_le_bytes()).into(), value])
                            .into()
                    });
                shared.busy.fetch_sub(1, Ordering::Relaxed);

                let mut pending = shared.pending_effects.lock();
                if let Some(slot) = pending.get_mut(&id) {
                    slot.result = Some(result);
                    shared.completed_effects.lock().push(id);
                } else {
                    discard_advice(runtime.storage(), &shared, result);
                }
                drop(pending);
                let _ = shared.event.wake(true);
            }
            shared.remaining.fetch_sub(1, Ordering::Release);
            coordinator.unpark();
        });
    }
    {
        let coordinator = coordinator.clone();
        let runtime = runtime.clone();
        let shared = shared.clone();
        spawn(move || {
            if let Err(error) = run(&runtime, &shared, 0, combination) {
                shared.finish(Err(error));
            }
            shared.remaining.fetch_sub(1, Ordering::Release);
            coordinator.unpark();
        });
    }

    while shared.remaining.load(Ordering::Acquire) != 0 {
        kthread::park();
    }
    shared.suspended.lock().clear();
    shared.saver.close();
    while shared.saving.load(Ordering::Acquire) {
        kthread::park();
    }
    if observing {
        kernel::host::os::evaluation_progress(shared.snapshot());
    }
    if let Some(error) = shared.saver.state.lock().error.clone() {
        return Err(error);
    }

    if let Some(profile) = &shared.profile {
        let profile = profile.lock();
        let total: u64 = profile.values().map(|entry| entry.calls).sum();
        let claims = profile.get(b"storage_claims".as_slice());
        kernel::println!(
            "profile continuations: {} captures",
            runtime.capture_count() - initial_captures.unwrap()
        );
        let (loads, load_ns) = runtime.elf_loads();
        let (initial_loads, initial_ns) = initial_elf_loads.unwrap();
        kernel::println!(
            "profile ELF loading: {} loads, {} us total",
            loads - initial_loads,
            (load_ns - initial_ns) / 1000,
        );
        let (calls, gets, reads, checks) = runtime.kernel_calls();
        let (initial_calls, initial_gets, initial_reads, initial_checks) =
            initial_kernel_calls.unwrap();
        kernel::println!(
            "profile Shell kernel calls: {} total, {} tree elements, {} handle reads, {} privilege checks",
            calls - initial_calls,
            gets - initial_gets,
            reads - initial_reads,
            checks - initial_checks,
        );
        let times = runtime.call_times();
        let initial_times = initial_call_times.unwrap();
        for (name, (current, initial)) in [
            "guest force",
            "tree element",
            "handle bytes",
            "privilege",
            "tree backing",
        ]
        .into_iter()
        .zip(times.into_iter().zip(initial_times))
        {
            kernel::println!("profile runtime {name}: {} us", (current - initial) / 1000);
        }
        for (name, (current, initial)) in ["lift", "read_blob", "effect", "return", "get_blob"]
            .into_iter()
            .zip(
                runtime
                    .other_calls()
                    .into_iter()
                    .zip(initial_other_calls.unwrap()),
            )
        {
            kernel::println!("profile Shell call {name}: {} calls", current - initial);
        }
        kernel::println!(
            "profile scheduler: {} Fix effects, {} direct coordination calls, {} cache queries, {} with candidates, {} coupons offered, {} collector calls",
            total.saturating_sub(shared.coordination_calls.load(Ordering::Relaxed) as u64),
            shared.coordination_calls.load(Ordering::Relaxed),
            claims.map_or(0, |entry| entry.calls),
            claims.map_or(0, |entry| entry.nonempty_claims),
            claims.map_or(0, |entry| entry.returned_claims),
            profile
                .get(b"callable_invoke".as_slice())
                .map_or(0, |entry| entry.calls),
        );
        let (hits, misses) = shared.saved_claims.counts();
        for (key, (calls, elapsed)) in runtime.application_profiles() {
            let (executable, arguments) = key;
            let (previous_calls, previous_elapsed) =
                initial_applications.get(&key).copied().unwrap_or_default();
            if calls != previous_calls {
                let executable = unsafe { Handle::parse(executable) }.unwrap();
                kernel::println!(
                    "profile application {executable} argc {arguments}: {} slices, {} us total",
                    calls - previous_calls,
                    (elapsed - previous_elapsed) / 1000
                );
            }
        }
        kernel::println!("profile saved-claims index: {hits} reused, {misses} repository lookups");
        for (name, entry) in profile.iter() {
            kernel::println!(
                "profile request {}: {} calls, {} us total",
                core::str::from_utf8(name).unwrap_or("?"),
                entry.calls,
                entry.elapsed.as_micros(),
            );
        }
    }

    let result = shared.outcome.lock().ok_or("no scheduler result")??;
    let report = Report {
        result,
        executed: shared
            .executed
            .iter()
            .map(|count| count.load(Ordering::Relaxed))
            .collect(),
    };
    Ok(report)
}

fn run<S: Storage + AsRef<crate::storage::memory::MemoryStorage> + Send + Sync + 'static>(
    runtime: &Arc<FixOnArca<S>>,
    shared: &Arc<Shared>,
    worker: usize,
    combination: Tree,
) -> Result<(), &'static str> {
    let storage = runtime.storage();
    if shared.stopped() {
        return Ok(());
    }
    let result = runtime.execute_with_coordination(
        combination,
        &mut |effect| {
            if shared.stopped() {
                return EffectAction::Stop(storage.add_blob(b"").into());
            }
            match handle_effect(runtime, shared, worker, effect) {
                Ok(reply) if !shared.stopped() => EffectAction::Resume(reply),
                Ok(reply) => EffectAction::Stop(reply),
                Err(error) => {
                    shared.finish(Err(error));
                    EffectAction::Stop(storage.add_blob(b"").into())
                }
            }
        },
        &mut |name, args| {
            if shared.stopped() {
                return crate::arca::CoordinationAction::Stop(storage.add_blob(b"").into());
            }
            if shared.profile.is_some() {
                shared.coordination_calls.fetch_add(1, Ordering::Relaxed);
            }
            let reply = if matches!(
                name,
                b"application_start" | b"effect_ready" | b"effect_discard" | b"worker_events"
            ) {
                handle_async_task(runtime, shared, worker, name, args)
            } else {
                handle_task(runtime, shared, worker, name, args, true)
            };
            match reply {
                Ok((status, value)) if !shared.stopped() => {
                    crate::arca::CoordinationAction::Resume(status, value)
                }
                Ok((_, value)) => crate::arca::CoordinationAction::Stop(value),
                Err(error) => {
                    shared.finish(Err(error));
                    crate::arca::CoordinationAction::Stop(storage.add_blob(b"").into())
                }
            }
        },
    );
    match result {
        Ok(result) => shared.finish(Ok(Ok(result))),
        Err(InvalidApplication) => shared.fail(storage, combination.into()),
    }
    Ok(())
}

fn handle_effect<
    S: Storage + AsRef<crate::storage::memory::MemoryStorage> + Send + Sync + 'static,
>(
    runtime: &Arc<FixOnArca<S>>,
    shared: &Arc<Shared>,
    worker: usize,
    effect: Handle,
) -> Result<Handle, &'static str> {
    let storage = runtime.storage();
    let effect = Tree::try_from(effect).map_err(|_| "expected an effect combination")?;
    let effect = storage
        .get_tree(effect)
        .map_err(|_| "missing effect combination")?;
    let Some((name, args)) = effect.split_first() else {
        return Err("missing effect name");
    };
    let name = Blob::try_from(*name).map_err(|_| "expected an effect name Blob")?;
    let name = storage.get_blob(name).map_err(|_| "missing effect name")?;
    let (status, value) = handle_async_task(runtime, shared, worker, &name, args)?;
    let status = storage.add_blob(&status.to_le_bytes());
    Ok(storage.add_tree(&[status.into(), value]).into())
}

fn handle_async_task<
    S: Storage + AsRef<crate::storage::memory::MemoryStorage> + Send + Sync + 'static,
>(
    runtime: &Arc<FixOnArca<S>>,
    shared: &Arc<Shared>,
    worker: usize,
    name: &[u8],
    args: &[Handle],
) -> Result<(u64, Handle), &'static str> {
    let storage = runtime.storage();
    Ok(match (name, args) {
        (b"application_start", [effect]) => {
            if shared.executed.is_empty() {
                return Err(
                    "local application execution is disabled (-j0); configure a reachable execution remote and a distributed scheduler",
                );
            }
            let call = storage
                .get_tree(Tree::try_from(*effect).map_err(|_| "invalid invocation")?)
                .map_err(|_| "missing invocation")?;
            let [operation, argument] = call.as_ref() else {
                return Err("invalid invocation");
            };
            let operation = storage
                .get_blob(Blob::try_from(*operation).map_err(|_| "invalid invocation operation")?)
                .map_err(|_| "missing invocation operation")?;
            let operation: &'static [u8] = match operation.as_ref() {
                b"callable_run" => b"callable_run",
                b"callable_apply" => b"callable_apply",
                b"callable_invoke" => b"callable_invoke",
                b"callable_resume" => b"callable_resume",
                _ => return Err("invalid invocation operation"),
            };
            let id = shared.next_effect.fetch_add(1, Ordering::Relaxed);
            shared.pending_effects.lock().insert(
                id,
                PendingEffect {
                    invocation: true,
                    result: None,
                },
            );
            shared.applications.push(id, operation, *argument);
            (0, storage.add_blob(&(id as u64).to_le_bytes()).into())
        }
        (b"effect_start", [effect]) => {
            let id = shared.next_effect.fetch_add(1, Ordering::Relaxed);
            shared.pending_effects.lock().insert(
                id,
                PendingEffect {
                    invocation: false,
                    result: None,
                },
            );
            let effect = *effect;
            let runtime = runtime.clone();
            let shared = shared.clone();
            spawn(move || {
                let result = handle_effect(&runtime, &shared, worker, effect);
                if let Some(slot) = shared.pending_effects.lock().get_mut(&id) {
                    slot.result = Some(result);
                    shared.completed_effects.lock().push(id);
                }
                let _ = shared.event.wake(true);
            });
            (0, storage.add_blob(&(id as u64).to_le_bytes()).into())
        }
        (name @ (b"effect_ready" | b"worker_events"), []) => {
            let epoch = shared.event.listen();
            let completed = core::mem::take(&mut *shared.completed_effects.lock());
            let mut results = Vec::new();
            for id in completed {
                let effect = shared.pending_effects.lock().remove(&id);
                if let Some(effect) = effect {
                    let result = effect.result.expect("notified effect must be complete")?;
                    results.push(
                        storage
                            .add_tree(&[
                                storage.add_blob(&(id as u64).to_le_bytes()).into(),
                                result,
                            ])
                            .into(),
                    );
                }
            }
            let results = storage.add_tree(&results).into();
            if name == b"worker_events" {
                (
                    0,
                    storage
                        .add_tree(&[storage.add_blob(&epoch.to_le_bytes()).into(), results])
                        .into(),
                )
            } else {
                (0, results)
            }
        }
        (b"effect_discard", [id]) => {
            let id = integer(storage, *id).map_err(|_| "invalid effect token")?;
            if let Some(PendingEffect {
                invocation: true,
                result: Some(result),
            }) = shared.pending_effects.lock().remove(&id)
            {
                discard_advice(storage, shared, result);
            }
            (0, storage.add_blob(b"").into())
        }
        _ => handle_task(runtime, shared, worker, name, args, false)?,
    })
}

fn handle_task<S: Storage + AsRef<crate::storage::memory::MemoryStorage>>(
    runtime: &FixOnArca<S>,
    shared: &Shared,
    worker: usize,
    name: &[u8],
    args: &[Handle],
    coordination_only: bool,
) -> Result<(u64, Handle), &'static str> {
    let start = shared.profile.as_ref().map(|_| kernel::tsc::read());
    let storage = runtime.storage();
    if shared.executed.is_empty()
        && matches!(
            name,
            b"callable_run" | b"callable_apply" | b"callable_invoke" | b"callable_resume"
        )
    {
        return Err(
            "local application execution is disabled (-j0); configure a reachable execution remote and a distributed scheduler",
        );
    }
    if coordination_only
        && !matches!(
            name,
            b"callable_invoke"
                | b"callable_resume"
                | b"callable_discard"
                | b"map_create"
                | b"map_named"
                | b"map_count"
                | b"map_get"
                | b"map_contains_keys"
                | b"map_compare_exchange"
                | b"map_remove"
                | b"storage_cache"
                | b"storage_claims"
                | b"storage_identify"
                | b"queue_create"
                | b"queue_named"
                | b"queue_count"
                | b"queue_pop"
                | b"queue_push"
                | b"worker_listen"
                | b"worker_wait"
                | b"worker_wake"
                | b"worker_wake_one"
                | b"worker_config"
                | b"worker_finish"
                | b"worker_yield"
                | b"tree_progress"
                | b"evaluation_start"
                | b"evaluation_finish"
                | b"load_finish"
        )
    {
        return Err("unsupported scheduler coordination operation");
    }
    let task: Result<(u64, Handle), TaskError> = (|| match (name, args) {
        (b"callable_run", [value]) => {
            let execution = runtime.run_observed(value_combination(*value)?);
            observed_reply(runtime, shared, worker, true, *value, execution).map_err(Into::into)
        }
        (b"callable_apply", [value]) => {
            let execution = runtime.execute_observed(value_combination(*value)?, false);
            observed_reply(runtime, shared, worker, true, *value, execution).map_err(Into::into)
        }
        (b"callable_invoke", [value]) => {
            let execution = runtime.execute_observed(combination(*value)?, true);
            observed_reply(runtime, shared, worker, false, *value, execution).map_err(Into::into)
        }
        (b"callable_discard", [token]) => {
            let token = integer(storage, *token)?;
            shared
                .suspended
                .lock()
                .remove(&token)
                .ok_or(Error::Invalid)?;
            Ok((0, storage.add_blob(b"").into()))
        }
        (b"callable_resume", [token]) => {
            let token = integer(storage, *token)?;
            let (application, suspended) = shared
                .suspended
                .lock()
                .remove(&token)
                .ok_or(Error::Invalid)?;
            let task = suspended.combination().into();
            let execution = runtime.resume_observed(suspended);
            observed_reply(runtime, shared, worker, application, task, execution)
                .map_err(Into::into)
        }
        _ => execute_task(runtime, shared, worker, name, args).map(|value| (0, value)),
    })();
    let returned_claims = if name == b"storage_claims" {
        task.as_ref()
            .ok()
            .and_then(|(_, handle)| Tree::try_from(*handle).ok())
            .map_or(0, |tree| tree.len() as u64)
    } else {
        0
    };
    let (status, value): (u64, Handle) = match task {
        Ok(reply) => reply,
        Err(TaskError::Message(message)) => (5, storage.add_blob(message.as_bytes()).into()),
        Err(TaskError::Status(error)) => (
            match error {
                Error::Empty => 1,
                Error::Full => 2,
                Error::Closed => 3,
                Error::Unauthorized => 4,
                Error::Invalid => 5,
            },
            storage.add_blob(&0u64.to_le_bytes()).into(),
        ),
    };
    if let (Some(start), Some(profile)) = (start, &shared.profile) {
        let elapsed = kernel::tsc::read().saturating_sub(start);
        let mut profile = profile.lock();
        let entry = profile.entry(name.to_vec()).or_default();
        entry.calls += 1;
        entry.elapsed += elapsed;
        entry.returned_claims += returned_claims;
        entry.nonempty_claims += u64::from(returned_claims != 0);
    }
    Ok((status, value))
}

enum TaskError {
    Status(Error),
    Message(alloc::string::String),
}

impl From<Error> for TaskError {
    fn from(error: Error) -> Self {
        Self::Status(error)
    }
}

fn observed_reply<S: Storage + AsRef<crate::storage::memory::MemoryStorage>>(
    runtime: &FixOnArca<S>,
    shared: &Shared,
    worker: usize,
    application: bool,
    task: Handle,
    execution: Result<Execution, InvalidApplication>,
) -> Result<(u64, Handle), Error> {
    let storage = runtime.storage();
    match execution {
        Ok(Execution::Complete(value)) => {
            if application && let Ok(task) = storage.canonicalize(task).map(Handle::from) {
                shared.finish_task(
                    (*task.as_bytes(), Some(EvaluationOperation::Apply)),
                    "completed",
                );
            }
            if application {
                shared.executed[worker].fetch_add(1, Ordering::Relaxed);
            } else {
                shared.privileged_completed.fetch_add(1, Ordering::Relaxed);
            }
            Ok((0, value))
        }
        Ok(Execution::Advice { event, suspended }) => {
            if matches!(event.key.as_slice(), b"fix::title" | b"fix::progress")
                && let Ok(target) = storage.canonicalize(event.target).map(Handle::from)
            {
                let operation = storage
                    .canonicalize(task)
                    .ok()
                    .map(Handle::from)
                    .filter(|task| task.same_encoding(&target))
                    .map(|_| EvaluationOperation::Apply);
                let mut tasks = shared.advised.lock();
                let task = tasks
                    .entry((*target.as_bytes(), operation))
                    .or_insert_with(|| {
                        ActiveTask::new(
                            EvaluationTask {
                                id: *target.as_bytes(),
                                operation,
                                ..EvaluationTask::default()
                            },
                            false,
                        )
                    });
                let task = &mut task.progress;
                if event.key == b"fix::title" {
                    task.title =
                        alloc::string::String::from_utf8_lossy(&event.payload).into_owned();
                } else if event.payload.len() == 16 {
                    let completed = u64::from_le_bytes(event.payload[..8].try_into().unwrap());
                    let total = u64::from_le_bytes(event.payload[8..].try_into().unwrap());
                    if completed <= total {
                        task.progress = Some((completed, total));
                    }
                }
                drop(tasks);
                kernel::host::os::task_advice(
                    *target.as_bytes(),
                    operation,
                    &event.key,
                    &event.payload,
                );
            }
            let token = shared
                .next_suspended
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                .map_err(|_| Error::Invalid)?;
            shared
                .suspended
                .lock()
                .insert(token, (application, suspended));
            let token = storage.add_blob(&(token as u64).to_le_bytes());
            let key = storage.add_blob(&event.key);
            let payload = storage.add_blob(&event.payload);
            let event = storage.add_tree(&[token.into(), event.target, key.into(), payload.into()]);
            Ok((2, event.into()))
        }
        Err(InvalidApplication) => {
            shared.fail(storage, task);
            Err(Error::Invalid)
        }
    }
}

fn unsigned(storage: &dyn Storage, value: Handle) -> Result<u64, Error> {
    let value = Blob::try_from(value).map_err(|_| Error::Invalid)?;
    let bytes = storage.get_blob(value).map_err(|_| Error::Invalid)?;
    let bytes = bytes.as_ref().try_into().map_err(|_| Error::Invalid)?;
    Ok(u64::from_le_bytes(bytes))
}

fn integer(storage: &dyn Storage, value: Handle) -> Result<usize, Error> {
    usize::try_from(unsigned(storage, value)?).map_err(|_| Error::Invalid)
}

fn object_descriptor(value: Handle) -> Object {
    match value.view() {
        HandleView::Object(object) => *object,
        HandleView::Ref(reference) => reference.object_descriptor(),
        HandleView::Thunk(thunk) => match thunk.view() {
            ThunkView::Identification(reference) => reference.object_descriptor(),
            ThunkView::Application(reference) | ThunkView::Selection(reference) => {
                reference.object_descriptor().object()
            }
        },
        HandleView::Encode(encode) => match encode.view() {
            EncodeView::Strict(thunk) | EncodeView::Shallow(thunk) => {
                object_descriptor(thunk.into())
            }
        },
    }
}

fn identified(storage: &dyn Storage, value: Handle) -> Result<Handle, Error> {
    let object = object_descriptor(value);
    storage
        .identify(object.as_handle().as_bytes())
        .map_err(|_| {
            if storage
                .identify(object.into_ref().as_handle().as_bytes())
                .is_ok()
            {
                Error::Empty
            } else {
                Error::Invalid
            }
        })
}

fn todo_reference(storage: &dyn Storage, value: Handle) -> Result<Ref, Error> {
    let reference = object_descriptor(value).into_ref();
    Ref::try_from(
        storage
            .identify(reference.as_handle().as_bytes())
            .map_err(|_| Error::Invalid)?,
    )
    .map_err(|_| Error::Invalid)
}

fn text(storage: &dyn Storage, value: Handle) -> Result<alloc::string::String, Error> {
    let blob = Blob::try_from(value).map_err(|_| Error::Invalid)?;
    let bytes = storage.get_blob(blob).map_err(|_| Error::Invalid)?;
    alloc::string::String::from_utf8(bytes.into_vec()).map_err(|_| Error::Invalid)
}

fn add_stream(shared: &Shared, stream: kernel::host::net::Stream) -> usize {
    insert_resource(&mut shared.streams.lock(), Arc::new(KMutex::new(stream)))
}

fn insert_resource<T>(resources: &mut Vec<Option<T>>, resource: T) -> usize {
    if let Some(index) = resources.iter().position(Option::is_none) {
        resources[index] = Some(resource);
        index
    } else {
        let index = resources.len();
        resources.push(Some(resource));
        index
    }
}

fn combination(value: Handle) -> Result<Tree, Error> {
    let tree = Tree::try_from(value).map_err(|_| Error::Invalid)?;
    if tree.len() < 2 {
        return Err(Error::Invalid);
    }
    Ok(tree)
}

fn value_combination(value: Handle) -> Result<Tree, Error> {
    if !value.is_value() {
        return Err(Error::Invalid);
    }
    combination(value)
}

fn execute_task<S: Storage + AsRef<crate::storage::memory::MemoryStorage>>(
    runtime: &FixOnArca<S>,
    shared: &Shared,
    worker: usize,
    name: &[u8],
    args: &[Handle],
) -> Result<Handle, TaskError> {
    let storage = runtime.storage();
    let number = |value: usize| storage.add_blob(&(value as u64).to_le_bytes()).into();
    match (name, args) {
        (b"evaluation_labels", []) => {
            let labels = shared
                .labels
                .iter()
                .map(|(name, _)| storage.add_blob(name).into())
                .collect::<Vec<_>>();
            Ok(storage.add_tree(&labels).into())
        }
        (b"evaluation_start", [input, operation]) => {
            let input = Handle::from(storage.canonicalize(*input).map_err(|_| Error::Invalid)?);
            let operation = match unsigned(storage, *operation)? {
                1 => EvaluationOperation::Evaluate,
                2 => EvaluationOperation::Apply,
                3 => EvaluationOperation::Force,
                4 => EvaluationOperation::Think,
                6 => EvaluationOperation::Select,
                _ => return Err(Error::Invalid.into()),
            };
            let child = match (operation, input.view()) {
                (EvaluationOperation::Force, _) => Some((input, EvaluationOperation::Think)),
                (EvaluationOperation::Think, HandleView::Thunk(thunk))
                    if !matches!(thunk.view(), ThunkView::Identification(_)) =>
                {
                    Some((
                        object_descriptor(input).into(),
                        EvaluationOperation::Evaluate,
                    ))
                }
                (EvaluationOperation::Evaluate, HandleView::Encode(encode)) => {
                    let thunk = match encode.view() {
                        EncodeView::Strict(thunk) | EncodeView::Shallow(thunk) => thunk,
                    };
                    Some((thunk.into(), EvaluationOperation::Force))
                }
                _ => None,
            };
            let children = child
                .into_iter()
                .filter_map(|(child, operation)| {
                    storage
                        .canonicalize(child)
                        .ok()
                        .map(Handle::from)
                        .map(|child| (*child.as_bytes(), Some(operation)))
                })
                .collect();
            shared.advised.lock().insert(
                (*input.as_bytes(), Some(operation)),
                ActiveTask::new(
                    EvaluationTask {
                        id: *input.as_bytes(),
                        operation: Some(operation),
                        children,
                        ..EvaluationTask::default()
                    },
                    true,
                ),
            );
            kernel::host::os::task_advice(*input.as_bytes(), Some(operation), b"fix::title", b"");
            Ok(number(0))
        }
        (b"evaluation_finish", [input, operation, outcome]) => {
            let input = Handle::from(storage.canonicalize(*input).map_err(|_| Error::Invalid)?);
            let operation = match unsigned(storage, *operation)? {
                1 => EvaluationOperation::Evaluate,
                2 => EvaluationOperation::Apply,
                3 => EvaluationOperation::Force,
                4 => EvaluationOperation::Think,
                6 => EvaluationOperation::Select,
                _ => return Err(Error::Invalid.into()),
            };
            shared.finish_task(
                (*input.as_bytes(), Some(operation)),
                &text(storage, *outcome)?,
            );
            Ok(number(0))
        }
        (b"load_start", [input, source, destination]) => {
            let input = Handle::from(storage.canonicalize(*input).map_err(|_| Error::Invalid)?);
            let endpoint = |value| -> Result<Vec<alloc::string::String>, Error> {
                let name = text(storage, value)?;
                Ok(if name.is_empty() {
                    Vec::new()
                } else {
                    alloc::vec![name]
                })
            };
            shared.advised.lock().insert(
                (*input.as_bytes(), Some(EvaluationOperation::Load)),
                ActiveTask::new(
                    EvaluationTask {
                        id: *input.as_bytes(),
                        operation: Some(EvaluationOperation::Load),
                        transfer: Some((endpoint(*source)?, endpoint(*destination)?)),
                        ..EvaluationTask::default()
                    },
                    false,
                ),
            );
            kernel::host::os::evaluation_progress(shared.snapshot());
            Ok(number(0))
        }
        (b"load_finish", [input]) => {
            let input = Handle::from(storage.canonicalize(*input).map_err(|_| Error::Invalid)?);
            shared
                .advised
                .lock()
                .remove(&(*input.as_bytes(), Some(EvaluationOperation::Load)));
            kernel::host::os::evaluation_progress(shared.snapshot());
            Ok(number(0))
        }
        (b"progress_status", []) => {
            let bytes = postcard::to_allocvec(&shared.snapshot()).map_err(|_| Error::Invalid)?;
            Ok(storage.add_blob(&bytes).into())
        }
        (b"progress_subscribe", []) => Ok(number(shared.observers.lock().subscribe())),
        (b"progress_unsubscribe", [id]) => {
            let mut observers = shared.observers.lock();
            observers.cursors.remove(&integer(storage, *id)?);
            observers.collect();
            Ok(number(0))
        }
        (b"progress_snapshot", [id]) => {
            let mut snapshot = shared.snapshot();
            snapshot.steps = shared.observers.lock().read(integer(storage, *id)?)?;
            let bytes = postcard::to_allocvec(&snapshot).map_err(|_| Error::Invalid)?;
            Ok(storage.add_blob(&bytes).into())
        }
        (b"remote_status", [location, status, workers, memory, error]) => {
            let location = text(storage, *location)?;
            let availability = match text(storage, *status)?.as_str() {
                "connecting" => RemoteAvailability::Connecting,
                "available" => RemoteAvailability::Available {
                    workers: unsigned(storage, *workers)?,
                    memory_bytes: if storage
                        .get_blob(Blob::try_from(*memory).map_err(|_| Error::Invalid)?)
                        .map_err(|_| Error::Invalid)?
                        .is_empty()
                    {
                        None
                    } else {
                        Some(unsigned(storage, *memory)?)
                    },
                },
                "unavailable" => RemoteAvailability::Unavailable(text(storage, *error)?),
                _ => return Err(Error::Invalid.into()),
            };
            {
                let mut remotes = shared.remote_progress.lock();
                let snapshot = remotes.entry(location.clone()).or_default();
                if let Some(machine) = snapshot
                    .machines
                    .iter_mut()
                    .find(|machine| machine.location.len() == 1 && machine.location[0] == location)
                {
                    machine.availability = Some(availability);
                } else {
                    snapshot.machines.push(MachineProgress {
                        location: alloc::vec![location],
                        availability: Some(availability),
                        ..MachineProgress::default()
                    });
                }
            }
            kernel::host::os::evaluation_progress(shared.snapshot());
            Ok(number(0))
        }
        (b"remote_progress", [location, snapshot, used]) => {
            let location = text(storage, *location)?;
            let bytes = storage
                .get_blob(Blob::try_from(*snapshot).map_err(|_| Error::Invalid)?)
                .map_err(|_| Error::Invalid)?;
            let mut snapshot: EvaluationSnapshot =
                postcard::from_bytes(&bytes).map_err(|_| Error::Invalid)?;
            let used = unsigned(storage, *used)? != 0;
            for machine in &mut snapshot.machines {
                machine.used &= used;
                machine.location.insert(0, location.clone());
            }
            for mut step in snapshot.steps.drain(..) {
                step.location.insert(0, location.clone());
                shared.record_step(step);
            }
            {
                let mut remotes = shared.remote_progress.lock();
                if let Some(previous) = remotes.get(&location) {
                    for machine in &mut snapshot.machines {
                        machine.used |= previous
                            .machines
                            .iter()
                            .find(|old| old.location == machine.location)
                            .is_some_and(|old| old.used);
                        if machine.availability.is_none() {
                            machine.availability = previous
                                .machines
                                .iter()
                                .find(|old| old.location == machine.location)
                                .and_then(|old| old.availability.clone());
                        }
                    }
                }
                remotes.insert(location, snapshot);
            }
            kernel::host::os::evaluation_progress(shared.snapshot());
            Ok(number(0))
        }
        (b"sleep", [milliseconds]) => {
            kernel::host::os::sleep(unsigned(storage, *milliseconds)?);
            Ok(number(0))
        }
        (b"log", [message]) => {
            kernel::println!("{}", text(storage, *message)?);
            Ok(number(0))
        }
        (b"remote_config", []) => {
            let remotes = shared
                .services
                .remotes()
                .iter()
                .map(|remote| {
                    storage
                        .add_tree(&[
                            storage.add_blob(remote.name.as_bytes()).into(),
                            storage.add_blob(remote.address.as_bytes()).into(),
                            storage
                                .add_blob(remote.scheduler.as_deref().unwrap_or("").as_bytes())
                                .into(),
                            storage
                                .add_blob(remote.collector.as_deref().unwrap_or("").as_bytes())
                                .into(),
                        ])
                        .into()
                })
                .collect::<Vec<_>>();
            Ok(storage.add_tree(&remotes).into())
        }
        (b"remote_programs", []) => Ok(storage
            .add_tree(&[shared.scheduler, shared.collector])
            .into()),
        (b"remote_trust_collector", [collector]) => {
            let collector = Tree::try_from(*collector).map_err(|_| {
                TaskError::Message("requested coupon collector is not a Tree".into())
            })?;
            if !runtime.trusts_tagger(collector).map_err(|error| {
                TaskError::Message(alloc::format!(
                    "cannot validate requested coupon collector: {error}"
                ))
            })? {
                return Err(TaskError::Message(
                    "requested coupon collector is not a trusted tagger on this machine".into(),
                ));
            }
            Ok(number(0))
        }
        (b"tree_progress", [task, children, completed, total]) => {
            let completed = unsigned(storage, *completed)?;
            let total = unsigned(storage, *total)?;
            if completed > total {
                return Err(Error::Invalid.into());
            }
            let key = *task.as_bytes();
            let now = kernel::tsc::read();
            let previous = shared.progress_trees.lock().get(&key).copied();
            if let Some(previous) = previous
                && completed != total
                && now.saturating_sub(previous.updated) < Duration::from_millis(100)
            {
                return Ok(number(0));
            }
            let task = match previous {
                Some(previous) => previous.task,
                None => *storage
                    .canonicalize(*task)
                    .map_err(|error| {
                        TaskError::Message(alloc::format!(
                            "cannot identify progress task: {error:?}"
                        ))
                    })
                    .map(Handle::from)?
                    .as_bytes(),
            };
            let children = storage
                .get_tree(Tree::try_from(*children).map_err(|_| Error::Invalid)?)
                .map_err(|_| Error::Invalid)?;
            let children = children
                .iter()
                .map(|child| {
                    storage
                        .canonicalize(*child)
                        .map(|child| *Handle::from(child).as_bytes())
                        .map_err(|error| {
                            TaskError::Message(alloc::format!(
                                "cannot identify Tree child: {error:?}"
                            ))
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let finished = {
                let mut tasks = shared.advised.lock();
                let key = (task, Some(EvaluationOperation::Evaluate));
                let entry = tasks.entry(key).or_insert_with(|| {
                    ActiveTask::new(
                        EvaluationTask {
                            id: task,
                            operation: Some(EvaluationOperation::Evaluate),
                            ..EvaluationTask::default()
                        },
                        false,
                    )
                });
                entry.progress.progress = Some((completed, total));
                if !children.is_empty() {
                    entry.progress.children = children
                        .iter()
                        .map(|child| (*child, Some(EvaluationOperation::Evaluate)))
                        .collect();
                }
                let finished = completed == total && !entry.scoped;
                if finished {
                    tasks.remove(&key);
                }
                finished
            };
            kernel::host::os::tree_progress(task, children, completed, total);
            if finished {
                kernel::host::os::task_finished(task, Some(EvaluationOperation::Evaluate));
            }
            if completed == total {
                shared.progress_trees.lock().remove(&key);
            } else {
                shared
                    .progress_trees
                    .lock()
                    .insert(key, TreeProgress { task, updated: now });
            }
            Ok(number(0))
        }
        (b"remote_labels", []) => {
            let labels = shared
                .services
                .labels()
                .map_err(|error| TaskError::Message(error.to_string()))?;
            let bytes = postcard::to_allocvec(&labels).map_err(|_| Error::Invalid)?;
            Ok(storage.add_blob(&bytes).into())
        }
        (b"remote_export", [name, backing]) => {
            let bytes = storage
                .get_blob(Blob::try_from(*name).map_err(|_| Error::Invalid)?)
                .map_err(|_| Error::Invalid)?;
            let name: [u8; 32] = bytes.as_ref().try_into().map_err(|_| Error::Invalid)?;
            let bytes = shared
                .services
                .export_object(name, unsigned(storage, *backing)? != 0)
                .map_err(|error| TaskError::Message(error.to_string()))?;
            Ok(storage.add_blob(&bytes).into())
        }
        (b"remote_read_backing", [name, offset, length]) => {
            let bytes = storage
                .get_blob(Blob::try_from(*name).map_err(|_| Error::Invalid)?)
                .map_err(|_| Error::Invalid)?;
            let name = bytes.as_ref().try_into().map_err(|_| Error::Invalid)?;
            let bytes = shared
                .services
                .read_backing(
                    name,
                    unsigned(storage, *offset)?,
                    usize::try_from(unsigned(storage, *length)?).map_err(|_| Error::Invalid)?,
                )
                .map_err(|error| TaskError::Message(error.to_string()))?;
            Ok(storage.add_blob(&bytes).into())
        }
        (b"remote_label_update", [bytes]) => {
            let bytes = storage
                .get_blob(Blob::try_from(*bytes).map_err(|_| Error::Invalid)?)
                .map_err(|_| Error::Invalid)?;
            let (label, expected, value): (alloc::string::String, Option<[u8; 32]>, [u8; 32]) =
                postcard::from_bytes(&bytes).map_err(|_| Error::Invalid)?;
            shared
                .services
                .update_label(&label, expected, value)
                .map_err(|error| TaskError::Message(error.to_string()))?;
            Ok(number(0))
        }
        (b"remote_information", [collector, memory]) => {
            let information = shared
                .services
                .information(
                    *collector,
                    shared
                        .executed
                        .len()
                        .try_into()
                        .map_err(|_| Error::Invalid)?,
                    unsigned(storage, *memory)?,
                )
                .map_err(TaskError::Message)?;
            let bytes = postcard::to_allocvec(&information).map_err(|_| Error::Invalid)?;
            Ok(storage.add_blob(&bytes).into())
        }
        (b"remote_file_information", [address, collector]) => {
            let information = shared
                .services
                .file_information(&text(storage, *address)?, *collector)
                .map_err(TaskError::Message)?;
            let bytes = postcard::to_allocvec(&information).map_err(|_| Error::Invalid)?;
            Ok(storage.add_blob(&bytes).into())
        }
        (b"remote_trusted_keys", []) => {
            let keys = shared
                .services
                .trusted_keys()
                .map_err(|error| TaskError::Message(error.to_string()))?;
            let bytes = postcard::to_allocvec(&keys).map_err(|_| Error::Invalid)?;
            Ok(storage.add_blob(&bytes).into())
        }
        (b"storage_certify", [bytes]) => {
            let bytes = storage
                .get_blob(Blob::try_from(*bytes).map_err(|_| Error::Invalid)?)
                .map_err(|_| Error::Invalid)?;
            let (name, keys): ([u8; 32], Vec<alloc::string::String>) =
                postcard::from_bytes(&bytes).map_err(|_| Error::Invalid)?;
            let certificate = shared
                .services
                .certify(name, &keys)
                .map_err(|error| TaskError::Message(error.to_string()))?;
            let bytes = postcard::to_allocvec(&certificate).map_err(|_| Error::Invalid)?;
            Ok(storage.add_blob(&bytes).into())
        }
        (b"storage_missing", [bytes]) => {
            let bytes = storage
                .get_blob(Blob::try_from(*bytes).map_err(|_| Error::Invalid)?)
                .map_err(|_| Error::Invalid)?;
            let offers: Vec<fixremote::protocol::Offer> =
                postcard::from_bytes(&bytes).map_err(|_| Error::Invalid)?;
            let names = shared
                .services
                .missing(&offers)
                .map_err(|error| TaskError::Message(error.to_string()))?;
            let bytes = postcard::to_allocvec(&names).map_err(|_| Error::Invalid)?;
            Ok(storage.add_blob(&bytes).into())
        }
        (b"storage_import", [bytes]) => {
            let bytes = storage
                .get_blob(Blob::try_from(*bytes).map_err(|_| Error::Invalid)?)
                .map_err(|_| Error::Invalid)?;
            let roots = shared
                .services
                .import(&bytes)
                .map_err(|error| TaskError::Message(error.to_string()))?;
            Ok(storage.add_tree(&roots).into())
        }
        (b"tcp_listen", [ip, port]) => {
            let ip = Blob::try_from(*ip).map_err(|_| Error::Invalid)?;
            let ip = storage.get_blob(ip).map_err(|_| Error::Invalid)?;
            let ip: &[u8; 4] = ip.as_ref().try_into().map_err(|_| Error::Invalid)?;
            let port = u16::try_from(integer(storage, *port)?).map_err(|_| Error::Invalid)?;
            let listener =
                kernel::host::net::TcpListener::try_bind(ip, port).map_err(|_| Error::Invalid)?;
            Ok(number(insert_resource(
                &mut shared.listeners.lock(),
                Arc::new(listener),
            )))
        }
        (b"tcp_accept", [listener]) => {
            let id = integer(storage, *listener)?;
            let listener = shared
                .listeners
                .lock()
                .get(id)
                .and_then(Option::as_ref)
                .cloned()
                .ok_or(Error::Closed)?;
            let stream = listener.try_accept().map_err(|_| Error::Invalid)?;
            Ok(number(add_stream(shared, stream)))
        }
        (b"tcp_local_addr", [listener]) => {
            let id = integer(storage, *listener)?;
            let listener = shared
                .listeners
                .lock()
                .get(id)
                .and_then(Option::as_ref)
                .cloned()
                .ok_or(Error::Closed)?;
            let (ip, port) = listener.local_addr().map_err(|_| Error::Invalid)?;
            Ok(storage
                .add_tree(&[storage.add_blob(&ip).into(), number(port as usize)])
                .into())
        }
        (b"tcp_listener_close", [listener]) => {
            let id = integer(storage, *listener)?;
            let listener = shared
                .listeners
                .lock()
                .get_mut(id)
                .and_then(Option::take)
                .ok_or(Error::Closed)?;
            drop(listener);
            Ok(number(0))
        }
        (b"tcp_connect", [host, port]) => {
            let host = text(storage, *host)?;
            let port = u16::try_from(integer(storage, *port)?).map_err(|_| Error::Invalid)?;
            let stream =
                kernel::host::net::Stream::try_connect(&host, port).map_err(|_| Error::Invalid)?;
            Ok(number(add_stream(shared, stream)))
        }
        (b"ssh_connect", [host, repository, scheduler, collector]) => {
            let scheduler = text(storage, *scheduler)?;
            let collector = text(storage, *collector)?;
            let stream = kernel::host::net::Stream::ssh_connect(
                &text(storage, *host)?,
                &text(storage, *repository)?,
                (!scheduler.is_empty()).then_some(scheduler.as_str()),
                (!collector.is_empty()).then_some(collector.as_str()),
            )
            .map_err(|_| Error::Invalid)?;
            Ok(number(add_stream(shared, stream)))
        }
        (b"stdio_stream", []) => {
            let stream = kernel::host::net::Stream::stdio_stream().map_err(|_| Error::Invalid)?;
            Ok(number(add_stream(shared, stream)))
        }
        (b"uri_open", [uri]) => {
            let stream = kernel::host::net::Stream::open_uri(&text(storage, *uri)?)
                .map_err(|_| Error::Invalid)?;
            Ok(number(add_stream(shared, stream)))
        }
        (b"uri_read_directory", [uri]) => {
            use common::protocol::control::DirectoryEntryKind;
            let entries = kernel::host::fs::read_directory_uri(&text(storage, *uri)?)
                .map_err(|_| Error::Invalid)?;
            let mut children = Vec::with_capacity(entries.len());
            for entry in entries {
                let (kind, target) = match entry.kind {
                    DirectoryEntryKind::File => (4, Vec::new()),
                    DirectoryEntryKind::Directory => (3, Vec::new()),
                    DirectoryEntryKind::Symlink(target) => (7, target),
                    DirectoryEntryKind::Unsupported => (0, Vec::new()),
                };
                children.push(
                    storage
                        .add_tree(&[
                            storage.add_blob(&entry.name).into(),
                            number(kind),
                            storage.add_blob(&target).into(),
                        ])
                        .into(),
                );
            }
            Ok(storage.add_tree(&children).into())
        }
        (b"stream_send", [stream, bytes]) => {
            let id = integer(storage, *stream)?;
            let bytes = Blob::try_from(*bytes).map_err(|_| Error::Invalid)?;
            let bytes = storage.get_blob(bytes).map_err(|_| Error::Invalid)?;
            let stream = shared
                .streams
                .lock()
                .get(id)
                .and_then(Option::as_ref)
                .cloned()
                .ok_or(Error::Closed)?;
            let mut stream = stream.lock();
            let length = stream.try_send(&bytes).map_err(|_| Error::Invalid)?;
            shared
                .bytes_sent
                .fetch_add(length as u64, Ordering::Relaxed);
            Ok(number(length))
        }
        (b"stream_receive", [stream, length]) => {
            let id = integer(storage, *stream)?;
            let length = integer(storage, *length)?.min(STREAM_READ_CHUNK);
            let mut bytes = alloc::vec![0; length];
            let stream = shared
                .streams
                .lock()
                .get(id)
                .and_then(Option::as_ref)
                .cloned()
                .ok_or(Error::Closed)?;
            let mut stream = stream.lock();
            let length = stream.try_recv(&mut bytes).map_err(|_| Error::Invalid)?;
            shared
                .bytes_received
                .fetch_add(length as u64, Ordering::Relaxed);
            Ok(storage.add_blob(&bytes[..length]).into())
        }
        (b"stream_close", [stream]) => {
            let id = integer(storage, *stream)?;
            let stream = shared
                .streams
                .lock()
                .get_mut(id)
                .and_then(Option::take)
                .ok_or(Error::Closed)?;
            drop(stream);
            Ok(number(0))
        }
        (b"worker_listen", []) => Ok(storage
            .add_blob(&shared.event.listen().to_le_bytes())
            .into()),
        (b"worker_wait", [epoch]) => {
            shared.event.wait(unsigned(storage, *epoch)?)?;
            Ok(number(0))
        }
        (name @ (b"worker_wake" | b"worker_wake_one"), []) => {
            shared.event.wake(name == b"worker_wake")?;
            Ok(number(0))
        }
        (b"worker_config", [field]) => {
            let value = match integer(storage, *field)? {
                0 => worker,
                1 => shared.executed.len(),
                2 => usize::from(shared.stopped()),
                3 => usize::from(runtime.memoize != fix::Memoization::Never),
                _ => return Err(Error::Invalid.into()),
            };
            Ok(number(value))
        }
        (b"queue_named", [name, capacity]) => {
            let name = Blob::try_from(*name).map_err(|_| Error::Invalid)?;
            let name = storage.get_blob(name).map_err(|_| Error::Invalid)?;
            let queue = shared
                .queues
                .lock()
                .named(worker, &name, integer(storage, *capacity)?)?;
            Ok(number(queue as usize))
        }
        (b"queue_create", [capacity]) => {
            let queue = shared
                .queues
                .lock()
                .create_queue(worker, integer(storage, *capacity)?)?;
            Ok(number(queue as usize))
        }
        (b"map_create", []) => Ok(number(shared.maps.lock().create())),
        (b"map_named", [name]) => {
            let name = Blob::try_from(*name).map_err(|_| Error::Invalid)?;
            let name = storage.get_blob(name).map_err(|_| Error::Invalid)?;
            Ok(number(shared.maps.lock().named(&name)))
        }
        (b"map_count", []) => Ok(number(shared.maps.lock().count())),
        (b"map_get", [map, key]) => {
            let map = integer(storage, *map)?;
            let (version, value) = shared
                .maps
                .lock()
                .get(map, |other| Ok(key.as_bytes().cmp(other.as_bytes())))
                .map(|(version, value)| (version, *value))?;
            Ok(storage
                .add_tree(&[storage.add_blob(&version.to_le_bytes()).into(), value])
                .into())
        }
        (b"map_contains_keys", [map, keys]) => {
            let map = integer(storage, *map)?;
            let keys = storage
                .get_tree(Tree::try_from(*keys).map_err(|_| Error::Invalid)?)
                .map_err(|_| Error::Invalid)?;
            let present = {
                let maps = shared.maps.lock();
                keys.iter()
                    .map(|key| {
                        match maps.get(map, |other| Ok(key.as_bytes().cmp(other.as_bytes()))) {
                            Ok(_) => Ok(1u8),
                            Err(Error::Empty) => Ok(0u8),
                            Err(error) => Err(error),
                        }
                    })
                    .collect::<Result<Vec<_>, Error>>()?
            };
            Ok(storage.add_blob(&present).into())
        }
        (b"map_compare_exchange", [map, key, version, value]) => {
            let map = integer(storage, *map)?;
            let expected = unsigned(storage, *version)?;
            let version = shared
                .maps
                .lock()
                .replace(map, *key, expected, *value, |other| {
                    Ok(key.as_bytes().cmp(other.as_bytes()))
                })?;
            Ok(storage.add_blob(&version.to_le_bytes()).into())
        }
        (b"map_remove", [map, key]) => {
            let map = integer(storage, *map)?;
            let (_, _, value) = shared
                .maps
                .lock()
                .remove(map, |other| Ok(key.as_bytes().cmp(other.as_bytes())))?;
            Ok(value)
        }
        (b"queue_count", []) => Ok(number(shared.queues.lock().queue_count(worker)?)),
        (b"queue_pop", [queue]) => {
            let queue = integer(storage, *queue)?;
            Ok(shared.queues.lock().pop(worker, queue)?)
        }
        (b"queue_push", [queue, value]) => {
            let queue = integer(storage, *queue)?;
            shared.queues.lock().push(worker, queue, *value)?;
            Ok(number(0))
        }
        (b"storage_selection", [value]) => {
            let tree = Tree::try_from(*value).map_err(|_| Error::Invalid)?;
            if !value.is_value() {
                return Err(Error::Invalid.into());
            }
            Ok(storage.select(tree).map_err(|_| Error::Invalid)?)
        }
        (b"storage_select", [value]) => {
            let tree = Tree::try_from(*value).map_err(|_| Error::Invalid)?;
            if !value.is_value() {
                return Err(Error::Invalid.into());
            }
            let coupon = storage
                .selection_certificate(tree)
                .map_err(|_| Error::Invalid)?;
            Ok(coupon.into())
        }
        (b"storage_eval", [value]) => {
            let coupon = storage
                .evaluation_certificate(*value)
                .map_err(|_| Error::Invalid)?;
            Ok(coupon.into())
        }
        (b"storage_canonicalize", [value]) => storage
            .canonicalization_certificate(*value)
            .map(Into::into)
            .map_err(|_| Error::Invalid.into()),
        (b"storage_identify", [value]) => {
            let value = identified(storage, *value)?;
            let available = match value.view() {
                HandleView::Object(object) => match object.view() {
                    ObjectView::Blob(blob) => storage.has_blob(*blob),
                    ObjectView::Tree(tree) => storage.has_tree(*tree),
                },
                _ => true,
            };
            if !available {
                return Err(Error::Empty.into());
            }
            Ok(value)
        }
        (b"storage_get_todo", [key]) => shared
            .services
            .get_todo(todo_reference(storage, *key)?)
            .map_err(|_| Error::Invalid)?
            .ok_or(Error::Empty.into()),
        (b"storage_set_todo", [key, value]) => {
            shared
                .services
                .set_todo(todo_reference(storage, *key)?, *value)
                .map_err(|_| Error::Invalid)?;
            Ok(number(0))
        }
        (b"storage_cache", [value, completed]) => {
            runtime.index_claim(*value).map_err(|_| Error::Invalid)?;
            let recommendation = unsigned(storage, *completed)?;
            if recommendation > 2 {
                return Err(Error::Invalid.into());
            }
            if runtime.persists_coupon(recommendation) {
                shared.saver.push(*value);
            }
            Ok(*value)
        }
        (b"storage_put", [value]) => {
            runtime.index_claim(*value).map_err(|_| Error::Invalid)?;
            let saved = shared.services.put(*value).map_err(|_| Error::Invalid)?;
            runtime.index_claim(saved).map_err(|_| Error::Invalid)?;
            Ok(saved)
        }
        (b"storage_claims", [operation, input]) => {
            let coupons = runtime
                .cached_claims_in(&[*operation, *input], &shared.saved_claims)
                .map_err(|_| Error::Invalid)?;
            let coupons: Vec<_> = coupons.into_iter().map(Into::into).collect();
            Ok(storage.add_tree(&coupons).into())
        }
        (b"worker_finish", [value]) => {
            shared.finish(Ok(Ok(*value)));
            Ok(number(0))
        }
        (b"worker_yield", []) => {
            kthread::yield_now();
            Ok(number(0))
        }
        _ => Err(Error::Invalid.into()),
    }
}

fn spawn(f: impl FnOnce() + Send + 'static) {
    kthread::spawn(f);
}
