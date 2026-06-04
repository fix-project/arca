//! End-to-end tests: run an [`ArcaSession`] against a real [`Monitor`] over a
//! shared transport, with real Linux TCP sockets at the far end.
//!
//! What's *not* exercised here:
//! - The `arca_pipe::BidirectionalPipe` itself. That has its own unit tests
//!   in `arca-pipe`, and using it cross-thread requires `unsafe impl Sync`
//!   on `SharedMemoryRegion`, which is owned by another crate. The protocol
//!   is transport-agnostic — anything that implements `arca_pipe::Read +
//!   Write` slots in.
//!
//! What *is* exercised:
//! - The full request/reply round trip across two threads.
//! - Real outbound `connect` against a stdlib `TcpListener` peer.
//! - Real inbound `accept` flow: Arca `bind`, Arca `accept` (posts
//!   `AcceptRequest`), monitor pairs the kernel `accept` with that wait and
//!   replies with `IncomingConnection`.

use std::collections::VecDeque;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use arca_control::{ArcaSession, Endpoint};
use arca_monitor::Monitor;
use arca_pipe::{PipeError, Read, Write};

/// One end of a thread-safe bidirectional in-memory transport.
///
/// Reads come from `inbox`; writes append to the *other* side's inbox.
/// `read` is non-blocking (returns `WouldBlock` on empty), which is exactly
/// what `arca_pipe::Read` requires.
#[derive(Clone)]
struct ChannelEnd {
    inbox: Arc<Mutex<VecDeque<u8>>>,
    outbox: Arc<Mutex<VecDeque<u8>>>,
}

fn channel_pair() -> (ChannelEnd, ChannelEnd) {
    let a_to_b = Arc::new(Mutex::new(VecDeque::<u8>::new()));
    let b_to_a = Arc::new(Mutex::new(VecDeque::<u8>::new()));
    let a = ChannelEnd {
        inbox: b_to_a.clone(),
        outbox: a_to_b.clone(),
    };
    let b = ChannelEnd {
        inbox: a_to_b,
        outbox: b_to_a,
    };
    (a, b)
}

impl Read for ChannelEnd {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PipeError> {
        let mut q = self.inbox.lock().unwrap();
        if q.is_empty() {
            return Err(PipeError::WouldBlock);
        }
        let n = buf.len().min(q.len());
        for slot in buf.iter_mut().take(n) {
            *slot = q.pop_front().unwrap();
        }
        Ok(n)
    }
}

impl Write for ChannelEnd {
    fn write(&mut self, src: &[u8]) -> Result<usize, PipeError> {
        let mut q = self.outbox.lock().unwrap();
        q.extend(src.iter().copied());
        Ok(src.len())
    }
}

#[test]
fn arca_connect_round_trip_against_real_tcp_listener() {
    // The Linux kernel-side "remote peer" — accepts one connection, echoes 4 bytes.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        use std::io::{Read as IoRead, Write as IoWrite};
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4];
        s.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"ping");
        s.write_all(b"pong").unwrap();
    });

    let (mut arca_end, mut mon_end) = channel_pair();

    // Monitor thread: handle exactly one ConnectRequest.
    let monitor_thread = thread::spawn(move || {
        let mut m = Monitor::new(64);
        m.serve_one(&mut mon_end).unwrap();
        m
    });

    // Arca side: connect to the listener.
    let mut arca = ArcaSession::new(&mut arca_end);
    let stream = arca
        .connect(Endpoint::new([127, 0, 0, 1], port))
        .expect("connect should succeed");

    assert!(!stream.is_inbound(), "outbound connect has listener_id 0");
    assert_eq!(stream.connection_id(), 1, "first connection gets id 1");
    assert_eq!(stream.pipe().ring_size, 64, "ring_size from default config");
    assert!(stream.pipe().shm_offset > 0, "shm_offset should be a real BuddyAllocator offset");

    // Monitor returns; verify it actually owns a live socket for that id.
    let mut m = monitor_thread.join().unwrap();
    let mut owned = m
        .connection(stream.connection_id())
        .unwrap()
        .try_clone()
        .unwrap();

    // Drive the byte exchange to make sure the kernel socket really works.
    use std::io::{Read as IoRead, Write as IoWrite};
    owned.write_all(b"ping").unwrap();
    let mut got = [0u8; 4];
    owned.read_exact(&mut got).unwrap();
    assert_eq!(&got, b"pong");

    server.join().unwrap();
}

#[test]
fn arca_bind_then_accept_after_external_connect() {
    let (mut arca_end, mut mon_end) = channel_pair();

    // Lets the monitor thread tell the test what port it bound to and lets
    // the test signal it to shut down.
    let (port_tx, port_rx) = mpsc::channel::<u16>();
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_for_thread = shutdown.clone();

    let monitor_thread = thread::spawn(move || {
        let mut m = Monitor::new(64);

        // 1. Read the bind request, dispatch, peek the bound port, write reply.
        use arca_control::{read_frame, write_frame, ControlReply, MessageType};
        let req = read_frame(&mut mon_end).unwrap();
        assert_eq!(req.message_type, MessageType::ListenRequest);
        let reply = m.dispatch_request(req).unwrap();
        let ControlReply::ListenOk {
            listener_id: lid, ..
        } = ControlReply::try_from(&reply).unwrap()
        else {
            panic!("expected ListenOk");
        };
        let port = m
            .listener(lid)
            .expect("listener should exist after dispatch")
            .local_addr()
            .unwrap()
            .port();
        port_tx.send(port).unwrap();
        write_frame(&mut mon_end, &reply).unwrap();

        // 2. Poll: kernel accepts + any Arca→Linux frames (AcceptRequest, …).
        while !shutdown_for_thread.load(Ordering::Relaxed) {
            m.pump_once(&mut mon_end).unwrap();
            thread::sleep(Duration::from_millis(2));
        }
        // Last pass so a frame just before shutdown is not left stranded.
        m.pump_once(&mut mon_end).unwrap();
        m
    });

    // Arca: bind on an ephemeral port.
    let mut arca = ArcaSession::new(&mut arca_end);
    let listener = arca
        .bind(Endpoint::new([127, 0, 0, 1], 0))
        .expect("bind should succeed");
    assert_eq!(listener.id(), 1);

    // External peer connects to the bound port.
    let port = port_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let _peer = thread::spawn(move || {
        // Tiny delay so the monitor's pump loop is already running.
        thread::sleep(Duration::from_millis(20));
        let _stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        // Hold the connection open until the test ends, otherwise the peer
        // might close before the monitor can `accept`.
        thread::sleep(Duration::from_millis(500));
    });

    // Arca: accept the inbound connection. Bound the wait so a regression
    // doesn't hang CI.
    let started = Instant::now();
    let stream = loop {
        if started.elapsed() > Duration::from_secs(3) {
            panic!("accept timed out");
        }
        match arca.accept(&listener) {
            Ok(s) => break s,
            Err(arca_control::ArcaError::Codec(_)) => {
                // Codec spinning on WouldBlock — read_frame's read_exact
                // already loops, so this branch is mostly dead, but keep
                // it for safety.
                thread::sleep(Duration::from_millis(2));
            }
            Err(e) => panic!("unexpected accept error: {:?}", e),
        }
    };

    assert!(stream.is_inbound());
    assert_eq!(stream.listener_id(), listener.id());

    // Tear down monitor cleanly.
    shutdown.store(true, Ordering::Relaxed);
    let mut m = monitor_thread.join().unwrap();
    assert!(m.connection(stream.connection_id()).is_some());
}
