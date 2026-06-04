#![feature(allocator_api)]
//! Linux-side **monitor**: holds the kernel sockets and translates Arca's
//! control-protocol requests into real `TcpListener` / `TcpStream` actions.
//!
//! Architecture in one paragraph:
//!
//! - The monitor owns one [`TcpListener`] per `ControlReply::ListenOk` it
//!   has handed out, and one [`TcpStream`] per live connection.
//! - Listeners are kept **non-blocking** so [`Monitor::poll_accepts`] can probe
//!   the kernel when Arca has queued an accept wait, without wedging the I/O
//!   thread when no TCP peer is ready yet.
//! - Connection streams are left in their default mode — the *byte pump*
//!   (the data-pipe layer) decides blocking vs non-blocking based on its own
//!   scheduling needs.
//!
//! Driving the protocol is one of:
//! - [`Monitor::dispatch_request`] — `Listen` / `Connect` only (for tests and
//!   custom drivers); [`MessageType::AcceptRequest`] is handled in
//!   [`Monitor::pump_once`] / [`Monitor::serve_one`].
//! - [`Monitor::pump_once`] — non-blocking kernel accepts + try read every
//!   fully received Arca→Linux frame on `transport`.
//! - [`Monitor::serve_one`] — spins until a full Arca→Linux frame exists, then
//!   dispatches it (uses [`std::thread::yield_now`] while waiting).

mod relay;

pub use relay::{pipe_to_tcp, tcp_to_pipe};

use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};

use arca_control::{
    write_frame, CodecError, ConnectionReady, ControlFrame, ControlReply, ControlRequest,
    DataPipeInfo, FrameReadBuf, MessageType,
};
use arca_pipe::{BidirectionalPipe, Read, SharedMemoryRegion, Side, Write};
use common::BuddyAllocator;

/// Errno-like code we send to the guest in [`ControlReply::AcceptErr`] when
/// the request referenced a `listener_id` we don't know about (closest match
/// to Linux's `EBADF`).
const ERR_UNKNOWN_LISTENER: u32 = 9;

#[derive(Debug)]
pub enum MonitorError {
    Io(io::Error),
    Codec(CodecError),
    UnexpectedRequest(MessageType),
}

impl From<io::Error> for MonitorError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<CodecError> for MonitorError {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

fn io_err_code(e: &io::Error) -> u32 {
    // Linux `errno` when available, else `1` to mean "unknown error".
    e.raw_os_error().map(|x| x as u32).unwrap_or(1)
}

/// One live TCP connection plus its data pipe on the monitor side.
pub struct Connection {
    pub tcp: TcpStream,
    /// Owns the SHM allocation backing the BidirectionalPipe.
    /// Set to None when the pipe is fully closed to free the region;
    /// the TCP socket stays alive until the OS-level connection closes.
    pub shm: Option<Box<[u8], BuddyAllocator>>,
}

/// Linux-side state machine.
pub struct Monitor {
    next_listener_id: u32,
    next_connection_id: u32,
    default_ring_size: u64,
    listeners: HashMap<u32, TcpListener>,
    connections: HashMap<u32, Connection>,
    /// For each listener, FIFO of Arca `request_id`s waiting for a kernel `accept`.
    pending_accepts: HashMap<u32, VecDeque<u32>>,
    control_rx: FrameReadBuf,
}

impl Monitor {
    pub fn new(default_ring_size: u64) -> Self {
        Self {
            // 0 is reserved as "no listener" sentinel for outbound connects.
            next_listener_id: 1,
            next_connection_id: 1,
            default_ring_size,
            listeners: HashMap::new(),
            connections: HashMap::new(),
            pending_accepts: HashMap::new(),
            control_rx: FrameReadBuf::new(),
        }
    }

