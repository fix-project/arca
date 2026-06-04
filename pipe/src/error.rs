use core::fmt;

/// Errors returned by pipe operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeError {
    /// Ring buffer is empty (read) or full (write). Try again later.
    WouldBlock,
}

impl fmt::Display for PipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipeError::WouldBlock => write!(f, "operation would block"),
        }
    }
}
