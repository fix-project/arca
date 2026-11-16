pub use super::node::Node;
use crate::shell::Value;
use alloc::{
    boxed::Box,
    collections::{BTreeMap, VecDeque},
    rc::{Rc, Weak},
    vec::Vec,
};
use core::cell::RefCell;
use core::{
    future::{Future, poll_fn},
    task::{Poll, Waker},
};
use fixwasm::Error;

const RUNNING: u64 = 1;
const READY: u64 = 2;

pub enum Claim {
    Owned,
    Done(Value),
    Busy,
}

type WakerSlot = RefCell<Option<Waker>>;

#[derive(Default)]
pub struct Graph {
    nodes: RefCell<BTreeMap<Node, u64>>,
    ready: RefCell<VecDeque<Node>>,
    completed: RefCell<BTreeMap<Node, Result<Value, Error>>>,
    current: RefCell<Option<Node>>,
    executing: RefCell<Option<Node>>,
    wakers: RefCell<BTreeMap<Node, Vec<Weak<WakerSlot>>>>,
}

impl Graph {
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
                self.complete_result(node, &result)?;
                owner.finished = true;
                result
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
        self.submit(node)?;
        self.wait(node).await
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
        self.completed.borrow().get(request).cloned().transpose()
    }

    /// Await a previously submitted request, registering the current task's waker.
    pub async fn wait(&self, request: &Node) -> Result<Value, Error> {
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
            match self.completed(request) {
                Ok(Some(value)) => Poll::Ready(Ok(value)),
                Err(error) => Poll::Ready(Err(error)),
                Ok(None) => Poll::Pending,
            }
        })
        .await
    }

    pub fn complete(&self, request: &Node, result: &Value) -> Result<(), Error> {
        self.complete_result(request, &Ok(result.clone()))
    }

    fn complete_result(&self, request: &Node, result: &Result<Value, Error>) -> Result<(), Error> {
        self.completed
            .borrow_mut()
            .entry(request.clone())
            .or_insert_with(|| result.clone());
        self.nodes.borrow_mut().remove(request);
        let wakers = self.wakers.borrow_mut().remove(request).unwrap_or_default();
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
            if let Some(state) = self.nodes.borrow_mut().get_mut(&request) {
                if *state == READY {
                    *state = RUNNING;
                    return Ok(Some(request));
                }
            }
        }
        Ok(None)
    }

    /// Offer an owned operation for execution.
    pub fn offer(&self, request: Node) -> Result<(), Error> {
        if let Some(state) = self.nodes.borrow_mut().get_mut(&request) {
            if *state != READY {
                *state = READY;
                self.ready.borrow_mut().push_back(request);
            }
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
            }
        }
    }
}

struct Owned<'a> {
    graph: &'a Graph,
    node: Node,
    finished: bool,
}
impl Drop for Owned<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.graph.offer(self.node.clone());
        }
    }
}