    /// Translate one Arca → Linux request frame into the reply we owe Arca.
    ///
    /// Handles only [`MessageType::ListenRequest`] and
    /// [`MessageType::ConnectRequest`]. [`MessageType::AcceptRequest`] is
    /// handled in [`Monitor::handle_control_frame`].
    pub fn dispatch_request(&mut self, frame: ControlFrame) -> Result<ControlFrame, MonitorError> {
        let request = ControlRequest::try_from(&frame)?;
        let rid = request.request_id();
        match request {
            ControlRequest::Listen { endpoint, .. } => {
                let addr = SocketAddr::from((Ipv4Addr::from(endpoint.host), endpoint.port));
                let reply = match TcpListener::bind(addr) {
                    Ok(listener) => {
                        listener.set_nonblocking(true)?;
                        let id = self.alloc_listener_id();
                        self.listeners.insert(id, listener);
                        ControlReply::ListenOk {
                            request_id: rid,
                            listener_id: id,
                        }
                    }
                    Err(e) => ControlReply::ListenErr {
                        request_id: rid,
                        code: io_err_code(&e),
                    },
                };
                Ok(reply.to_frame())
            }
            ControlRequest::Connect { endpoint, .. } => {
                let addr = SocketAddr::from((Ipv4Addr::from(endpoint.host), endpoint.port));
                // Blocks until the kernel handshake completes — “connect waits”.
                let reply = match TcpStream::connect(addr) {
                    Ok(stream) => {
                        let id = self.alloc_connection_id();
                        let shm: Box<[u8], BuddyAllocator> = unsafe {
                            Box::new_zeroed_slice_in(
                                BidirectionalPipe::required_size(self.default_ring_size) as usize,
                                BuddyAllocator,
                            )
                            .assume_init()
                        };
                        let shm_offset =
                            BuddyAllocator.to_offset(shm.as_ptr() as *const ()) as u64;
                        self.connections.insert(id, Connection { tcp: stream, shm: Some(shm) });
                        ControlReply::ConnectOk {
                            request_id: rid,
                            ready: ConnectionReady {
                                listener_id: 0,
                                connection_id: id,
                                pipe: DataPipeInfo::new(shm_offset, self.default_ring_size),
                            },
                        }
                    }
                    Err(e) => ControlReply::ConnectErr {
                        request_id: rid,
                        code: io_err_code(&e),
                    },
                };
                Ok(reply.to_frame())
            }
            ControlRequest::Accept { .. } => {
                Err(MonitorError::UnexpectedRequest(MessageType::AcceptRequest))
            }
        }
    }

    /// Try pairing pending Arca `AcceptRequest`s with kernel `accept` results,
    /// writing one [`MessageType::IncomingConnection`] per successful accept
    /// (each carrying the Arca-issued `request_id`).
    pub fn poll_accepts<T: Write>(&mut self, transport: &mut T) -> Result<usize, MonitorError> {
        use std::io::ErrorKind;
        let mut written = 0usize;
        let lids: Vec<u32> = self
            .pending_accepts
            .iter()
            .filter(|(_, q)| !q.is_empty())
            .map(|(k, _)| *k)
            .collect();
        for lid in lids {
            let Some(listener) = self.listeners.get_mut(&lid) else {
                continue;
            };
            match listener.accept() {
                Ok((stream, _)) => {
                    let Some(rid) = self
                        .pending_accepts
                        .get_mut(&lid)
                        .and_then(|q| q.pop_front())
                    else {
                        continue;
                    };
                    let cid = self.alloc_connection_id();
                    let shm: Box<[u8], BuddyAllocator> = unsafe {
                        Box::new_zeroed_slice_in(
                            BidirectionalPipe::required_size(self.default_ring_size) as usize,
                            BuddyAllocator,
                        )
                        .assume_init()
                    };
                    let shm_offset =
                        BuddyAllocator.to_offset(shm.as_ptr() as *const ()) as u64;
                    self.connections.insert(cid, Connection { tcp: stream, shm: Some(shm) });
                    let reply = ControlReply::AcceptOk {
                        request_id: rid,
                        ready: ConnectionReady {
                            listener_id: lid,
                            connection_id: cid,
                            pipe: DataPipeInfo::new(shm_offset, self.default_ring_size),
                        },
                    };
                    write_frame(transport, &reply.to_frame())?;
                    written += 1;
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) => {
                    // Kernel `accept` failed for a reason other than
                    // `WouldBlock`. Tell the next waiter on this listener
                    // instead of dropping the error on the floor.
                    if let Some(rid) = self
                        .pending_accepts
                        .get_mut(&lid)
                        .and_then(|q| q.pop_front())
                    {
                        let reply = ControlReply::AcceptErr {
                            request_id: rid,
                            code: io_err_code(&e),
                        };
                        write_frame(transport, &reply.to_frame())?;
                        written += 1;
                    }
                }
            }
        }
        Ok(written)
    }

    /// Non-blocking progress: poll kernel accepts, then read and handle every
    /// fully received Arca→Linux frame currently available on `transport`.
    pub fn pump_once<T: Read + Write>(&mut self, transport: &mut T) -> Result<(), MonitorError> {
        self.poll_accepts(transport)?;
        while let Some(frame) = self.control_rx.try_read_frame(transport)? {
            self.handle_control_frame(transport, frame)?;
        }
        Ok(())
    }

