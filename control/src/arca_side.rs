//! Arca-side wrappers over the control pipe.
//!
//! Designed to *feel* like `std::net` for callers:
//!
//! ```ignore
//! let listener = session.bind(Endpoint::new([0, 0, 0, 0], 8080))?;
//! let stream   = session.connect(Endpoint::new([8, 8, 8, 8], 443))?;
//! let inbound  = session.accept(&listener)?;
//! ```
//!
//! The objects we hand back ([`ArcaTcpListener`], [`ArcaTcpStream`]) are
//! lightweight handles — just IDs and a [`DataPipeInfo`]. They deliberately
//! don't implement `Read`/`Write` themselves; the per-connection bytestream
//! is wired up by the data-pipe layer (the rings live in their own SHM
//! region, separate from the control pipe).
//!
//! **Correlation:** every Linux→Arca frame is tagged with the same
//! `request_id` as the Arca→Linux request it answers (including inbound
//! connections via [`MessageType::AcceptRequest`]). There is no separate
//! event stash — if several Arca threads share one control pipe, they must
//! coordinate so only one thread reads at a time (or a single demux task
//! routes by `request_id`).

use arca_pipe::{Read, Write};

use crate::{
    read_frame, write_frame, CodecError, ConnectionReady, ControlReply, ControlRequest,
    DataPipeInfo, Endpoint, MessageType,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcaError {
    Codec(CodecError),
    /// Linux returned `ListenErr` with the given errno-like code.
    ListenFailed {
        code: u32,
    },
    /// Linux returned `ConnectErr` with the given errno-like code.
    ConnectFailed {
        code: u32,
    },
    /// Linux returned `AcceptErr` with the given errno-like code (unknown
    /// listener, or kernel `accept` failed on the monitor side).
    AcceptFailed {
        code: u32,
    },
    /// Got a frame we weren't expecting — protocol bug or out-of-sync state.
    UnexpectedReply(MessageType),
    /// Reply came back with a `request_id` we didn't issue (wrong order on
    /// this transport, or another thread's reply).
    UnexpectedRequestId {
        expected: u32,
        got: u32,
    },
}

impl From<CodecError> for ArcaError {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

/// Handle to a listener Linux is holding open for us. POD on purpose —
/// pass it to `accept` to wait for new connections on this listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArcaTcpListener {
    listener_id: u32,
}

impl ArcaTcpListener {
    pub fn id(&self) -> u32 {
        self.listener_id
    }
}

/// Handle to one accepted/connected TCP session.
///
/// `listener_id == 0` means "outbound connection, no listener". The actual
/// per-direction bytestream is in the data pipe described by `pipe`; this
/// struct is just the metadata Arca needs to attach to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArcaTcpStream {
    listener_id: u32,
    connection_id: u32,
    pipe: DataPipeInfo,
}

impl ArcaTcpStream {
    pub fn connection_id(&self) -> u32 {
        self.connection_id
    }

    /// `0` for outbound `connect`, otherwise the listener that produced this
    /// stream via `accept`.
    pub fn listener_id(&self) -> u32 {
        self.listener_id
    }

    /// Where the per-connection data pipe lives in shared memory.
    pub fn pipe(&self) -> DataPipeInfo {
        self.pipe
    }

    pub fn is_inbound(&self) -> bool {
        self.listener_id != 0
    }
}

/// Owner of the **single** control pipe on the Arca side.
pub struct ArcaSession<'a, T: Read + Write> {
    transport: &'a mut T,
    next_request_id: u32,
}

impl<'a, T: Read + Write> ArcaSession<'a, T> {
    pub fn new(transport: &'a mut T) -> Self {
        Self {
            transport,
            next_request_id: 1,
        }
    }

    fn alloc_request_id(&mut self) -> u32 {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        id
    }

    /// Ask Linux to bind+listen on `ep`. Returns a handle on success.
    pub fn bind(&mut self, ep: Endpoint) -> Result<ArcaTcpListener, ArcaError> {
        let request_id = self.alloc_request_id();
        write_frame(
            self.transport,
            &ControlRequest::Listen {
                request_id,
                endpoint: ep,
            }
            .to_frame(),
        )?;

        match self.read_reply_for(request_id)? {
            ControlReply::ListenOk { listener_id, .. } => Ok(ArcaTcpListener { listener_id }),
            ControlReply::ListenErr { code, .. } => Err(ArcaError::ListenFailed { code }),
            other => Err(ArcaError::UnexpectedReply(other.message_type())),
        }
    }

