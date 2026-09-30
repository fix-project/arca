use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    WouldBlock,
    Closed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::WouldBlock => write!(f, "operation would block"),
            Error::Closed => write!(f, "pipe closed by peer"),
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;