    fn handle_control_frame<T: Write>(
        &mut self,
        transport: &mut T,
        frame: ControlFrame,
    ) -> Result<(), MonitorError> {
        match ControlRequest::try_from(&frame)? {
            ControlRequest::Accept {
                request_id,
                listener_id,
            } => {
                if !self.listeners.contains_key(&listener_id) {
                    let reply = ControlReply::AcceptErr {
                        request_id,
                        code: ERR_UNKNOWN_LISTENER,
                    };
                    write_frame(transport, &reply.to_frame())?;
                    return Ok(());
                }
                self.pending_accepts
                    .entry(listener_id)
                    .or_default()
                    .push_back(request_id);
                self.poll_accepts(transport)?;
                Ok(())
            }
            _ => {
                let reply = self.dispatch_request(frame)?;
                write_frame(transport, &reply)?;
                self.poll_accepts(transport)?;
                Ok(())
            }
        }
    }

    /// Read and dispatch one Arca→Linux frame. Spins with
    /// [`std::thread::yield_now`] until the incremental decoder can produce a
    /// full frame (transport keeps returning [`arca_pipe::PipeError::WouldBlock`]).
    pub fn serve_one<T: Read + Write>(&mut self, transport: &mut T) -> Result<(), MonitorError> {
        loop {
            self.poll_accepts(transport)?;
            if let Some(frame) = self.control_rx.try_read_frame(transport)? {
                return self.handle_control_frame(transport, frame);
            }
            std::thread::yield_now();
        }
    }

    /// Borrow a live connection's `TcpStream` for the byte pump.
    pub fn connection(&mut self, id: u32) -> Option<&mut TcpStream> {
        self.connections.get_mut(&id).map(|c| &mut c.tcp)
    }

    /// Main event loop: pumps control frames and relays bytes between each
    /// live TcpStream and its data pipe until an error occurs.
    pub fn event_loop<T: Read + Write>(&mut self, control_pipe: &mut T) -> Result<(), MonitorError> {
        loop {
            self.pump_once(control_pipe)?;
            for (_, conn) in &mut self.connections {
                let Some(shm) = &conn.shm else { continue };
                let region = unsafe {
                    SharedMemoryRegion::from_raw(shm.as_ptr() as *mut u8, shm.len() as u64)
                };
                let mut pipe = BidirectionalPipe::new(&region, self.default_ring_size, Side::B);
                tcp_to_pipe(&mut conn.tcp, &mut pipe).ok();
                pipe_to_tcp(&mut conn.tcp, &mut pipe).ok();
                if pipe.is_closed() {
                    // Drop the Box — BuddyAllocator::deallocate runs automatically,
                    // returning the SHM region to the buddy allocator.
                    conn.shm = None;
                }
            }
            std::thread::yield_now();
        }
    }

    /// Borrow a live listener (mostly for tests / introspection).
    pub fn listener(&self, id: u32) -> Option<&TcpListener> {
        self.listeners.get(&id)
    }

    fn alloc_listener_id(&mut self) -> u32 {
        let id = self.next_listener_id;
        self.next_listener_id = self.next_listener_id.wrapping_add(1);
        if self.next_listener_id == 0 {
            self.next_listener_id = 1;
        }
        id
    }

