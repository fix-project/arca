extern crate alloc;

use crate::runtime::arca::{EffectAction, Execution, FixOnArca, SavedClaims, Suspended};
use crate::user_maps::Maps;
use crate::user_queues::{Error, WorkQueues};
use crate::{
    Blob, EncodeView, Handle, HandleView, InvalidApplication, Object, ObjectView, Ref, Runtime,
    Storage, ThunkView, Tree,
};
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::time::Duration;
use kernel::{
    kthread::{self, KCondvar},
    prelude::{Arc, Box, KMutex, Vec},
};

const MAX_QUEUE_CAPACITY: usize = 65536;
const STREAM_READ_CHUNK: usize = 65536;

pub trait Services: Send + Sync {
    fn put(&self, value: Handle) -> Result<Handle, &'static str>;
    fn get_todo(&self, reference: Ref) -> Result<Option<Handle>, &'static str>;
    fn set_todo(&self, reference: Ref, value: Handle) -> Result<(), &'static str>;
}

pub struct PersistenceOnly<F>(pub F);
impl<F: Fn(Handle) -> Result<Handle, &'static str> + Send + Sync> Services for PersistenceOnly<F> {
    fn put(&self, value: Handle) -> Result<Handle, &'static str> {
        (self.0)(value)
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
}
impl Services for RepositoryServices {
    fn put(&self, value: Handle) -> Result<Handle, &'static str> {
        self.storage
            .save(value)
            .map_err(|_| "could not persist scheduler value")
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
        let value = self.put(value)?;
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
    error: Option<&'static str>,
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

struct Shared {
    saved_claims: SavedClaims,
    suspended: KMutex<BTreeMap<usize, (bool, Suspended)>>,
    next_suspended: AtomicUsize,
    saver: Saver,
    saving: AtomicBool,
    queues: KMutex<WorkQueues<Handle>>,
    maps: KMutex<Maps<Handle, Handle>>,
    event: WorkerEvent,
    streams: Vec<KMutex<Vec<Option<kernel::host::net::Stream>>>>,
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
    // needed to signal this whole run is completed
    fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    fn finish(&self, result: Result<Outcome, &'static str>) {
        self.outcome.lock().get_or_insert(result);
        self.stopped.store(true, Ordering::Release);
        self.event.close();
    }

    /// Ends the run because a combination it needed is not an application.
    fn fail(&self, storage: &dyn Storage) {
        self.finish(Ok(Err(InvalidApplication::request(storage))));
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
) -> Result<Report, &'static str> {
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
) -> Result<Report, &'static str> {
    if !(1..=256).contains(&workers) {
        return Err("worker count must be 1..=256");
    }
    if combination.len() < 2 {
        return Err("scheduler combination needs metadata and a callable");
    }
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

    let shared = Arc::new(Shared {
        saved_claims: SavedClaims::with_profile(profile),
        suspended: KMutex::new(BTreeMap::new()),
        next_suspended: AtomicUsize::new(0),
        saver: Saver::default(),
        saving: AtomicBool::new(true),
        queues: KMutex::new(WorkQueues::new(workers, MAX_QUEUE_CAPACITY, workers + 1)),
        maps: KMutex::new(Maps::new()),
        event: WorkerEvent::default(),
        services: Box::new(services),
        streams: (0..workers).map(|_| KMutex::new(Vec::new())).collect(),
        outcome: KMutex::new(None),
        stopped: AtomicBool::new(false),
        remaining: AtomicUsize::new(workers),
        coordination_calls: AtomicUsize::new(0),
        executed: (0..workers).map(|_| AtomicUsize::new(0)).collect(),
        profile: profile.then(|| KMutex::new(BTreeMap::new())),
    });

