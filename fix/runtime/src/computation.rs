// new data structure to keep track of handle as well as what to do with it
// used in task graph and probably the queues when we call on it
use core::hash::{Hash, Hasher};
use fixhandle::{Handle, Name, Thunk, Tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operation {
    Evaluate,
    Force,
    Apply,
    Select,
}

#[derive(Clone, Copy, Debug)]
pub struct Computation {
    operation: Operation,
    input: Handle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    LocalHandle,
    WrongType,
}

impl Computation {
    pub fn new(operation: Operation, input: Handle) -> Result<Self, Error> {
        if matches!(input.name(), Name::Local { .. }) {
            return Err(Error::LocalHandle);
        }

        match operation {
            Operation::Force if Thunk::try_from(input).is_err() => {
                return Err(Error::WrongType);
            }
            Operation::Apply | Operation::Select if Tree::try_from(input).is_err() => {
                return Err(Error::WrongType);
            }
            _ => (),
        }

        Ok(Self { operation, input })
    }

    pub fn operation(self) -> Operation {
        self.operation
    }

    pub fn input(self) -> Handle {
        self.input
    }
}

impl PartialEq for Computation {
    fn eq(&self, other: &Self) -> bool {
        self.operation == other.operation && self.input.same_encoding(&other.input)
    }
}
impl Eq for Computation {}

impl Hash for Computation {
    fn hash<S: Hasher>(&self, state: &mut S) {
        self.operation.hash(state);
        self.input.as_bytes().hash(state);
    }
}
