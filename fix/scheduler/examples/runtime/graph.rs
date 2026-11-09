use super::kernel::{queue::Queue, worker};
use super::{Key, Resolved, as_tree, fields, root};
use crate::{Evidence, Request};
use alloc::{collections::BTreeMap, rc::Rc, vec::Vec};
use core::cell::RefCell;
use fix::map::Map;
use fix::{Error, Tree, Value, coupon::Statement, tree};

pub trait Executor {
    type State: Default;
    fn known(&self, key: &(u64, Key)) -> Option<Evidence>;
    fn cached(&self, request: &Request) -> Result<Option<Evidence>, Error>;
    fn remember(&self, request: &Request, evidence: &Evidence) -> Result<(), Error>;
    fn step(
        &self,
        request: &Request,
        state: &mut Self::State,
        resolved: &Resolved,
        missing: &mut Vec<Request>,
    ) -> Result<Option<Evidence>, Error>;
}

struct Frame<S> {
    request: Request,
    dependencies: Vec<Request>,
    resolved: Resolved,
    stage: S,
}

impl<S: Default> Frame<S> {
    fn new(request: Request) -> Self {
        Self {
            request,
            dependencies: Vec::new(),
            resolved: Resolved::new(),
            stage: S::default(),
        }
    }
}

enum Claim {
    Owned,
    Done(Evidence),
    Busy,
}

enum Next {
    Step,
    Child(Request),
    Blocked(Request),
}

/// Each claimed request has an entry in `nodes` listing its dependents, and
/// is held by exactly one of: a worker's stack, the ready queue, a worker's
/// overflow, or the dependents of another claimed request. A dependent is a
/// request to resume, or an empty Tree for a worker that only waits to wake.
pub struct Graph {
    nodes: [Map; 5],
    ready: Queue,
    overflow: RefCell<Vec<Request>>,
    workers: u64,
}

impl Graph {
    pub fn join() -> Result<Self, Error> {
        if worker::id()? == 0 {
            Self::create(worker::count()?)
        } else {
            loop {
                let notification = worker::listen()?;
                if super::kernel::queue::count()? != 0 && Map::count()? >= 5 {
                    break;
                }
                notification.wait()?;
            }
            Ok(Self::open(worker::count()?))
        }
    }

    pub fn create(workers: u64) -> Result<Self, Error> {
        let graph = Self {
            nodes: [
                Map::create()?,
                Map::create()?,
                Map::create()?,
                Map::create()?,
                Map::create()?,
            ],
            ready: Queue::create(65536)?,
            overflow: RefCell::new(Vec::new()),
            workers,
        };
        worker::wake()?;
        Ok(graph)
    }

    pub fn open(workers: u64) -> Self {
        Self {
            nodes: core::array::from_fn(|index| Map::open(index as u64)),
            ready: Queue::open(0),
            overflow: RefCell::new(Vec::new()),
            workers,
        }
    }

    fn nodes(&self, statement: Statement) -> Map {
        self.nodes[match statement {
            Statement::Eval => 0,
            Statement::Apply => 1,
            Statement::Force => 2,
            Statement::Think => 3,
            Statement::Select => 4,
            _ => unreachable!(),
        }]
    }

    fn cached<E: Executor>(
        &self,
        executor: &E,
        request: &Request,
    ) -> Result<Option<Evidence>, Error> {
        executor.cached(request)
    }

    fn claim<E: Executor>(&self, executor: &E, request: &Request) -> Result<Claim, Error> {
        if let Some(evidence) = executor.known(&request.key()) {
            return Ok(Claim::Done(evidence));
        }
        let nodes = self.nodes(request.statement);
        let owned = nodes.compare_exchange(&request.input, 0, tree![])?;
        if !owned {
            return Ok(Claim::Busy);
        }
        if let Some(evidence) = self.cached(executor, request)? {
            self.finish(request)?;
            return Ok(Claim::Done(evidence));
        }
        Ok(Claim::Owned)
    }

    fn offer(&self, request: Request) -> Result<(), Error> {
        if !self.ready.push(&request.value()?)? {
            self.overflow.borrow_mut().push(request);
        } else {
            worker::wake_one()?;
        }
        Ok(())
    }