    let coordinator = kthread::current();
    {
        let coordinator = coordinator.clone();
        let runtime = runtime.clone();
        let shared = shared.clone();
        spawn(move || {
            while let Some(value) = shared.saver.pop() {
                let saved = shared.services.put(value).and_then(|saved| {
                    runtime
                        .index_claim(saved)
                        .map_err(|_| "could not index a saved coupon")
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
            let result = run(&runtime, &shared, worker, combination);

            if let Err(error) = result {
                shared.finish(Err(error));
            }

            let _ = shared.queues.lock().close(worker);
            shared.remaining.fetch_sub(1, Ordering::Release);
            coordinator.unpark();
        });
    }

    while shared.remaining.load(Ordering::Acquire) != 0 {
        kthread::park();
    }
    shared.saver.close();
    while shared.saving.load(Ordering::Acquire) {
        kthread::park();
    }
    if let Some(error) = shared.saver.state.lock().error {
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

fn run<S: Storage + AsRef<crate::storage::memory::MemoryStorage>>(
    runtime: &FixOnArca<S>,
    shared: &Shared,
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
            match handle_task(runtime, shared, worker, name, args, true) {
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
        Err(InvalidApplication) => shared.fail(storage),
    }
    Ok(())
}

fn handle_effect<S: Storage + AsRef<crate::storage::memory::MemoryStorage>>(
    runtime: &FixOnArca<S>,
    shared: &Shared,
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
    let (status, value) = handle_task(runtime, shared, worker, &name, args, false)?;
    let status = storage.add_blob(&status.to_le_bytes());
    Ok(storage.add_tree(&[status.into(), value]).into())
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
    if coordination_only
        && !matches!(
            name,
            b"callable_invoke"
                | b"callable_resume"
                | b"map_create"
                | b"map_count"
                | b"map_get"
                | b"map_compare_exchange"
                | b"map_remove"
                | b"storage_cache"
                | b"storage_claims"
                | b"storage_identify"
                | b"queue_create"
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
        )
    {
        return Err("unsupported scheduler coordination operation");
    }
    let task = (|| match (name, args) {
        (b"callable_apply", [value]) => {
            let execution = runtime.execute_observed(value_combination(*value)?, false);
            observed_reply(runtime, shared, worker, true, execution)
        }
        (b"callable_invoke", [value]) => {
            let execution = runtime.execute_observed(combination(*value)?, true);
            observed_reply(runtime, shared, worker, false, execution)
        }
        (b"callable_resume", [token]) => {
            let token = integer(storage, *token)?;
            let (application, suspended) = shared
                .suspended
                .lock()
                .remove(&token)
                .ok_or(Error::Invalid)?;
            let execution = runtime.resume_observed(suspended);
            observed_reply(runtime, shared, worker, application, execution)
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
        Err(error) => (
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

fn observed_reply<S: Storage + AsRef<crate::storage::memory::MemoryStorage>>(
    runtime: &FixOnArca<S>,
    shared: &Shared,
    worker: usize,
    application: bool,
    execution: Result<Execution, InvalidApplication>,
) -> Result<(u64, Handle), Error> {
    let storage = runtime.storage();
    match execution {
        Ok(Execution::Complete(value)) => {
            if application {
                runtime.index_claim(value).map_err(|_| Error::Invalid)?;
                shared.executed[worker].fetch_add(1, Ordering::Relaxed);
            }
            Ok((0, value))
        }
        Ok(Execution::Advice { event, suspended }) => {
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
            shared.fail(storage);
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

fn add_stream(shared: &Shared, worker: usize, stream: kernel::host::net::Stream) -> usize {
    let mut streams = shared.streams[worker].lock();
    if let Some(index) = streams.iter().position(Option::is_none) {
        streams[index] = Some(stream);
        index
    } else {
        let index = streams.len();
        streams.push(Some(stream));
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
) -> Result<Handle, Error> {
    let storage = runtime.storage();
    let number = |value: usize| storage.add_blob(&(value as u64).to_le_bytes()).into();
    match (name, args) {
        (b"tcp_connect", [host, port]) => {
            let host = text(storage, *host)?;
            let port = u16::try_from(integer(storage, *port)?).map_err(|_| Error::Invalid)?;
            let stream =
                kernel::host::net::Stream::try_connect(&host, port).map_err(|_| Error::Invalid)?;
            Ok(number(add_stream(shared, worker, stream)))
        }
        (b"uri_open", [uri]) => {
            let stream = kernel::host::net::Stream::open_uri(&text(storage, *uri)?)
                .map_err(|_| Error::Invalid)?;
            Ok(number(add_stream(shared, worker, stream)))
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
            let mut streams = shared.streams[worker].lock();
            let stream = streams
                .get_mut(id)
                .and_then(Option::as_mut)
                .ok_or(Error::Closed)?;
            Ok(number(stream.try_send(&bytes).map_err(|_| Error::Invalid)?))
        }
        (b"stream_receive", [stream, length]) => {
            let id = integer(storage, *stream)?;
            let length = integer(storage, *length)?.min(STREAM_READ_CHUNK);
            let mut bytes = alloc::vec![0; length];
            let mut streams = shared.streams[worker].lock();
            let stream = streams
                .get_mut(id)
                .and_then(Option::as_mut)
                .ok_or(Error::Closed)?;
            let length = stream.try_recv(&mut bytes).map_err(|_| Error::Invalid)?;
            Ok(storage.add_blob(&bytes[..length]).into())
        }
        (b"stream_close", [stream]) => {
            let id = integer(storage, *stream)?;
            let stream = shared.streams[worker]
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
                _ => return Err(Error::Invalid),
            };
            Ok(number(value))
        }
        (b"queue_create", [capacity]) => {
            let queue = shared
                .queues
                .lock()
                .create_queue(worker, integer(storage, *capacity)?)?;
            Ok(number(queue as usize))
        }
        (b"map_create", []) => Ok(number(shared.maps.lock().create())),
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
        (b"storage_select", [value]) => {
            let tree = Tree::try_from(*value).map_err(|_| Error::Invalid)?;
            if !value.is_value() {
                return Err(Error::Invalid);
            }
            let coupon = storage
                .selection_certificate(tree)
                .map_err(|_| Error::Invalid)?;
            runtime
                .index_claim(coupon.into())
                .map_err(|_| Error::Invalid)?;
            Ok(coupon.into())
        }
        (b"storage_eval", [value]) => {
            let coupon = storage
                .evaluation_certificate(*value)
                .map_err(|_| Error::Invalid)?;
            runtime
                .index_claim(coupon.into())
                .map_err(|_| Error::Invalid)?;
            Ok(coupon.into())
        }
        (b"storage_canonicalize", [value]) => storage
            .canonicalization_certificate(*value)
            .map(Into::into)
            .map_err(|_| Error::Invalid),
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
                return Err(Error::Empty);
            }
            Ok(value)
        }
        (b"storage_get_todo", [key]) => shared
            .services
            .get_todo(todo_reference(storage, *key)?)
            .map_err(|_| Error::Invalid)?
            .ok_or(Error::Empty),
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
                return Err(Error::Invalid);
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
        (b"storage_claims", [statement, input]) => {
            let coupons = runtime
                .cached_claims_in(&[*statement, *input], &shared.saved_claims)
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
        _ => Err(Error::Invalid),
    }
}

fn spawn(f: impl FnOnce() + Send + 'static) {
    kthread::spawn(f);
}
