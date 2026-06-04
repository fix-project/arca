use arca_pipe::{BidirectionalPipe, PipeError};
use arca_pipe::Read as PipeRead;
use arca_pipe::Write as PipeWrite;

#[derive(Debug)]
pub enum StreamError {
    WriteClosed,
}

pub struct AsyncStream<'a> {
    /// BuddyAllocator offset of the SHM region backing this pipe.
    pub shm_offset: u64,
    pipe: BidirectionalPipe<'a>,
}

impl<'a> AsyncStream<'a> {
    pub fn from_pipe(shm_offset: u64, pipe: BidirectionalPipe<'a>) -> Self {
        Self { shm_offset, pipe }
    }

    /// Write all of `buf` into the pipe, yielding if the ring is full; returns `Err(WriteClosed)` if the peer closed their read side.
    pub async fn send(&mut self, buf: &[u8]) -> Result<usize, StreamError> {
        if self.pipe.is_peer_read_closed() {
            self.pipe.close_write();
            return Err(StreamError::WriteClosed);
        }
        if buf.is_empty() {
            return Ok(0);
        }
        write_all(&mut self.pipe, buf).await;
        Ok(buf.len())
    }

    /// Read exactly `buf.len()` bytes, yielding until full; returns `Ok(n < buf.len())` only on EOF when the peer closed their write side.
    pub async fn recv(&mut self, buf: &mut [u8]) -> Result<usize, StreamError> {
        let n = read_exact(&mut self.pipe, buf).await;
        if n < buf.len() {
            self.pipe.close_read();
        }
        Ok(n)
    }

    pub fn close_write(&mut self) {
        self.pipe.close_write();
    }

    pub fn close_read(&mut self) {
        self.pipe.close_read();
    }

    pub fn is_closed(&self) -> bool {
        self.pipe.is_closed()
    }
}

async fn read_exact(pipe: &mut arca_pipe::BidirectionalPipe<'_>, buf: &mut [u8]) -> usize {
    let mut filled = 0;
    while filled < buf.len() {
        match pipe.read(&mut buf[filled..]) {
            Ok(n) => filled += n,
            Err(PipeError::WouldBlock) => {
                if pipe.is_peer_write_closed() {
                    break;
                }
                yield_now().await;
            }
        }
    }
    filled
}

async fn write_all<W: PipeWrite>(pipe: &mut W, buf: &[u8]) {
    let mut remaining = buf;
    while !remaining.is_empty() {
        match pipe.write(remaining) {
            Ok(n) => remaining = &remaining[n..],
            Err(PipeError::WouldBlock) => yield_now().await,
        }
    }
}

async fn yield_now() {
    let mut yielded = false;
    core::future::poll_fn(|cx| {
        if yielded { return core::task::Poll::Ready(()); }
        yielded = true;
        cx.waker().wake_by_ref();
        core::task::Poll::Pending
    }).await
}