    fn alloc_connection_id(&mut self) -> u32 {
        let id = self.next_connection_id;
        self.next_connection_id = self.next_connection_id.wrapping_add(1);
        if self.next_connection_id == 0 {
            self.next_connection_id = 1;
        }
        id
    }
}

#[cfg(test)]
impl Monitor {
    pub(crate) fn test_enqueue_accept(&mut self, listener_id: u32, rid: u32) {
        self.pending_accepts
            .entry(listener_id)
            .or_default()
            .push_back(rid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arca_control::{read_frame, Endpoint};
    use arca_pipe::{PipeError, Read as PipeRead, Write as PipeWrite};
    use std::io::{Read as IoRead, Write as IoWrite};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    struct VecWriter(Vec<u8>);

    impl PipeWrite for VecWriter {
        fn write(&mut self, buf: &[u8]) -> Result<usize, PipeError> {
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }
    }

    /// In-memory control pipe: pops inbound bytes, collects outbound writes.
    struct QueuePipe {
        inbound: std::collections::VecDeque<u8>,
        outbound: Vec<u8>,
    }

    impl PipeRead for QueuePipe {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, PipeError> {
            if self.inbound.is_empty() {
                return Err(PipeError::WouldBlock);
            }
            let n = std::cmp::min(buf.len(), self.inbound.len());
            for i in 0..n {
                buf[i] = self.inbound.pop_front().unwrap();
            }
            Ok(n)
        }
    }

    impl PipeWrite for QueuePipe {
        fn write(&mut self, buf: &[u8]) -> Result<usize, PipeError> {
            self.outbound.extend_from_slice(buf);
            Ok(buf.len())
        }
    }

    #[test]
    fn accept_for_unknown_listener_replies_with_accept_err() {
        use arca_control::write_frame;
        let mut m = Monitor::new(64);

        // Accept on a listener_id we never handed out.
        let acc = ControlRequest::Accept {
            request_id: 55,
            listener_id: 999,
        }
        .to_frame();
        let mut enc = VecWriter(Vec::new());
        write_frame(&mut enc, &acc).unwrap();

        let mut transport = QueuePipe {
            inbound: std::collections::VecDeque::from(enc.0),
            outbound: Vec::new(),
        };
        m.pump_once(&mut transport).unwrap();

        // Monitor should have written an AcceptErr frame back to the guest.
        struct SliceReader<'a> {
            data: &'a [u8],
            pos: usize,
        }
        impl PipeRead for SliceReader<'_> {
            fn read(&mut self, buf: &mut [u8]) -> Result<usize, PipeError> {
                if self.pos >= self.data.len() {
                    return Err(PipeError::WouldBlock);
                }
                let n = std::cmp::min(buf.len(), self.data.len() - self.pos);
                buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            }
        }
        let mut r = SliceReader {
            data: &transport.outbound,
            pos: 0,
        };
        let fr = read_frame(&mut r).unwrap();
        let ControlReply::AcceptErr {
            request_id: 55,
            code,
        } = ControlReply::try_from(&fr).unwrap()
        else {
            panic!("expected AcceptErr(rid=55)");
        };
        assert_eq!(code, ERR_UNKNOWN_LISTENER);
    }

