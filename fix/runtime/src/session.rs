extern crate alloc;
use crate::{
    Blob, Handle, Ref, Storage, Tree,
    runtime::arca::FixOnArca,
    user_scheduler_driver::{self, Outcome, RepositoryServices},
};
use alloc::{
    collections::{BTreeMap, VecDeque},
    string::String,
    sync::Arc,
    vec::Vec,
};
use kernel::kthread::{KCondvar, KMutex};

type Result = core::result::Result<Outcome, String>;
struct Reply {
    value: KMutex<Option<Result>>,
    ready: KCondvar,
}
impl Reply {
    fn finish(&self, result: Result) {
        *self.value.lock() = Some(result);
        self.ready.notify_all();
    }
    fn wait(&self) -> Result {
        let mut result = self.value.lock();
        while result.is_none() {
            result = self.ready.wait(result);
        }
        result.take().unwrap()
    }
}
pub(crate) struct Request {
    pub id: usize,
    pub combination: Tree,
    pub stdio: Option<u64>,
}
struct State {
    started: bool,
    stopped: Option<Result>,
    next: usize,
    pending: VecDeque<Request>,
    replies: BTreeMap<usize, Arc<Reply>>,
    todos: BTreeMap<usize, Vec<(Ref, Blob)>>,
}
pub struct Scheduler {
    pub scheduler: String,
    pub collector: String,
    pub memoize: fix::Memoization,
    state: KMutex<State>,
    ready: KCondvar,
}
impl Scheduler {
    pub fn new(scheduler: String, collector: String, memoize: fix::Memoization) -> Arc<Self> {
        Arc::new(Self {
            scheduler,
            collector,
            memoize,
            state: KMutex::new(State {
                started: false,
                stopped: None,
                next: 0,
                pending: VecDeque::new(),
                replies: BTreeMap::new(),
                todos: BTreeMap::new(),
            }),
            ready: KCondvar::new(),
        })
    }
    pub fn execute(
        self: &Arc<Self>,
        runtime: Arc<FixOnArca<Arc<crate::storage::repository::RepositoryStorage>>>,
        combination: Tree,
        workers: usize,
        mut services: RepositoryServices,
        profile: bool,
        stdio: Option<u64>,
    ) -> Result {
        let storage = &*services.storage;
        let mut fields = storage
            .get_tree(combination)
            .map_err(|error| alloc::format!("{error:?}"))?
            .into_vec();
        let callable = *fields
            .get(1)
            .ok_or("privileged combination needs a callable")?;
        let resolve = |name: &str| {
            let bytes = storage
                .resolve_name(name)
                .map_err(|error| alloc::format!("{error}"))?;
            storage.identify(&bytes).map_err(|error| {
                alloc::format!("could not resolve session program {name}: {error:?}")
            })
        };
        let scheduler = match resolve(&self.scheduler).ok() {
            Some(scheduler)
                if storage
                    .equals(callable, scheduler)
                    .map_err(|error| alloc::format!("{error:?}"))? =>
            {
                scheduler
            }
            _ => {
                return user_scheduler_driver::start_profiled(
                    runtime,
                    combination,
                    workers,
                    services,
                    profile,
                )
                .map(|report| report.result);
            }
        };
        let collector = resolve(&self.collector)?;
        if fields.len() != 5 {
            return Err("scheduler combination must have five fields".into());
        }
        fields[1] = scheduler;
        fields[3] = collector;
        let combination = storage.add_tree(&fields);
        let reply = Arc::new(Reply {
            value: KMutex::new(None),
            ready: KCondvar::new(),
        });
        let start = {
            let mut state = self.state.lock();
            if state.stopped.is_some() {
                return Err("session scheduler has terminated".into());
            }
            let id = state.next;
            state.next = id.checked_add(1).ok_or("session request IDs exhausted")?;
            state.replies.insert(id, reply.clone());
            state.pending.push_back(Request {
                id,
                combination,
                stdio,
            });
            state.todos.insert(id, core::mem::take(&mut services.todos));
            let start = !state.started;
            state.started = true;
            start
        };
        self.ready.notify_all();
        if start {
            let combination = storage.add_tree(&[
                fields[0],
                scheduler,
                storage.add_blob(b"serve").into(),
                collector,
                storage.add_tree(&[]).into(),
            ]);
            let session = self.clone();
            kernel::kthread::spawn(move || {
                let result = user_scheduler_driver::start_session(
                    runtime,
                    combination,
                    workers,
                    services,
                    profile,
                    session.clone(),
                )
                .map(|report| report.result);
                let replies = {
                    let mut state = session.state.lock();
                    state.stopped = Some(result.clone());
                    core::mem::take(&mut state.replies)
                };
                session.ready.notify_all();
                for reply in replies.into_values() {
                    reply.finish(result.clone());
                }
            });
        }
        reply.wait()
    }
    pub(crate) fn next(&self) -> Option<Request> {
        let mut state = self.state.lock();
        loop {
            if let Some(request) = state.pending.pop_front() {
                return Some(request);
            }
            if state.stopped.is_some() {
                return None;
            }
            state = self.ready.wait(state);
        }
    }
    pub(crate) fn reply(&self, id: usize, result: Result) {
        let reply = {
            let mut state = self.state.lock();
            state.todos.remove(&id);
            state.replies.remove(&id)
        };
        if let Some(reply) = reply {
            reply.finish(result);
        }
    }
    pub(crate) fn todo(&self, reference: Ref) -> Vec<Handle> {
        self.state
            .lock()
            .todos
            .values()
            .flatten()
            .filter(|(key, _)| key.as_handle().same_encoding(reference.as_handle()))
            .map(|(_, value)| Handle::from(*value))
            .collect()
    }
}
