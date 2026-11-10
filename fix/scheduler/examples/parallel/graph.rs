use super::evaluation::{EvaluationState, EvaluationStep};
use crate::{Evidence, Request, Resolved, Scheduler, as_tree, fields, object, root};
use alloc::{collections::BTreeMap, rc::Rc, vec::Vec};
use core::cell::RefCell;
use fixscheduler::kernel::{
    queue::{self, Queue},
    worker,
};
use fixwasm::map::Map;
use fixwasm::{Error, Tree, Value, coupon::Statement, tree};

struct Frame {
    request: Request,
    dependencies: Vec<Request>,
    resolved: Resolved,
    stage: EvaluationState,
}

impl Frame {
    fn new(request: Request) -> Self {
        Self {
            request,
            dependencies: Vec::new(),
            resolved: Resolved::new(),
            stage: EvaluationState::default(),
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
                if queue::count()? != 0 && Map::count()? >= 5 {
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

    fn claim(&self, services: &Scheduler, request: &Request) -> Result<Claim, Error> {
        if let Some(evidence) = services.known(&request.key()) {
            return Ok(Claim::Done(evidence));
        }
        let nodes = self.nodes(request.statement);
        let owned = nodes.compare_exchange(&request.input, 0, tree![])?;
        if !owned {
            return Ok(Claim::Busy);
        }
        if let Some(evidence) = services.cached(request.statement, &request.input)? {
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
                Tree::from_iter(&dependents),
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

    fn step(&self, services: &Scheduler, frame: &mut Frame) -> Result<Option<Evidence>, Error> {
        let (result, missing) = match frame.stage.step(
            &frame.request,
            services,
            |dependency| {
                Ok(frame
                    .resolved
                    .get(&dependency.key())
                    .cloned()
                    .or_else(|| services.known(&dependency.key())))
            },
            |input| object(input),
        )? {
            EvaluationStep::Complete(value) => (Some(value), Vec::new()),
            EvaluationStep::Dependencies(missing) => (None, missing),
            EvaluationStep::Primitive(request) => (
                Some(services.primitive(request.statement, &request.input)?),
                Vec::new(),
            ),
        };
        if let Some(evidence) = &result {
            services.remember(frame.request.statement, &frame.request.input, evidence)?;
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
                    match self.claim(services, dependency)? {
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

    fn next(&self, services: &Scheduler, frame: &mut Frame) -> Result<Next, Error> {
        let mut blocked = None;
        let mut index = 0;
        while let Some(dependency) = frame.dependencies.get(index) {
            let key = dependency.key();
            if frame.resolved.contains_key(&key) {
                frame.dependencies.remove(index);
                continue;
            }
            match self.claim(services, dependency)? {
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

    pub fn run(&self, services: &Scheduler, initial: Option<Request>) -> Result<Evidence, Error> {
        let mut stack = Vec::new();
        if let Some(request) = &initial {
            match self.claim(services, request)? {
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
                    && let Some(result) = services.cached(request.statement, &request.input)?
                {
                    return Ok(result);
                }
                if let Some(value) = self.ready.pop()? {
                    stack.push(Frame::new(Request::read(&value)?));
                    continue;
                }
                notification.wait()?;
                continue;
            };
            match self.next(services, frame)? {
                Next::Child(request) => stack.push(Frame::new(request)),
                Next::Blocked(dependency) => {
                    if self.depend(Some(&frame.request), &dependency)? {
                        stack.pop();
                    }
                }
                Next::Step => {
                    if let Some(evidence) = self.step(services, frame)? {
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
