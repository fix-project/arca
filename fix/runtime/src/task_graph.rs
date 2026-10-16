// this file keeps track of task graph that all instances share that updates
// dependencies 
use crate::handle::*;
use crate::runtime::Runtime;
use crate::storage::Storage;
use kernel::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    LocalResult,
}

pub struct TaskGraph<H: WorkHandle = Handle> {
    dependencies: Hashmap<>,
    dependents: Hashmap<>,
    results: Hashmap<>,
}

impl<H: WorkHandle> TaskGraph<H> {
    pub fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
            results: HashMap::new(),
        }
    }
    // add all other functions
}

impl<H: WorkHandle> Default for TaskGraph<H> {
    fn default() -> Self {
        Self::new()
    }
}

