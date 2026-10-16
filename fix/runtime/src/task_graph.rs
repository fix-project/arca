// this file keeps track of task graph that all instances share that updates
// dependencies 
use crate::handle::*;
use crate::runtime::Runtime;
use crate::storage::Storage;
use kernel::prelude::*;

extern crate alloc; 
use crate::computation::Computation;
use alloc::vec::Vec;
use fixhandle::{Handle, Name};
use hashbrown::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    LocalResult,
}

pub struct TaskGraph {
    dependencies: HashMap<Computation, HashSet<Computation>>,
    dependents: HashMap<Computation, HashSet<Computation>>,
    results: HashMap<Computation, Handle>,
}

impl TaskGraph {
    pub fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
            results: HashMap::new(),
        }
    }

    pub fn wait_for(&mut self,consumer: Computation,inputs: &[Computation],) -> usize {
        self.clear_dependencies(consumer);

        let missing: HashSet<Computation> = inputs
            .iter()
            .copied()
            .filter(|input| !self.results.contains_key(input))
            .collect();

        let count = missing.len();

        for input in &missing {
            self.dependents
                .entry(*input)
                .or_default()
                .insert(consumer);
        }

        if count != 0 { self.dependencies.insert(consumer, missing);}
        count
    }

    /// Stores an observation and returns newly unblocked computations
    pub fn publish( &mut self, work: Computation, result: Handle) -> Result<Vec<Computation>, Error> {
        if matches!(result.name(), Name::Local { .. }) {
            return Err(Error::LocalResult);
        }

        self.results.insert(work, result);
        let mut unblocked = Vec::new();

        if let Some(consumers) = self.dependents.remove(&work) {
            for consumer in consumers {
                let missing = self.dependencies.get_mut(&consumer).unwrap();
                missing.remove(&work);

                if missing.is_empty() {
                    self.dependencies.remove(&consumer);
                    unblocked.push(consumer);
                }
            }
        }
        Ok(unblocked)
    }

    pub fn result(&self, work: Computation) -> Option<Handle> {
        self.results.get(&work).copied()
    }

    pub fn dependencies(&self, work: Computation) -> Vec<Computation> {
        self.dependencies
            .get(&work)
            .map(|inputs| inputs.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn clear_dependencies(&mut self, work: Computation) {
        if let Some(inputs) = self.dependencies.remove(&work) {
            for input in inputs {
                let consumers = self.dependents.get_mut(&input).unwrap();
                consumers.remove(&work);

                if consumers.is_empty() {
                    self.dependents.remove(&input);
                }
            }
        }
    }

    // maybe we want a function to remove cached results later?

    pub fn retained_results(&self) -> usize {
        self.results.len()
    }
    // add all other functions
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}

