use crate::abi::{coordinate, coordinated};
pub use crate::progress::cached;
use crate::shell::{Value, as_tree, capture, child};
use fixwasm::{Blob, Error, Value as AsValue, tree};
pub use fixwasm::{map, stream};

pub struct Advice {
    pub target: Value,
    pub key: Value,
    pub payload: Value,
}

pub fn tree_progress(
    task: &Value,
    children: &[Value],
    completed: u64,
    total: u64,
) -> Result<(), Error> {
    coordinated(
        b"tree_progress",
        &[
            task.clone(),
            capture(fixwasm::Tree::from_iter(children))?,
            capture(completed)?,
            capture(total)?,
        ],
    )?;
    Ok(())
}

impl Advice {
    pub fn key_is(&self, key: &str) -> Result<bool, Error> {
        AsValue::equals(&self.key, Blob::create(key.as_bytes()))
    }
}

/// Run an application and return its Apply coupon, delivering advice without a guest reply.
pub async fn apply(
    combination: &Value,
    on_advice: &mut dyn FnMut(&Advice) -> Result<(), Error>,
) -> Result<Value, Error> {
    let evaluation = crate::progress::operation(2, combination);
    let result =
        async { observed(admitted(b"callable_apply", combination).await?, on_advice).await }.await;
    evaluation.finish(result)
}

/// Run an ordinary application and return its value without creating a coupon.
pub async fn run(
    combination: &Value,
    on_advice: &mut dyn FnMut(&Advice) -> Result<(), Error>,
) -> Result<Value, Error> {
    let evaluation = crate::progress::operation(2, combination);
    let result =
        async { observed(admitted(b"callable_run", combination).await?, on_advice).await }.await;
    evaluation.finish(result)
}

async fn admitted(operation: &[u8], combination: &Value) -> Result<(u64, Value), Error> {
    loop {
        let reply = crate::io::Io
            .application(tree![Blob::create(operation), combination])
            .await?;
        if reply.0 != 6 {
            return Ok(reply);
        }
        let required = child(&reply.1, 0)?;
        let (status, ready) = crate::io::Io
            .request(tree![Blob::create(b"memory_wait"), required])
            .await?;
        if status != 0 || ready.read_u64()? == 0 {
            crate::io::log(&alloc::format!(
                "memory admission rejected: requested {} bytes, {} bytes available",
                required.read_u64()?,
                worker::available_memory()?,
            ))?;
            return Err(Error::MemoryUnavailable);
        }
    }
}

/// Invoke a privileged callable with the same advisory notification handler.
pub async fn invoke(
    combination: &Value,
    on_advice: &mut dyn FnMut(&Advice) -> Result<(), Error>,
) -> Result<Value, Error> {
    observed(
        crate::io::Io
            .application(tree![Blob::create(b"callable_invoke"), combination])
            .await?,
        on_advice,
    )
    .await
}

async fn observed(
    mut reply: (u64, Value),
    on_advice: &mut dyn FnMut(&Advice) -> Result<(), Error>,
) -> Result<Value, Error> {
    loop {
        match reply {
            (6, _) => return Err(Error::MemoryUnavailable),
            (0, value) => return Ok(value),
            (2, event) => {
                if as_tree(&event)?.len()? != 4 {
                    return Err(Error::WrongType);
                }
                let mut invocation = AdviceReturn(Some(child(&event, 0)?));
                on_advice(&Advice {
                    target: child(&event, 1)?,
                    key: child(&event, 2)?,
                    payload: child(&event, 3)?,
                })?;
                reply = crate::io::Io
                    .application(tree![
                        Blob::create(b"callable_resume"),
                        invocation.0.take().unwrap()
                    ])
                    .await?;
            }
            _ => return Err(Error::WrongType),
        }
    }
}

// This token names a native invocation, not a captured Fix continuation.
struct AdviceReturn(Option<Value>);

impl Drop for AdviceReturn {
    fn drop(&mut self) {
        if let Some(token) = self.0.take() {
            let _ = coordinate(b"callable_discard", &[token]);
        }
    }
}

pub mod worker {
    use super::*;

    pub struct Notification(pub(crate) u64);

    pub fn listen() -> Result<Notification, Error> {
        Ok(Notification(
            coordinated(b"worker_listen", &[])?.read_u64()?,
        ))
    }

    impl Notification {
        pub fn wait(self) -> Result<(), Error> {
            coordinated(b"worker_wait", &[capture(self.0)?])?;
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
        coordinated(b"worker_config", &[capture(field)?])?.read_u64()
    }

    pub fn id() -> Result<u64, Error> {
        config(0)
    }

    pub fn count() -> Result<u64, Error> {
        config(1)
    }

    pub fn available_memory() -> Result<u64, Error> {
        config(4)
    }

    pub fn memory_capacity() -> Result<u64, Error> {
        config(5)
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

    pub fn finish(value: &Value) -> Result<(), Error> {
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
        pub fn id(&self) -> u64 {
            self.0
        }

        pub fn open(index: u64) -> Self {
            Self(index)
        }

        /// Open or create a queue shared by name within this scheduler run.
        /// Reopening a queue requires the same capacity.
        pub fn named(name: &[u8], capacity: u64) -> Result<Self, Error> {
            Ok(Self(
                coordinated(
                    b"queue_named",
                    &[capture(fixwasm::Blob::create(name))?, capture(capacity)?],
                )?
                .read_u64()?,
            ))
        }

        pub fn create(capacity: u64) -> Result<Self, Error> {
            Ok(Self(
                coordinated(b"queue_create", &[capture(capacity)?])?.read_u64()?,
            ))
        }

        pub fn pop(&self) -> Result<Option<Value>, Error> {
            let (status, value) = tree![self.0].coordinate(b"queue_pop")?;
            match status {
                0 => Ok(Some(Value::from_inner(value))),
                1 => Ok(None),
                _ => panic!("queue pop failed"),
            }
        }

        pub fn push(&self, value: &Value) -> Result<bool, Error> {
            let (status, _) = tree![self.0, value].coordinate(b"queue_push")?;
            match status {
                0 => Ok(true),
                2 => Ok(false),
                _ => panic!("queue push failed"),
            }
        }
    }
}