    /// Records `dependent` to resume when `dependency` finishes, or `None` to
    /// wake this worker then. Returns false if `dependency` already finished.
    fn depend(&self, dependent: Option<&Request>, dependency: &Request) -> Result<bool, Error> {
        let nodes = self.nodes(dependency.statement);
        let dependent = match dependent {
            Some(request) => request.value()?,
            None => root(tree![])?,
        };
        loop {
            let Some(entry) = nodes.get(&dependency.input)? else {
                return Ok(false);
            };
            let mut dependents = fields(&Rc::new(entry.value))?;
            dependents.push(dependent.clone());
            if nodes.compare_exchange(
                &dependency.input,
                entry.version,
                Tree::create(&dependents),
            )? {
                return Ok(true);
            }
        }
    }

    fn finish(&self, request: &Request) -> Result<(), Error> {
        let dependents = self
            .nodes(request.statement)
            .remove(&request.input)?
            .ok_or(Error::OutOfBounds)?;
        let mut waiting = false;
        for dependent in fields(&Rc::new(dependents))? {
            if as_tree(&dependent)?.len()? == 0 {
                waiting = true;
            } else {
                self.offer(Request::read(&dependent)?)?;
            }
        }
        if waiting {
            worker::wake()?;
        }
        Ok(())
    }

    fn step<E: Executor>(
        &self,
        executor: &E,
        frame: &mut Frame<E::State>,
    ) -> Result<Option<Evidence>, Error> {
        let mut missing = Vec::new();
        let result = executor.step(
            &frame.request,
            &mut frame.stage,
            &frame.resolved,
            &mut missing,
        )?;
        if let Some(evidence) = &result {
            executor.remember(&frame.request, evidence)?;
        }
        if result.is_none() {
            frame.dependencies = if missing.len() <= 1 {
                missing
            } else {
                let mut unique = BTreeMap::new();
                for dependency in missing {
                    unique.insert(dependency.key(), dependency);
                }
                unique.into_values().collect()
            };
            assert!(!frame.dependencies.is_empty());
            if self.workers > 1 {
                for dependency in frame.dependencies.iter().skip(1) {
                    match self.claim(executor, dependency)? {
                        Claim::Owned => self.offer(dependency.clone())?,
                        Claim::Done(evidence) => {
                            frame.resolved.insert(dependency.key(), evidence);
                        }
                        Claim::Busy => {}
                    }
                }
            }
        }
        Ok(result)
    }

    fn next<E: Executor>(&self, executor: &E, frame: &mut Frame<E::State>) -> Result<Next, Error> {
        let mut blocked = None;
        let mut index = 0;
        while let Some(dependency) = frame.dependencies.get(index) {
            let key = dependency.key();
            if frame.resolved.contains_key(&key) {
                frame.dependencies.remove(index);
                continue;
            }
            match self.claim(executor, dependency)? {
                Claim::Done(evidence) => {
                    frame.resolved.insert(key, evidence);
                    frame.dependencies.remove(index);
                }
                Claim::Owned => return Ok(Next::Child(dependency.clone())),
                Claim::Busy => {
                    blocked.get_or_insert_with(|| dependency.clone());
                    index += 1;
                }
            }
        }
        Ok(blocked.map_or(Next::Step, Next::Blocked))
    }

    pub fn run<E: Executor>(
        &self,
        executor: &E,
        initial: Option<Request>,
    ) -> Result<Evidence, Error> {
        let mut stack = Vec::new();
        if let Some(request) = &initial {
            match self.claim(executor, request)? {
                Claim::Owned => stack.push(Frame::new(request.clone())),
                Claim::Done(evidence) => return Ok(evidence),
                Claim::Busy => {}
            }
            self.depend(None, request)?;
        }
        loop {
            if let Some(request) = self.overflow.borrow_mut().pop() {
                stack.push(Frame::new(request));
            }
            let Some(frame) = stack.last_mut() else {
                let notification = worker::listen()?;
                if let Some(request) = &initial
                    && let Some(result) = self.cached(executor, request)?
                {
                    return Ok(result);
                }
                if let Some(value) = self.ready.pop()? {
                    stack.push(Frame::new(Request::read(&value)?));
                    continue;
                }
                fix::invalidate_externrefs()?;
                notification.wait()?;
                continue;
            };
            match self.next(executor, frame)? {
                Next::Child(request) => stack.push(Frame::new(request)),
                Next::Blocked(dependency) => {
                    if self.depend(Some(&frame.request), &dependency)? {
                        stack.pop();
                    }
                }
                Next::Step => {
                    if let Some(evidence) = self.step(executor, frame)? {
                        let frame = stack.pop().unwrap();
                        self.finish(&frame.request)?;
                        if let Some(parent) = stack.last_mut() {
                            parent.resolved.insert(frame.request.key(), evidence);
                        }
                    }
                }
            }
        }
    }
}
