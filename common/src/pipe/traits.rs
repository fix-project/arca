use crate::pipe::error::PipeError;

/// Read bytes from a pipe. Analogous to std::io::Read.
///
/// Partial reads are normal — `read` may return fewer bytes than `buf.len()`.
/// The caller loops if it needs more. This matches `std::io` semantics.
pub trait Read {
    /// Try to read bytes into `buf`.
    ///
    /// Returns `Ok(n)` where `n > 0` is the number of bytes read,
    /// or `Err(WouldBlock)` if no data is currently available.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PipeError>;
}

/// Write bytes to a pipe. Analogous to std::io::Write.
///
/// Partial writes are normal — `write` may accept fewer bytes than `buf.len()`.
/// The caller loops if it needs to write more. This matches `std::io` semantics.
pub trait Write {
    /// Try to write bytes from `buf`.
    ///
    /// Returns `Ok(n)` where `n > 0` is the number of bytes written,
    /// or `Err(WouldBlock)` if the ring is currently full.
    fn write(&mut self, buf: &[u8]) -> Result<usize, PipeError>;

    /// Write all bytes in `src`, spinning on `WouldBlock` until every byte is written.
    fn write_all(&mut self, mut src: &[u8]) {
        while !src.is_empty() {
            match self.write(src) {
                Ok(n) => src = &src[n..],
                Err(PipeError::WouldBlock) => core::hint::spin_loop(),
            }
        }
    }
}

/// Notify the waiter on newly available event (readable/writable)
///
/// ring blocks until it's possible to ring
pub trait DoorBell {
    fn ring(&self);
}

/// Wait for an event.
///
/// DoorBellWaiter on the arca-side is handled by kthreads wfi
#[cfg(feature = "std")]
pub trait DoorBellWaiter {
    fn wait(&mut self);
}
