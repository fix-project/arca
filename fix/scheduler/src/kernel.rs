use crate::abi::{coordinate, coordinated, request};
use crate::shell::{Root, as_tree, child, root};
use fixwasm::{Blob, Error, Value, tree};
pub use fixwasm::{map, stream};

pub struct Advice {
    pub target: Root,
    pub key: Root,
    pub payload: Root,
}

impl Advice {
    pub fn key_is(&self, key: &str) -> Result<bool, Error> {
        self.key.equals(Blob::create(key.as_bytes()))
    }
}

pub struct Continuation(Root);

pub enum Execution {
    Complete(Root),
    Advice {
        advice: Advice,
        continuation: Continuation,
    },
}

impl Execution {
    fn read(reply: (u64, Root)) -> Result<Self, Error> {
        match reply {
            (0, value) => Ok(Self::Complete(value)),
            (2, event) => {
                if as_tree(&event)?.len()? != 4 {
                    return Err(Error::WrongType);
                }
                Ok(Self::Advice {
                    advice: Advice {
                        target: child(&event, 1)?,
                        key: child(&event, 2)?,
                        payload: child(&event, 3)?,
                    },
                    continuation: Continuation(child(&event, 0)?),
                })
            }
            _ => Err(Error::WrongType),
        }
    }
}

pub fn apply(combination: &Root) -> Result<Execution, Error> {
    Execution::read(request(tree![
        Blob::create(b"callable_apply"),
        combination
    ])?)
}

pub fn invoke(combination: &Root) -> Result<Execution, Error> {
    Execution::read(coordinate(
        b"callable_invoke",
        core::slice::from_ref(combination),
    )?)
}

impl Continuation {
    pub fn resume(self) -> Result<Execution, Error> {
        Execution::read(coordinate(b"callable_resume", &[self.0])?)
    }
}

pub mod worker {
    use super::*;

    pub struct Notification(u64);

    pub fn listen() -> Result<Notification, Error> {
        Ok(Notification(
            coordinated(b"worker_listen", &[])?.read_u64()?,
        ))
    }

    impl Notification {
        pub fn wait(self) -> Result<(), Error> {
            coordinated(b"worker_wait", &[root(self.0)?])?;
            Ok(())
        }
    }

    pub fn wake() -> Result<(), Error> {
        coordinated(b"worker_wake", &[])?;
        Ok(())
    }

    pub fn wake_one() -> Result<(), Error> {
        coordinated(b"worker_wake_one", &[])?;
        Ok(())
    }

    fn config(field: u64) -> Result<u64, Error> {
        coordinated(b"worker_config", &[root(field)?])?.read_u64()
    }

    pub fn id() -> Result<u64, Error> {
        config(0)
    }

    pub fn count() -> Result<u64, Error> {
        config(1)
    }

    pub fn returns_coupon() -> Result<bool, Error> {
        Ok(config(3)? != 0)
    }

    pub fn stopped() -> Result<bool, Error> {
        Ok(config(2)? != 0)
    }

    pub fn yield_now() -> Result<(), Error> {
        coordinated(b"worker_yield", &[])?;
        Ok(())
    }

    pub fn finish(value: &Root) -> Result<(), Error> {
        coordinated(b"worker_finish", core::slice::from_ref(value))?;
        Ok(())
    }
}

pub mod queue {
    use super::*;

    pub fn count() -> Result<u64, Error> {
        coordinated(b"queue_count", &[])?.read_u64()
    }

    pub struct Queue(u64);

    impl Queue {
        pub fn open(index: u64) -> Self {
            Self(index)
        }

        pub fn create(capacity: u64) -> Result<Self, Error> {
            Ok(Self(
                coordinated(b"queue_create", &[root(capacity)?])?.read_u64()?,
            ))
        }

        pub fn pop(&self) -> Result<Option<Root>, Error> {
            let (status, value) = tree![self.0].coordinate(b"queue_pop")?;
            match status {
                0 => Ok(Some(alloc::rc::Rc::new(value))),
                1 => Ok(None),
                _ => panic!("queue pop failed"),
            }
        }

        pub fn push(&self, value: &Root) -> Result<bool, Error> {
            let (status, _) = tree![self.0, value].coordinate(b"queue_push")?;
            match status {
                0 => Ok(true),
                2 => Ok(false),
                _ => panic!("queue push failed"),
            }
        }
    }
}