    #[test]
    fn pump_once_reads_accept_request_then_tcp_pairs_request_id() {
        use arca_control::write_frame;
        let mut m = Monitor::new(64);
        let listen = ControlRequest::Listen {
            request_id: 1,
            endpoint: Endpoint::new([127, 0, 0, 1], 0),
        };
        let reply = m.dispatch_request(listen.to_frame()).unwrap();
        let ControlReply::ListenOk {
            listener_id: lid, ..
        } = ControlReply::try_from(&reply).unwrap()
        else {
            panic!("expected ListenOk");
        };

        let acc = ControlRequest::Accept {
            request_id: 77,
            listener_id: lid,
        }
        .to_frame();
        let mut enc = VecWriter(Vec::new());
        write_frame(&mut enc, &acc).unwrap();

        let mut transport = QueuePipe {
            inbound: std::collections::VecDeque::from(enc.0),
            outbound: Vec::new(),
        };
        m.pump_once(&mut transport).unwrap();

        let port = m
            .listener(lid)
            .expect("listener")
            .local_addr()
            .unwrap()
            .port();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(30));
            let _ = TcpStream::connect(("127.0.0.1", port));
        });
        thread::sleep(Duration::from_millis(60));

        let mut w = VecWriter(Vec::new());
        assert_eq!(m.poll_accepts(&mut w).unwrap(), 1);

        struct FrameSliceReader<'a> {
            data: &'a [u8],
            pos: usize,
        }
        impl PipeRead for FrameSliceReader<'_> {
            fn read(&mut self, buf: &mut [u8]) -> Result<usize, PipeError> {
                if self.pos >= self.data.len() {
                    return Err(PipeError::WouldBlock);
                }
                let n = std::cmp::min(buf.len(), self.data.len() - self.pos);
                buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            }
        }
        let mut r = FrameSliceReader { data: &w.0, pos: 0 };
        let fr = read_frame(&mut r).unwrap();
        let ControlReply::AcceptOk {
            request_id: 77,
            ready,
        } = ControlReply::try_from(&fr).unwrap()
        else {
            panic!("expected AcceptOk(rid=77)");
        };
        assert_eq!(ready.listener_id, lid);
    }

    #[test]
    fn listen_dispatch_binds() {
        let mut m = Monitor::new(64);
        let req = ControlRequest::Listen {
            request_id: 7,
            endpoint: Endpoint::new([127, 0, 0, 1], 0),
        }
        .to_frame();
        let reply = m.dispatch_request(req).unwrap();
        let ControlReply::ListenOk {
            request_id: 7,
            listener_id,
        } = ControlReply::try_from(&reply).unwrap()
        else {
            panic!("expected ListenOk(rid=7)");
        };
        assert!(m.listeners.contains_key(&listener_id));
    }

    #[test]
    fn connect_dispatch_reaches_listener() {
        let (tx, rx) = mpsc::channel::<u16>();
        let server = thread::spawn(move || {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            l.set_nonblocking(false).unwrap();
            let port = l.local_addr().unwrap().port();
            tx.send(port).unwrap();
            let (mut s, _) = l.accept().unwrap();
            let mut buf = [0u8; 4];
            s.read_exact(&mut buf).unwrap();
            assert_eq!(&buf, b"ping");
            s.write_all(b"pong").unwrap();
        });

        let port = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let mut m = Monitor::new(64);
        let req = ControlRequest::Connect {
            request_id: 1,
            endpoint: Endpoint::new([127, 0, 0, 1], port),
        }
        .to_frame();
        let reply = m.dispatch_request(req).unwrap();
        let ControlReply::ConnectOk { ready, .. } = ControlReply::try_from(&reply).unwrap() else {
            panic!("expected ConnectOk");
        };
        let stream = m.connection(ready.connection_id).unwrap();
        let mut owned = stream.try_clone().unwrap();
        owned.write_all(b"ping").unwrap();
        let mut out = [0u8; 4];
        owned.read_exact(&mut out).unwrap();
        assert_eq!(&out, b"pong");

        server.join().unwrap();
    }

    #[test]
    fn incoming_after_client_connects() {
        let mut m = Monitor::new(64);
        let req = ControlRequest::Listen {
            request_id: 1,
            endpoint: Endpoint::new([127, 0, 0, 1], 0),
        }
        .to_frame();
        let reply = m.dispatch_request(req).unwrap();
        let ControlReply::ListenOk {
            listener_id: lid, ..
        } = ControlReply::try_from(&reply).unwrap()
        else {
            panic!("expected ListenOk");
        };
        let port = m.listeners.get(&lid).unwrap().local_addr().unwrap().port();
        m.test_enqueue_accept(lid, 42);

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            let _ = TcpStream::connect(("127.0.0.1", port));
        });

        thread::sleep(Duration::from_millis(50));
        let mut w = VecWriter(Vec::new());
        let written = m.poll_accepts(&mut w).unwrap();
        assert_eq!(written, 1);

        struct SliceReader<'a> {
            data: &'a [u8],
            pos: usize,
        }

        impl PipeRead for SliceReader<'_> {
            fn read(&mut self, buf: &mut [u8]) -> Result<usize, PipeError> {
                if self.pos >= self.data.len() {
                    return Err(PipeError::WouldBlock);
                }
                let n = std::cmp::min(buf.len(), self.data.len() - self.pos);
                buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            }
        }

        let mut r = SliceReader { data: &w.0, pos: 0 };
        let ev = read_frame(&mut r).unwrap();
        let ControlReply::AcceptOk {
            request_id: 42,
            ready,
        } = ControlReply::try_from(&ev).unwrap()
        else {
            panic!("expected AcceptOk(rid=42)");
        };
        assert_eq!(ready.listener_id, lid);
    }

    #[test]
    fn tcp_to_pipe_reads_from_tcp() {
        const RING: u64 = 64;
        #[repr(align(8))]
        struct Mem([u8; BidirectionalPipe::required_size(RING) as usize]);

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            s.write_all(b"hi").unwrap();
        });

        let mut tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();
        thread::sleep(Duration::from_millis(20));

        let mut mem = Mem([0u8; BidirectionalPipe::required_size(RING) as usize]);
        let region = unsafe { SharedMemoryRegion::from_raw(mem.0.as_mut_ptr(), mem.0.len() as u64) };
        let mut pipe_b = BidirectionalPipe::new(&region, RING, Side::B);
        let n = relay::tcp_to_pipe(&mut tcp, &mut pipe_b).unwrap();
        assert_eq!(n, 2);

        // Side::A is what Arca would use; read back to verify bytes arrived in the ring.
        let mut pipe_a = BidirectionalPipe::new(&region, RING, Side::A);
        let mut out = [0u8; 2];
        pipe_a.read(&mut out).unwrap();
        assert_eq!(&out, b"hi");

        server.join().unwrap();
    }
}
