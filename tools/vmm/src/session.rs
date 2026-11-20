use std::{
    io::{self, Write},
    os::{
        fd::{AsFd, AsRawFd, OwnedFd, RawFd},
        unix::net::UnixStream,
    },
    sync::{
        Arc, Mutex,
        atomic::{AtomicU16, Ordering},
    },
    thread::JoinHandle,
};
use vmm_sys_util::eventfd::{EFD_NONBLOCK, EventFd};

pub struct Session {
    pub(crate) node_identity: [u8; 16],
    pub(crate) session_id: [u8; 16],
    pub(crate) broker: Option<Arc<crate::service::Broker>>,
    pub(crate) profile: Option<crate::profiling::Profile>,
    pub(crate) streams: crate::stream::Streams,
    status: AtomicU16,
    cancel: EventFd,
    workers: Mutex<Vec<JoinHandle<()>>>,
    callbacks: Mutex<Vec<Box<dyn Fn() + Send + Sync>>>,
    output: Option<Mutex<Vec<u8>>>,
    stdio: Option<UnixStream>,
    diagnostics: Mutex<common::progress::Stderr>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl Session {
    pub fn new(capture: bool) -> Arc<Self> {
        Self::with_stdio(capture, None)
    }
    pub fn with_stdio(capture: bool, stdio: Option<UnixStream>) -> Arc<Self> {
        let mut node_identity = [0; 16];
        let mut remaining = node_identity.as_mut_slice();
        while !remaining.is_empty() {
            let count =
                unsafe { libc::getrandom(remaining.as_mut_ptr().cast(), remaining.len(), 0) };
            if count < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                panic!("cannot allocate a runtime identity: {error}");
            }
            assert!(count > 0);
            remaining = &mut remaining[count as usize..];
        }
        Arc::new(Self {
            node_identity,
            session_id: crate::service::identity().expect("session identity"),
            broker: None,
            streams: crate::stream::Streams::default(),
            profile: crate::profiling::Profile::start(),
            status: AtomicU16::new(256),
            cancel: EventFd::new(EFD_NONBLOCK).expect("cancellation eventfd"),
            workers: Mutex::new(Vec::new()),
            callbacks: Mutex::new(Vec::new()),
            output: capture.then(|| Mutex::new(Vec::new())),
            stdio,
            diagnostics: Mutex::new(common::progress::Stderr::default()),
        })
    }
    pub(crate) fn with_broker(broker: Arc<crate::service::Broker>) -> Arc<Self> {
        let mut session = Self::new(true);
        let inner = Arc::get_mut(&mut session).unwrap();
        inner.session_id = broker.settings.id;
        inner.broker = Some(broker);
        session
    }
    pub fn write_diagnostic(&self, bytes: &[u8]) {
        if let Some(broker) = &self.broker {
            broker.diagnostic(bytes);
            return;
        }
        let _ = self.diagnostics.lock().unwrap().write_all(bytes);
    }

    pub fn has_stdio(&self) -> bool {
        self.stdio.is_some()
    }
    pub fn stdio(&self) -> Option<io::Result<OwnedFd>> {
        self.stdio
            .as_ref()
            .map(|stream| stream.as_fd().try_clone_to_owned())
    }
    pub fn wait_for_stop(&self, timeout: std::time::Duration) {
        let mut descriptor = libc::pollfd {
            fd: self.cancel.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        unsafe {
            libc::poll(
                &mut descriptor,
                1,
                timeout.as_millis().min(i32::MAX as u128) as i32,
            );
        }
    }

    pub fn stopped(&self) -> bool {
        self.status.load(Ordering::Acquire) != 256
    }
    pub fn status(&self) -> u8 {
        self.status.load(Ordering::Acquire).try_into().unwrap_or(1)
    }
    pub fn stop(&self, status: u8) {
        if self
            .status
            .compare_exchange(256, u16::from(status), Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.cancel.write(1).expect("signal cancellation");
            self.streams.close_all();
            if let Some(broker) = &self.broker {
                broker.stop();
            }
            if let Some(stream) = &self.stdio {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
            for callback in self.callbacks.lock().unwrap().drain(..) {
                callback();
            }
        }
    }
    pub fn on_stop(&self, callback: impl Fn() + Send + Sync + 'static) {
        let mut callbacks = self.callbacks.lock().unwrap();
        if self.stopped() {
            callback();
        } else {
            callbacks.push(Box::new(callback));
        }
    }
    pub fn spawn(self: &Arc<Self>, f: impl FnOnce() + Send + 'static) {
        let mut workers = self.workers.lock().unwrap();
        if self.stopped() {
            return;
        }
        let mut index = 0;
        while index < workers.len() {
            if workers[index].is_finished() {
                let _ = workers.swap_remove(index).join();
            } else {
                index += 1;
            }
        }
        let session = self.clone();
        let allocator = common::buddy::current_allocator();
        workers.push(std::thread::spawn(move || unsafe {
            common::buddy::with_allocator(&allocator, || {
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
                    session.stop(1);
                }
            })
        }));
    }
    pub fn join(&self) {
        loop {
            let workers = std::mem::take(&mut *self.workers.lock().unwrap());
            if workers.is_empty() {
                break;
            }
            for worker in workers {
                let _ = worker.join();
            }
        }
        let _ = self.diagnostics.lock().unwrap().flush();
    }
    pub fn captures_output(&self) -> bool {
        self.output.is_some()
    }
    pub fn write_output(&self, bytes: &[u8]) {
        self.output
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .extend_from_slice(bytes);
    }
    pub fn output(&self) -> Vec<u8> {
        self.output
            .as_ref()
            .map(|output| std::mem::take(&mut *output.lock().unwrap()))
            .unwrap_or_default()
    }
    pub fn wait(&self, fd: RawFd, events: i16) -> io::Result<()> {
        self.wait_with_cancel(fd, events, self.cancel.as_raw_fd())
    }
    pub(crate) fn wait_with_cancel(&self, fd: RawFd, events: i16, cancel: RawFd) -> io::Result<()> {
        loop {
            if self.stopped() {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let mut fds = [
                libc::pollfd {
                    fd,
                    events,
                    revents: 0,
                },
                libc::pollfd {
                    fd: self.cancel.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: cancel,
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            let result = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, -1) };
            if result < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if fds[1].revents != 0 || fds[2].revents != 0 || self.stopped() {
                return Err(io::ErrorKind::Interrupted.into());
            }
            if fds[0].revents != 0 {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn stopping_wakes_all_waiters_and_joins_workers() {
        let session = Session::new(false);
        let fd = Arc::new(EventFd::new(0).unwrap());
        let done = Arc::new(AtomicUsize::new(0));
        let callbacks = Arc::new(AtomicUsize::new(0));
        let callback = callbacks.clone();
        session.on_stop(move || {
            callback.fetch_add(1, Ordering::Relaxed);
        });
        for _ in 0..8 {
            let state = session.clone();
            let fd = fd.clone();
            let done = done.clone();
            session.spawn(move || {
                assert_eq!(
                    state.wait(fd.as_raw_fd(), libc::POLLIN).unwrap_err().kind(),
                    io::ErrorKind::Interrupted
                );
                done.fetch_add(1, Ordering::Relaxed);
            });
        }
        session.stop(7);
        session.stop(9);
        session.join();
        assert_eq!(done.load(Ordering::Relaxed), 8);
        assert_eq!(callbacks.load(Ordering::Relaxed), 1);
        assert_eq!(session.status(), 7);
    }
}
