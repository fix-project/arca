//! Asynchronous effects with completion-driven Rust wakers.
use crate::{
    abi::{coordinate, request},
    shell::{Value, as_tree, capture, fields},
};
use alloc::{boxed::Box, collections::BTreeMap, rc::Rc, vec::Vec};
use core::{
    cell::{Cell, RefCell},
    future::Future,
    future::poll_fn,
    pin::Pin,
    task::{Poll, Waker},
};
use fixwasm::{Blob, Error, Value as AsValue, tree};

/// Access this scheduler instance’s completion registrations.
#[derive(Default)]
pub struct Io;

pub struct Notification(crate::kernel::worker::Notification);
impl Notification {
    pub fn wait(self) -> Result<(), Error> {
        let ready = BACKGROUND
            .0
            .borrow()
            .iter()
            .any(|(active, task)| active.get() && task.is_ready());
        if !ready {
            self.0.wait()?;
        }
        Ok(())
    }
}
struct Registration {
    waker: Option<Waker>,
    result: Option<Value>,
}
struct Registrations(RefCell<BTreeMap<u64, Registration>>);
// A scheduler Wasm instance is accessed by one kernel thread.
unsafe impl Sync for Registrations {}
static REGISTRATIONS: Registrations = Registrations(RefCell::new(BTreeMap::new()));

type BackgroundFuture = Pin<Box<dyn Future<Output = ()>>>;
type BackgroundTask = (
    Rc<Cell<bool>>,
    crate::executor::task::Task<BackgroundFuture>,
);
struct BackgroundTasks(RefCell<Vec<BackgroundTask>>);
// One kernel thread polls this scheduler instance.
unsafe impl Sync for BackgroundTasks {}
static BACKGROUND: BackgroundTasks = BackgroundTasks(RefCell::new(Vec::new()));

pub(crate) struct Background(Rc<Cell<bool>>);
impl Drop for Background {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

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
    pub(crate) fn spawn(&self, future: impl Future<Output = ()> + 'static) -> Background {
        let active = Rc::new(Cell::new(true));
        BACKGROUND.0.borrow_mut().push((
            active.clone(),
            crate::executor::task::Task::new(Box::pin(future)),
        ));
        Background(active)
    }

    fn poll_background(&self) {
        BACKGROUND
            .0
            .borrow_mut()
            .retain_mut(|(active, task)| active.get() && task.poll().is_pending());
    }

    /// Wake only the futures whose kernel effects have completed.
    pub fn wake_completed(&self) -> Result<(), Error> {
        crate::storage::clear_identified();
        let (status, completed) = coordinate(b"effect_ready", &[])?;
        if status != 0 {
            return Err(Error::WrongType);
        }
        self.complete(&completed)?;
        self.poll_background();
        Ok(())
    }

    /// Listen for new events and deliver completed effects before polling futures.
    pub fn listen(&self) -> Result<Notification, Error> {
        crate::storage::clear_identified();
        let (status, events) = coordinate(b"worker_events", &[])?;
        if status != 0 {
            return Err(Error::WrongType);
        }
        let events = as_tree(&events)?;
        let epoch = events.get(0).read_u64()?;
        self.complete(&capture(events.get(1))?)?;
        self.poll_background();
        Ok(Notification(crate::kernel::worker::Notification(epoch)))
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
