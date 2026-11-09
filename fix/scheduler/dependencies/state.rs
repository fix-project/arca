use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};

pub type Slot = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    UnknownSlot,
    DuplicateSlot,
    AlreadyComplete,
    NotReady,
    EmptyWait,
    Overflow,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TaskError<E> {
    Evaluation(E),
    State(Error),
}

pub enum Poll<K, R, V> {
    Complete(V),
    Blocked(Vec<(K, R)>),
}

pub enum Step<R, V> {
    Complete(V),
    Blocked(Vec<(Slot, R)>),
}

pub struct Task<R, K, V, S> {
    request: R,
    state: S,
    resolved: BTreeMap<K, V>,
    waiting: BTreeMap<Slot, K>,
    next_slot: Slot,
    complete: bool,
}

impl<R, K: Ord, V, S: Default> Task<R, K, V, S> {
    pub fn new(request: R) -> Self {
        Self {
            request,
            state: S::default(),
            resolved: BTreeMap::new(),
            waiting: BTreeMap::new(),
            next_slot: 0,
            complete: false,
        }
    }
}

impl<R, K: Ord + Clone, V, S> Task<R, K, V, S> {
    pub fn request(&self) -> &R {
        &self.request
    }

    pub fn poll<E>(
        &mut self,
        evaluate: impl FnOnce(&R, &mut S, &BTreeMap<K, V>) -> Result<Poll<K, R, V>, E>,
    ) -> Result<Step<R, V>, TaskError<E>> {
        if self.complete {
            return Err(TaskError::State(Error::AlreadyComplete));
        }
        if !self.waiting.is_empty() {
            return Err(TaskError::State(Error::NotReady));
        }
        match evaluate(&self.request, &mut self.state, &self.resolved)
            .map_err(TaskError::Evaluation)?
        {
            Poll::Complete(value) => {
                self.complete = true;
                Ok(Step::Complete(value))
            }
            Poll::Blocked(dependencies) => {
                let dependencies: BTreeMap<_, _> = dependencies.into_iter().collect();
                if dependencies.is_empty() {
                    return Err(TaskError::State(Error::EmptyWait));
                }
                let Some(end) = self.next_slot.checked_add(dependencies.len() as u64) else {
                    return Err(TaskError::State(Error::Overflow));
                };
                let mut missing = Vec::new();
                for (slot, (key, request)) in (self.next_slot..end).zip(dependencies) {
                    self.waiting.insert(slot, key);
                    missing.push((slot, request));
                }
                self.next_slot = end;
                Ok(Step::Blocked(missing))
            }
        }
    }

    pub fn resume(&mut self, values: Vec<(Slot, V)>) -> Result<(), Error> {
        let slots: BTreeSet<_> = values.iter().map(|(slot, _)| *slot).collect();
        if slots.len() != values.len() {
            return Err(Error::DuplicateSlot);
        }
        if slots.len() != self.waiting.len()
            || slots.iter().any(|slot| !self.waiting.contains_key(slot))
        {
            return Err(Error::UnknownSlot);
        }
        for (slot, value) in values {
            let key = self.waiting.remove(&slot).unwrap();
            self.resolved.insert(key, value);
        }
        Ok(())
    }
}