    /// Ask Linux to connect outbound to `ep`. Returns a handle on success.
    pub fn connect(&mut self, ep: Endpoint) -> Result<ArcaTcpStream, ArcaError> {
        let request_id = self.alloc_request_id();
        write_frame(
            self.transport,
            &ControlRequest::Connect {
                request_id,
                endpoint: ep,
            }
            .to_frame(),
        )?;

        match self.read_reply_for(request_id)? {
            ControlReply::ConnectOk { ready, .. } => Ok(stream_from_ready(ready)),
            ControlReply::ConnectErr { code, .. } => Err(ArcaError::ConnectFailed { code }),
            other => Err(ArcaError::UnexpectedReply(other.message_type())),
        }
    }

    /// Wait for the next inbound connection on `listener`.
    ///
    /// Sends [`ControlRequest::Accept`] and blocks until Linux replies with a
    /// [`ControlReply::AcceptOk`] for the same `request_id`.
    pub fn accept(&mut self, listener: &ArcaTcpListener) -> Result<ArcaTcpStream, ArcaError> {
        let request_id = self.alloc_request_id();
        write_frame(
            self.transport,
            &ControlRequest::Accept {
                request_id,
                listener_id: listener.listener_id,
            }
            .to_frame(),
        )?;

        match self.read_reply_for(request_id)? {
            ControlReply::AcceptOk { ready, .. } => Ok(stream_from_ready(ready)),
            ControlReply::AcceptErr { code, .. } => Err(ArcaError::AcceptFailed { code }),
            other => Err(ArcaError::UnexpectedReply(other.message_type())),
        }
    }

    fn read_reply_for(&mut self, expected_rid: u32) -> Result<ControlReply, ArcaError> {
        let reply = ControlReply::try_from(&read_frame(self.transport)?)?;
        if reply.request_id() != expected_rid {
            return Err(ArcaError::UnexpectedRequestId {
                expected: expected_rid,
                got: reply.request_id(),
            });
        }
        Ok(reply)
    }
}

