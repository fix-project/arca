use crate::session::Session;
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    os::fd::AsRawFd,
    process::Child,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use vmm_sys_util::eventfd::{EFD_NONBLOCK, EventFd};

#[derive(Default)]
pub(crate) struct Streams {
    next: AtomicU64,
    entries: Mutex<BTreeMap<u64, Arc<Stream>>>,
}

impl Streams {
    pub(crate) fn insert(&self, stream: Stream) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        assert_ne!(id, u64::MAX);
        self.entries.lock().unwrap().insert(id, Arc::new(stream));
        id
    }

    pub(crate) fn get(&self, id: u64) -> io::Result<Arc<Stream>> {
        self.entries
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    pub(crate) fn close(&self, id: u64) {
        let stream = self.entries.lock().unwrap().remove(&id);
        if let Some(stream) = stream {
            stream.close();
        }
    }

    pub(crate) fn close_all(&self) {
        let entries = std::mem::take(&mut *self.entries.lock().unwrap());
        for stream in entries.into_values() {
            stream.close();
        }
    }
}

pub(crate) struct Stream {
    reader: Mutex<Box<dyn Read + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    cancel: Arc<EventFd>,
    child: Mutex<Option<Process>>,
    closed: AtomicBool,
    on_close: Option<Box<dyn Fn() + Send + Sync>>,
}

impl Stream {
    pub(crate) fn tcp(stream: std::net::TcpStream, session: Arc<Session>) -> io::Result<Self> {
        stream.set_nodelay(true)?;
        Self::new(stream.try_clone()?, stream, session, None)
    }

    pub(crate) fn new<R: Read + AsRawFd + Send + 'static, W: Write + AsRawFd + Send + 'static>(
        reader: R,
        writer: W,
        session: Arc<Session>,
        child: Option<Child>,
    ) -> io::Result<Self> {
        let child = child.map(Process);
        let cancel = Arc::new(EventFd::new(EFD_NONBLOCK)?);
        let reader = PollIo::new(reader, session.clone(), cancel.clone())?;
        let writer = PollIo::new(writer, session, cancel.clone())?;
        Ok(Self {
            reader: Mutex::new(Box::new(reader)),
            writer: Mutex::new(Box::new(writer)),
            cancel,
            child: Mutex::new(child),
            closed: AtomicBool::new(false),
            on_close: None,
        })
    }

    pub(crate) fn read_only(reader: Box<dyn Read + Send>) -> io::Result<Self> {
        Ok(Self {
            reader: Mutex::new(reader),
            writer: Mutex::new(Box::new(Unsupported)),
            cancel: Arc::new(EventFd::new(EFD_NONBLOCK)?),
            child: Mutex::new(None),
            closed: AtomicBool::new(false),
            on_close: None,
        })
    }

    pub(crate) fn receive(&self, length: usize) -> io::Result<Vec<u8>> {
        let mut bytes = vec![0; length];
        let length = self.reader.lock().unwrap().read(&mut bytes)?;
        bytes.truncate(length);
        Ok(bytes)
    }

    pub(crate) fn send(&self, bytes: &[u8]) -> io::Result<usize> {
        self.writer.lock().unwrap().write(bytes)
    }

    pub(crate) fn on_close(mut self, callback: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_close = Some(Box::new(callback));
        self
    }

    fn close(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(callback) = &self.on_close {
            callback();
        }
        let _ = self.cancel.write(1);
        drop(self.child.lock().unwrap().take());
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.close();
    }
}

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Unsupported;
impl Write for Unsupported {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct PollIo<T> {
    inner: T,
    session: Arc<Session>,
    cancel: Arc<EventFd>,
}

impl<T: AsRawFd> PollIo<T> {
    fn new(inner: T, session: Arc<Session>, cancel: Arc<EventFd>) -> io::Result<Self> {
        let fd = inner.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            inner,
            session,
            cancel,
        })
    }

    fn wait(&self, events: i16) -> io::Result<()> {
        self.session
            .wait_with_cancel(self.inner.as_raw_fd(), events, self.cancel.as_raw_fd())
    }
}

impl<T: Read + AsRawFd> Read for PollIo<T> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        loop {
            self.wait(libc::POLLIN)?;
            match self.inner.read(bytes) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                    ) => {}
                result => return result,
            }
        }
    }
}

impl<T: Write + AsRawFd> Write for PollIo<T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        loop {
            self.wait(libc::POLLOUT)?;
            match self.inner.write(bytes) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                    ) => {}
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::unix::net::UnixStream, sync::mpsc, time::Duration};

    #[test]
    fn pending_read_allows_write_and_close_wakes_reader() {
        let session = Session::new(false);
        let (socket, mut peer) = UnixStream::pair().unwrap();
        let id = session.streams.insert(
            Stream::new(socket.try_clone().unwrap(), socket, session.clone(), None).unwrap(),
        );
        let stream = session.streams.get(id).unwrap();
        let (done, received) = mpsc::channel();
        let reader = stream.clone();
        let thread = std::thread::spawn(move || {
            done.send(reader.receive(16)).unwrap();
        });
        peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        assert_eq!(stream.send(b"request").unwrap(), 7);
        let mut bytes = [0; 7];
        peer.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"request");
        peer.write_all(b"reply").unwrap();
        assert_eq!(
            received
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap(),
            b"reply"
        );
        thread.join().unwrap();

        let reader = stream.clone();
        let (done, received) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            done.send(reader.receive(16)).unwrap();
        });
        session.streams.close(id);
        assert_eq!(
            received
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap_err()
                .kind(),
            io::ErrorKind::Interrupted
        );
        thread.join().unwrap();
        assert!(session.streams.get(id).is_err());
        session.stop(0);
    }
}
