extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

pub use embedded_io::ErrorKind;

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    GetArgs,
    Exit(i32),
    Open(String, FileMode),
    Mkdir(String),
    Listen { ip: [u8; 4], port: u16 },
    Connect { host: String, port: u16 },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Args(Vec<String>),
    Pipe(PipeData),
    Ack,
    Err(IoErrorKind),
}

/// Wire-serializable mirror of [`embedded_io::ErrorKind`], which is
/// `#[non_exhaustive]` and not `serde`-derivable. Carries a host I/O failure
/// back to the guest, where it converts into an `embedded_io::ErrorKind`.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum IoErrorKind {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    InvalidInput,
    InvalidData,
    Unsupported,
    OutOfMemory,
    Interrupted,
    Other,
}

impl From<IoErrorKind> for ErrorKind {
    fn from(e: IoErrorKind) -> Self {
        match e {
            IoErrorKind::NotFound => ErrorKind::NotFound,
            IoErrorKind::PermissionDenied => ErrorKind::PermissionDenied,
            IoErrorKind::AlreadyExists => ErrorKind::AlreadyExists,
            IoErrorKind::InvalidInput => ErrorKind::InvalidInput,
            IoErrorKind::InvalidData => ErrorKind::InvalidData,
            IoErrorKind::Unsupported => ErrorKind::Unsupported,
            IoErrorKind::OutOfMemory => ErrorKind::OutOfMemory,
            IoErrorKind::Interrupted => ErrorKind::Interrupted,
            IoErrorKind::Other => ErrorKind::Other,
        }
    }
}

#[cfg(feature = "std")]
impl From<std::io::ErrorKind> for IoErrorKind {
    fn from(e: std::io::ErrorKind) -> Self {
        match e {
            std::io::ErrorKind::NotFound => IoErrorKind::NotFound,
            std::io::ErrorKind::PermissionDenied => IoErrorKind::PermissionDenied,
            std::io::ErrorKind::AlreadyExists => IoErrorKind::AlreadyExists,
            std::io::ErrorKind::InvalidInput => IoErrorKind::InvalidInput,
            std::io::ErrorKind::InvalidData => IoErrorKind::InvalidData,
            std::io::ErrorKind::Unsupported => IoErrorKind::Unsupported,
            std::io::ErrorKind::OutOfMemory => IoErrorKind::OutOfMemory,
            std::io::ErrorKind::Interrupted => IoErrorKind::Interrupted,
            _ => IoErrorKind::Other,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PipeData {
    pub rx_ptr: usize,
    pub rx_len: usize,
    pub tx_ptr: usize,
    pub tx_len: usize,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FileMode {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub append: bool,
    pub truncate: bool,
}