fn stream_from_ready(ready: ConnectionReady) -> ArcaTcpStream {
    ArcaTcpStream {
        listener_id: ready.listener_id,
        connection_id: ready.connection_id,
        pipe: ready.pipe,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ControlFrame, DataPipeInfo, MessageType};
    use arca_pipe::PipeError;

    /// In-memory transport for tests: writes append, reads pop from front.
    struct MemTransport {
        outbox: [u8; 1024],
        outbox_len: usize,
        inbox: [u8; 1024],
        inbox_len: usize,
        inbox_pos: usize,
    }

    impl MemTransport {
        fn new() -> Self {
            Self {
                outbox: [0u8; 1024],
                outbox_len: 0,
                inbox: [0u8; 1024],
                inbox_len: 0,
                inbox_pos: 0,
            }
        }

        fn push_inbound(&mut self, frame: &ControlFrame) {
            struct InboxWriter<'a>(&'a mut MemTransport);
            impl Write for InboxWriter<'_> {
                fn write(&mut self, src: &[u8]) -> Result<usize, PipeError> {
                    let end = self.0.inbox_len + src.len();
                    assert!(end <= self.0.inbox.len(), "inbox overflow");
                    self.0.inbox[self.0.inbox_len..end].copy_from_slice(src);
                    self.0.inbox_len = end;
                    Ok(src.len())
                }
            }
            let mut w = InboxWriter(self);
            write_frame(&mut w, frame).unwrap();
        }

        fn outbox_slice(&self) -> &[u8] {
            &self.outbox[..self.outbox_len]
        }
    }

    impl Write for MemTransport {
        fn write(&mut self, src: &[u8]) -> Result<usize, PipeError> {
            let end = self.outbox_len + src.len();
            assert!(end <= self.outbox.len(), "outbox overflow");
            self.outbox[self.outbox_len..end].copy_from_slice(src);
            self.outbox_len = end;
            Ok(src.len())
        }
    }

    impl Read for MemTransport {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, PipeError> {
            if self.inbox_pos >= self.inbox_len {
                return Err(PipeError::WouldBlock);
            }
            let n = core::cmp::min(buf.len(), self.inbox_len - self.inbox_pos);
            buf[..n].copy_from_slice(&self.inbox[self.inbox_pos..self.inbox_pos + n]);
            self.inbox_pos += n;
            Ok(n)
        }
    }

    fn listen_ok(rid: u32, listener_id: u32) -> ControlFrame {
        ControlReply::ListenOk {
            request_id: rid,
            listener_id,
        }
        .to_frame()
    }

    fn connect_ok(rid: u32, ready: ConnectionReady) -> ControlFrame {
        ControlReply::ConnectOk {
            request_id: rid,
            ready,
        }
        .to_frame()
    }

    fn incoming(rid: u32, ready: ConnectionReady) -> ControlFrame {
        ControlReply::AcceptOk {
            request_id: rid,
            ready,
        }
        .to_frame()
    }

    #[test]
    fn bind_returns_listener_handle() {
        let mut t = MemTransport::new();
        t.push_inbound(&listen_ok(1, 7));

        let listener = {
            let mut s = ArcaSession::new(&mut t);
            s.bind(Endpoint::new([127, 0, 0, 1], 8080)).unwrap()
        };
        assert_eq!(listener.id(), 7);

        let mut reader = SliceReader {
            data: t.outbox_slice(),
            pos: 0,
        };
        let req = read_frame(&mut reader).unwrap();
        assert_eq!(req.message_type, MessageType::ListenRequest);
        assert_eq!(req.request_id, 1);
    }

    #[test]
    fn connect_returns_stream_handle() {
        let ready = ConnectionReady {
            listener_id: 0,
            connection_id: 17,
            pipe: DataPipeInfo::new(17, 256),
        };
        let mut t = MemTransport::new();
        t.push_inbound(&connect_ok(1, ready));

        let stream = {
            let mut s = ArcaSession::new(&mut t);
            s.connect(Endpoint::new([8, 8, 8, 8], 443)).unwrap()
        };
        assert_eq!(stream.connection_id(), 17);
        assert!(!stream.is_inbound());
        assert_eq!(stream.pipe(), DataPipeInfo::new(17, 256));
    }

    #[test]
    fn accept_sends_accept_request_and_parses_reply() {
        let ready = ConnectionReady {
            listener_id: 5,
            connection_id: 99,
            pipe: DataPipeInfo::new(99, 64),
        };
        let mut t = MemTransport::new();
        t.push_inbound(&incoming(1, ready));

        let mut s = ArcaSession::new(&mut t);
        let listener = ArcaTcpListener { listener_id: 5 };
        let inbound = s.accept(&listener).unwrap();
        assert_eq!(inbound.connection_id(), 99);
        assert!(inbound.is_inbound());

        let mut reader = SliceReader {
            data: t.outbox_slice(),
            pos: 0,
        };
        let req = read_frame(&mut reader).unwrap();
        assert_eq!(
            ControlRequest::try_from(&req).unwrap(),
            ControlRequest::Accept {
                request_id: 1,
                listener_id: 5
            }
        );
    }

    #[test]
    fn listen_failure_propagates_errno() {
        let mut t = MemTransport::new();
        t.push_inbound(
            &ControlReply::ListenErr {
                request_id: 1,
                code: 98,
            }
            .to_frame(),
        );

        let mut s = ArcaSession::new(&mut t);
        let err = s.bind(Endpoint::new([0, 0, 0, 0], 1)).unwrap_err();
        assert_eq!(err, ArcaError::ListenFailed { code: 98 });
    }

    #[test]
    fn accept_failure_propagates_errno() {
        let mut t = MemTransport::new();
        t.push_inbound(
            &ControlReply::AcceptErr {
                request_id: 1,
                code: 9,
            }
            .to_frame(),
        );

        let mut s = ArcaSession::new(&mut t);
        let listener = ArcaTcpListener { listener_id: 99 };
        let err = s.accept(&listener).unwrap_err();
        assert_eq!(err, ArcaError::AcceptFailed { code: 9 });
    }

    struct SliceReader<'a> {
        data: &'a [u8],
        pos: usize,
    }

    impl<'a> Read for SliceReader<'a> {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, PipeError> {
            if self.pos >= self.data.len() {
                return Err(PipeError::WouldBlock);
            }
            let n = core::cmp::min(buf.len(), self.data.len() - self.pos);
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }
}
