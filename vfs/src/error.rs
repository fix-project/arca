use alloc::boxed::Box;

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub error: Option<Box<dyn core::error::Error + Send + Sync>>,
}

impl Error {
    pub fn new(
        kind: ErrorKind,
        error: impl Into<Box<dyn core::error::Error + Send + Sync>>,
    ) -> Error {
        Error {
            kind,
            error: Some(error.into()),
        }
    }

    pub fn other(error: impl Into<Box<dyn core::error::Error + Send + Sync>>) -> Error {
        Error {
            kind: ErrorKind::Other,
            error: Some(error.into()),
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    NotADirectory,
    IsADirectory,
    DirectoryNotEmpty,
    InvalidInput,
    InvalidData,
    TimedOut,
    StorageFull,
    NotSeekable,
    QuotaExceeded,
    FileTooLarge,
    ResourceBusy,
    Deadlock,
    CrossesDevices,
    InvalidFilename,
    ArgumentListTooLong,
    Interrupted,
    Unsupported,
    UnexpectedEof,
    OutOfMemory,
    InProgress,
    Other,
}

pub type Result<T> = core::result::Result<T, Error>;

impl From<ErrorKind> for Error {
    fn from(value: ErrorKind) -> Self {
        Error {
            kind: value,
            error: None,
        }
    }
}
