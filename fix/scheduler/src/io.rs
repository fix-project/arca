//! Asynchronous effects with completion-driven Rust wakers.
use crate::{
    abi::{coordinate, request},
    shell::{Value, as_tree, capture, fields},
};
use alloc::collections::BTreeMap;
use core::{
    cell::RefCell,
    future::poll_fn,
    task::{Poll, Waker},
};
use fixwasm::{Blob, Error, Value as AsValue, tree};

/// Access this scheduler instance’s completion registrations.
#[derive(Default)]
pub struct Io;
struct Registration {
    waker: Option<Waker>,
    result: Option<Value>,
}
struct Registrations(RefCell<BTreeMap<u64, Registration>>);
// A scheduler Wasm instance is accessed by one kernel thread.
unsafe impl Sync for Registrations {}
static REGISTRATIONS: Registrations = Registrations(RefCell::new(BTreeMap::new()));

struct Pending {
    token: u64,
    consumed: bool,
    application: bool,
}
impl Drop for Pending {
    fn drop(&mut self) {
        let registration = REGISTRATIONS.0.borrow_mut().remove(&self.token);
        if !self.consumed {
            if let Some(result) = registration.and_then(|registration| registration.result) {
                if self.application {
                    let _ = (|| -> Result<(), Error> {
                        let reply = as_tree(&result)?;
                        if reply.get(0).read_u64()? == 2 {
                            let event = as_tree(&capture(reply.get(1))?)?;
                            coordinate(b"callable_discard", &[capture(event.get(0))?])?;
                        }
                        Ok(())
                    })();
                }
            } else {
                let _ = coordinate(b"effect_discard", &[capture(self.token).unwrap()]);
            }
        }
    }
}

impl Io {
    /// Wake only the futures whose kernel effects have completed.
    pub fn wake_completed(&self) -> Result<(), Error> {
        crate::storage::clear_identified();
        let (status, completed) = coordinate(b"effect_ready", &[])?;
        if status != 0 {
            return Err(Error::WrongType);
        }
        self.complete(&completed)
    }

    /// Listen for new events and deliver completed effects before polling futures.
    pub fn listen(&self) -> Result<crate::kernel::worker::Notification, Error> {
        crate::storage::clear_identified();
        let (status, events) = coordinate(b"worker_events", &[])?;
        if status != 0 {
            return Err(Error::WrongType);
        }
        let events = as_tree(&events)?;
        let epoch = events.get(0).read_u64()?;
        self.complete(&capture(events.get(1))?)?;
        Ok(crate::kernel::worker::Notification(epoch))
    }

    fn complete(&self, completed: &Value) -> Result<(), Error> {
        for completion in fields(completed)? {
            let completion = as_tree(&completion)?;
            let token = completion.get(0).read_u64()?;
            let result = capture(completion.get(1))?;
            let waker = {
                let mut registrations = REGISTRATIONS.0.borrow_mut();
                let registration = registrations.get_mut(&token).ok_or(Error::WrongType)?;
                registration.result = Some(result);
                registration.waker.take()
            };
            if let Some(waker) = waker {
                waker.wake();
            }
        }
        Ok(())
    }

    /// Submit an effect to a kernel thread and await its result.
    /// The event loop must call `listen` or drain `wake_completed` to deliver results.
    pub async fn request(&self, effect: impl AsValue) -> Result<(u64, Value), Error> {
        self.start(b"effect_start", effect).await
    }

    pub(crate) async fn application(&self, effect: impl AsValue) -> Result<(u64, Value), Error> {
        self.start(b"application_start", effect).await
    }

    async fn start(&self, operation: &[u8], effect: impl AsValue) -> Result<(u64, Value), Error> {
        let (status, token) = if operation == b"application_start" {
            coordinate(operation, &[capture(effect)?])?
        } else {
            request(tree![Blob::create(operation), effect])?
        };
        if status != 0 {
            return Err(Error::WrongType);
        }
        let mut pending = Pending {
            token: token.read_u64()?,
            consumed: false,
            application: operation == b"application_start",
        };
        REGISTRATIONS.0.borrow_mut().insert(
            pending.token,
            Registration {
                waker: None,
                result: None,
            },
        );
        poll_fn(|cx| {
            let mut registrations = REGISTRATIONS.0.borrow_mut();
            let registration = registrations.get_mut(&pending.token).unwrap();
            if let Some(value) = registration.result.take() {
                pending.consumed = true;
                Poll::Ready((|| {
                    let reply = as_tree(&value)?;
                    Ok((reply.get(0).read_u64()?, capture(reply.get(1))?))
                })())
            } else {
                registration.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await
    }
}

/// Write a diagnostic to the host's kernel log.
pub fn log(message: &str) -> Result<(), Error> {
    request(tree![Blob::create(b"log"), Blob::create(message)])?;
    Ok(())
}
