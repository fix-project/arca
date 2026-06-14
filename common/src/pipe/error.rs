use core::fmt;

/// Errors returned by pipe operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeError {
    /// Ring buffer is empty (read) or full (write). Try again later.
    WouldBlock,
    /// The other end has closed: writing to a pipe whose reader has closed.
    ///
    /// Note that read-side end-of-stream is *not* an error — a drained ring
    /// whose writer has closed returns `Ok(0)` (EOF), matching `std::io::Read`.
    Closed,
}

impl fmt::Display for PipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipeError::WouldBlock => write!(f, "operation would block"),
            PipeError::Closed => write!(f, "pipe closed by peer"),
        }
    }
}
