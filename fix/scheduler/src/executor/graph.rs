pub use super::node::Node;
use crate::shell::Value;
use alloc::{
    boxed::Box,
    collections::{BTreeMap, VecDeque},
    rc::{Rc, Weak},
    string::String,
    vec::Vec,
};
use core::cell::RefCell;
use core::{
    fmt::Write,
    future::{Future, poll_fn},
    task::{Poll, Waker},
};
use fixwasm::{Error, Value as _};

const RUNNING: u64 = 1;
const READY: u64 = 2;

pub enum Claim {
    Owned,
    Done(Value),
    Busy,
}

type WakerSlot = RefCell<Option<Waker>>;

#[derive(Clone)]
pub(crate) struct Failure {
    pub code: Error,
    pub detail: Option<Rc<str>>,
}

impl From<Error> for Failure {
    fn from(code: Error) -> Self {
        Self { code, detail: None }
    }
}

#[derive(Default)]
pub struct Graph {
    nodes: RefCell<BTreeMap<Node, u64>>,
    ready: RefCell<VecDeque<Node>>,
    completed: RefCell<BTreeMap<Node, Result<Value, Failure>>>,
    failures: RefCell<BTreeMap<Node, Failure>>,
    current: RefCell<Option<Node>>,
    executing: RefCell<Option<Node>>,
    wakers: RefCell<BTreeMap<Node, Vec<Weak<WakerSlot>>>>,
}

impl Graph {
    pub fn fail(&self, code: Error, detail: String) -> Error {
        self.propagate(Failure {
            code,
            detail: Some(Rc::from(detail)),
        })
    }

    fn propagate(&self, failure: Failure) -> Error {
        let code = failure.code;
        if failure.detail.is_some()
            && let Some(parent) = self.current().or_else(|| self.executing.borrow().clone())
        {
            self.failures.borrow_mut().insert(parent, failure);
        }
        code
    }

    fn failure(&self, node: &Node, code: Error) -> Failure {
        let failure = self.failures.borrow_mut().remove(node);
        if let Some(failure) = failure.filter(|failure| failure.code == code) {
            return failure;
        }
        let detail = if node.is_apply() {
            node.application_description().ok().map(|application| {
                Rc::from(if code == Error::MemoryUnavailable {
                    alloc::format!("memory admission rejected for {application}")
                } else {
                    alloc::format!(
                        "scheduler request failed: {code:?} while applying {application}"
                    )
                })
            })
        } else {
            None
        }
        .or_else(|| {
            let bytes = crate::storage::canonical_value(node.input())
                .as_ref()
                .unwrap_or(node.input())
                .raw_handle_bytes()
                .ok()?;
            let mut input = String::new();
            for byte in bytes {
                let _ = write!(input, "{byte:02x}");
            }
            Some(Rc::from(alloc::format!(
                "scheduler request failed: {code:?} in operation {} on {input}",
                node.code(),
            )))
        });
        Failure { code, detail }
    }

    /// Return the parent established by the active dependency traversal.
    pub fn current(&self) -> Option<Node> {
        self.current.borrow().clone()
    }

    /// Establish the dependency parent only while polling the supplied future.
    pub async fn within<T>(&self, node: &Node, evaluate: impl Future<Output = T>) -> T {
        let mut future = Box::pin(evaluate);
        poll_fn(|cx| {
            let previous = self.current.replace(Some(node.clone()));
            let result = future.as_mut().poll(cx);
            self.current.replace(previous);
            result
        })
        .await
    }

    /// Share an inline evaluation, awaiting the existing owner when already claimed.
    pub async fn evaluate(
        &self,
        node: &Node,
        evaluate: impl Future<Output = Result<Value, Error>>,
    ) -> Result<Value, Error> {
        if let Some(result) = self.completed(node)? {
            return Ok(result);
        }
        if self.executing.borrow().as_ref() == Some(node) {
            return self.shared(node, evaluate).await;
        }
        match self.claim(node)? {
            Claim::Done(result) => Ok(result),
            Claim::Busy => self.wait(node).await,
            Claim::Owned => {
                let mut owner = Owned {
                    graph: self,
                    node: node.clone(),
                    finished: false,
                };
                let result = self.shared(node, evaluate).await;
                let result = result.map_err(|code| self.failure(node, code));
                self.complete_result(node, &result)?;
                owner.finished = true;
                result.map_err(|failure| self.propagate(failure))
            }
        }
    }

    async fn shared(
        &self,
        node: &Node,
        evaluate: impl Future<Output = Result<Value, Error>>,
    ) -> Result<Value, Error> {
        let mut completed = Box::pin(self.wait(node));
        let mut evaluating = Box::pin(self.within(node, evaluate));
        poll_fn(|cx| {
            if let Poll::Ready(result) = completed.as_mut().poll(cx) {
                return Poll::Ready(result);
            }
            evaluating.as_mut().poll(cx)
        })
        .await
    }

    /// Submit work without tying its lifetime to any one waiter.
    pub fn submit(&self, node: &Node) -> Result<(), Error> {
        if matches!(self.claim(node)?, Claim::Owned) {
            self.offer(node.clone())?;
        }
        Ok(())
    }

    /// Submit a dependency and await its shared result.
    pub async fn defer(&self, node: &Node) -> Result<Value, Error> {
        self.defer_detailed(node)
            .await
            .map_err(|failure| self.propagate(failure))
    }

    pub(crate) async fn defer_detailed(&self, node: &Node) -> Result<Value, Failure> {
        if let Some(result) = self.completed.borrow().get(node).cloned() {
            return result;
        }
        self.submit(node)?;
        self.wait_detailed(node).await
    }

