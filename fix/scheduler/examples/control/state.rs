use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};
pub use scheduler_dependencies::{Poll, Slot, Step, Task, TaskError};
pub type Ticket = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    UnknownTask,
    WrongEndpoint,
    UnknownSlot,
    DuplicateSlot,
    AlreadyComplete,
    NotReady,
    EmptyWait,
    Overflow,
}

#[derive(Clone)]
pub enum Command<R, V> {
    Start {
        ticket: Ticket,
        request: R,
    },
    Resume {
        ticket: Ticket,
        values: Vec<(Slot, V)>,
    },
}
impl<R, V> Command<R, V> {
    pub fn ticket(&self) -> Ticket {
        match self {
            Self::Start { ticket, .. } | Self::Resume { ticket, .. } => *ticket,
        }
    }
}

pub enum Event<R, V, A = ()> {
    Blocked {
        ticket: Ticket,
        dependencies: Vec<(Slot, R)>,
    },
    Complete {
        ticket: Ticket,
        value: V,
    },
    Advice {
        ticket: Ticket,
        value: A,
    },
}
impl<R, V, A> Event<R, V, A> {
    pub fn ticket(&self) -> Ticket {
        match self {
            Self::Blocked { ticket, .. }
            | Self::Complete { ticket, .. }
            | Self::Advice { ticket, .. } => *ticket,
        }
    }
}

struct Node<R, V> {
    request: R,
    waiting: Option<BTreeMap<Slot, Option<V>>>,
    dependents: Vec<(Ticket, Slot)>,
    result: Option<V>,
}

pub struct Coordinator<R, K, V> {
    identities: BTreeMap<K, Ticket>,
    nodes: BTreeMap<Ticket, Node<R, V>>,
    next_ticket: Ticket,
}
impl<R, K, V> Default for Coordinator<R, K, V> {
    fn default() -> Self {
        Self {
            identities: BTreeMap::new(),
            nodes: BTreeMap::new(),
            next_ticket: 0,
        }
    }
}
impl<R: Clone, K: Ord, V: Clone> Coordinator<R, K, V> {
    pub fn submit(
        &mut self,
        identity: K,
        request: R,
    ) -> Result<(Ticket, Vec<Command<R, V>>), Error> {
        if let Some(ticket) = self.identities.get(&identity) {
            return Ok((*ticket, Vec::new()));
        }
        let ticket = self.next_ticket;
        self.next_ticket = ticket.checked_add(1).ok_or(Error::Overflow)?;
        self.identities.insert(identity, ticket);
        self.nodes.insert(
            ticket,
            Node {
                request: request.clone(),
                waiting: None,
                dependents: Vec::new(),
                result: None,
            },
        );
        Ok((ticket, alloc::vec![Command::Start { ticket, request }]))
    }

    pub fn request(&self, ticket: Ticket) -> Result<&R, Error> {
        Ok(&self.nodes.get(&ticket).ok_or(Error::UnknownTask)?.request)
    }
    pub fn result(&self, ticket: Ticket) -> Option<&V> {
        self.nodes.get(&ticket)?.result.as_ref()
    }

    pub fn blocked(
        &mut self,
        ticket: Ticket,
        dependencies: Vec<(Slot, K, R)>,
    ) -> Result<Vec<Command<R, V>>, Error> {
        let node = self.nodes.get(&ticket).ok_or(Error::UnknownTask)?;
        if node.result.is_some() {
            return Err(Error::AlreadyComplete);
        }
        if node.waiting.is_some() {
            return Err(Error::NotReady);
        }
        if dependencies.is_empty() {
            return Err(Error::EmptyWait);
        }
        let slots: BTreeSet<_> = dependencies.iter().map(|(slot, _, _)| *slot).collect();
        if slots.len() != dependencies.len() {
            return Err(Error::DuplicateSlot);
        }
        let mut ready = Vec::new();
        let mut waiting = BTreeMap::new();
        for (slot, key, request) in dependencies {
            let (dependency, mut work) = self.submit(key, request)?;
            ready.append(&mut work);
            let node = self.nodes.get_mut(&dependency).unwrap();
            let value = node.result.clone();
            if value.is_none() {
                node.dependents.push((ticket, slot));
            }
            waiting.insert(slot, value);
        }
        self.nodes.get_mut(&ticket).unwrap().waiting = Some(waiting);
        self.ready(ticket, &mut ready)?;
        Ok(ready)
    }

    pub fn complete(&mut self, ticket: Ticket, value: V) -> Result<Vec<Command<R, V>>, Error> {
        let node = self.nodes.get_mut(&ticket).ok_or(Error::UnknownTask)?;
        if node.result.is_some() {
            return Err(Error::AlreadyComplete);
        }
        if node.waiting.is_some() {
            return Err(Error::NotReady);
        }
        node.result = Some(value.clone());
        let dependents = core::mem::take(&mut node.dependents);
        let mut ready = Vec::new();
        for (parent, slot) in dependents {
            let waiting = self
                .nodes
                .get_mut(&parent)
                .unwrap()
                .waiting
                .as_mut()
                .ok_or(Error::NotReady)?;
            *waiting.get_mut(&slot).ok_or(Error::UnknownSlot)? = Some(value.clone());
            self.ready(parent, &mut ready)?;
        }
        Ok(ready)
    }

    fn ready(&mut self, ticket: Ticket, ready: &mut Vec<Command<R, V>>) -> Result<(), Error> {
        let node = self.nodes.get_mut(&ticket).ok_or(Error::UnknownTask)?;
        if node
            .waiting
            .as_ref()
            .is_some_and(|values| values.values().all(Option::is_some))
        {
            let values = node
                .waiting
                .take()
                .unwrap()
                .into_iter()
                .map(|(slot, value)| (slot, value.unwrap()))
                .collect();
            ready.push(Command::Resume { ticket, values });
        }
        Ok(())
    }
}

pub struct Ownership<P> {
    owners: BTreeMap<Ticket, P>,
}
impl<P> Default for Ownership<P> {
    fn default() -> Self {
        Self {
            owners: BTreeMap::new(),
        }
    }
}

impl<P: Eq> Ownership<P> {
    pub fn assign(&mut self, ticket: Ticket, peer: P) -> bool {
        if self.owners.contains_key(&ticket) {
            return false;
        }
        self.owners.insert(ticket, peer);
        true
    }
    pub fn owner(&self, ticket: Ticket) -> Option<&P> {
        self.owners.get(&ticket)
    }
    pub fn accepts(&self, ticket: Ticket, peer: &P) -> bool {
        self.owner(ticket) == Some(peer)
    }
    pub fn release(&mut self, ticket: Ticket, peer: &P) -> bool {
        if !self.accepts(ticket, peer) {
            return false;
        }
        self.owners.remove(&ticket);
        true
    }
}

pub trait Transport<P, C, E> {
    type Error;
    fn send(&mut self, destination: &P, command: C) -> Result<(), Self::Error>;
    fn receive(&mut self) -> Result<Option<(P, E)>, Self::Error>;
}
