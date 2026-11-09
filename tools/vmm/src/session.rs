use std::{
    io,
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
    pub(crate) profile: Option<crate::profiling::Profile>,
    status: AtomicU16,
    cancel: EventFd,
    workers: Mutex<Vec<JoinHandle<()>>>,
    callbacks: Mutex<Vec<Box<dyn Fn() + Send + Sync>>>,
    output: Option<Mutex<Vec<u8>>>,
    stdio: Option<UnixStream>,
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
        Arc::new(Self {
            profile: crate::profiling::Profile::start(),
            status: AtomicU16::new(256),
            cancel: EventFd::new(EFD_NONBLOCK).expect("cancellation eventfd"),
            workers: Mutex::new(Vec::new()),
            callbacks: Mutex::new(Vec::new()),
            output: capture.then(|| Mutex::new(Vec::new())),
            stdio,
        })
    }
    pub fn has_stdio(&self) -> bool {
        self.stdio.is_some()
    }
    pub fn stdio(&self) -> Option<io::Result<OwnedFd>> {
        self.stdio
            .as_ref()
            .map(|stream| stream.as_fd().try_clone_to_owned())
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
            ];
            let result = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, -1) };
            if result < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if fds[1].revents != 0 || self.stopped() {
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