    /// Own queued work until completion. Dropping this future requeues the operation.
    pub async fn execute(
        &self,
        node: Node,
        policy: &(impl super::Scheduler + ?Sized),
    ) -> Result<(), Error> {
        let mut owner = Owned {
            graph: self,
            node: node.clone(),
            finished: false,
        };
        let result = {
            let mut future = Box::pin(node.dispatch(policy));
            poll_fn(|cx| {
                let previous = self.executing.replace(Some(node.clone()));
                let result = future.as_mut().poll(cx);
                self.executing.replace(previous);
                result
            })
            .await
        };
        let result = result.map_err(|code| self.failure(&node, code));
        self.complete_result(&node, &result)?;
        owner.finished = true;
        Ok(())
    }

    pub fn claim(&self, request: &Node) -> Result<Claim, Error> {
        if let Some(result) = self.completed(request)? {
            return Ok(Claim::Done(result));
        }
        match self.nodes.borrow_mut().entry(request.clone()) {
            alloc::collections::btree_map::Entry::Occupied(_) => Ok(Claim::Busy),
            alloc::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(RUNNING);
                Ok(Claim::Owned)
            }
        }
    }

    pub fn completed(&self, request: &Node) -> Result<Option<Value>, Error> {
        self.completed
            .borrow()
            .get(request)
            .cloned()
            .transpose()
            .map_err(|failure| self.propagate(failure))
    }

    pub fn scheduled_elsewhere(&self, node: &Node) -> bool {
        self.executing.borrow().as_ref() != Some(node) && self.nodes.borrow().contains_key(node)
    }

    /// Claim delegated work and requeue it locally if the transfer is abandoned.
    pub fn reserve(&self, node: &Node) -> Result<Option<Owned<'_>>, Error> {
        Ok(matches!(self.claim(node)?, Claim::Owned).then(|| Owned {
            graph: self,
            node: node.clone(),
            finished: false,
        }))
    }

    /// Await a previously submitted request, registering the current task's waker.
    pub async fn wait(&self, request: &Node) -> Result<Value, Error> {
        self.wait_detailed(request)
            .await
            .map_err(|failure| self.propagate(failure))
    }

    async fn wait_detailed(&self, request: &Node) -> Result<Value, Failure> {
        let registration = Waiting {
            graph: self,
            node: request.clone(),
            waker: Rc::new(RefCell::new(None)),
        };
        self.wakers
            .borrow_mut()
            .entry(request.clone())
            .or_default()
            .push(Rc::downgrade(&registration.waker));
        poll_fn(|cx| {
            *registration.waker.borrow_mut() = Some(cx.waker().clone());
            match self.completed.borrow().get(request).cloned() {
                Some(result) => Poll::Ready(result),
                None => Poll::Pending,
            }
        })
        .await
    }

    pub fn complete(&self, request: &Node, result: &Value) -> Result<(), Error> {
        self.complete_result(request, &Ok(result.clone()))
    }

    fn complete_result(
        &self,
        request: &Node,
        result: &Result<Value, Failure>,
    ) -> Result<(), Error> {
        if result.is_ok() {
            self.failures.borrow_mut().remove(request);
        }
        let wakers = self
            .wakers
            .borrow()
            .get(request)
            .cloned()
            .unwrap_or_default();
        if result.is_ok() || wakers.iter().any(|waker| waker.strong_count() != 0) {
            self.completed
                .borrow_mut()
                .entry(request.clone())
                .or_insert_with(|| result.clone());
        }
        self.nodes.borrow_mut().remove(request);
        if result.is_ok() {
            self.wakers.borrow_mut().remove(request);
        }
        for registration in wakers {
            if let Some(waker) = registration
                .upgrade()
                .and_then(|slot| slot.borrow().clone())
            {
                waker.wake();
            }
        }
        Ok(())
    }

    /// Take a ready operation, acquiring its evaluation ownership.
    pub fn take(&self) -> Result<Option<Node>, Error> {
        while let Some(request) = self.ready.borrow_mut().pop_front() {
            if let Some(state) = self.nodes.borrow_mut().get_mut(&request)
                && *state == READY
            {
                *state = RUNNING;
                return Ok(Some(request));
            }
        }
        Ok(None)
    }

    /// Offer an owned operation for execution.
    pub fn offer(&self, request: Node) -> Result<(), Error> {
        if let Some(state) = self.nodes.borrow_mut().get_mut(&request)
            && *state != READY
        {
            *state = READY;
            self.ready.borrow_mut().push_back(request);
        }
        Ok(())
    }
}

struct Waiting<'a> {
    graph: &'a Graph,
    node: Node,
    waker: Rc<RefCell<Option<Waker>>>,
}
impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        let mut entries = self.graph.wakers.borrow_mut();
        if let Some(wakers) = entries.get_mut(&self.node) {
            let weak = Rc::downgrade(&self.waker);
            wakers.retain(|entry| !entry.ptr_eq(&weak));
            if wakers.is_empty() {
                entries.remove(&self.node);
                let mut completed = self.graph.completed.borrow_mut();
                if completed.get(&self.node).is_some_and(Result::is_err) {
                    completed.remove(&self.node);
                }
            }
        }
    }
}

pub struct Owned<'a> {
    graph: &'a Graph,
    node: Node,
    finished: bool,
}
impl Drop for Owned<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.graph.failures.borrow_mut().remove(&self.node);
            let _ = self.graph.offer(self.node.clone());
        }
    }
}
