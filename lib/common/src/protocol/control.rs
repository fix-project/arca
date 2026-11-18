extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::buffer::Region;
use serde::{Deserialize, Serialize};

pub use embedded_io::ErrorKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EvaluationOperation {
    Evaluate,
    Think,
    Force,
    Apply,
    Select,
    Load,
}

impl core::fmt::Display for EvaluationOperation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Evaluate => "Evaluate",
            Self::Think => "Think",
            Self::Force => "Force",
            Self::Apply => "Apply",
            Self::Select => "Select",
            Self::Load => "Load",
        })
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EvaluationSnapshot {
    pub machines: Vec<MachineProgress>,
    pub steps: Vec<EvaluationStep>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MachineProgress {
    pub location: Vec<String>,
    pub availability: Option<RemoteAvailability>,
    pub used: bool,
    pub runtime: Option<RuntimeStats>,
    pub tasks: Vec<EvaluationTask>,
    pub labels: Vec<([u8; 32], String)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RemoteAvailability {
    Configured,
    Connecting,
    Available {
        workers: u64,
        memory_bytes: Option<u64>,
    },
    Unavailable(String),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RuntimeStats {
    pub elapsed_ns: u64,
    pub workers: u64,
    pub busy_workers: u64,
    pub queued: u64,
    pub peak_busy_workers: u64,
    pub peak_queued: u64,
    pub completed: u64,
    pub privileged_completed: u64,
    pub cpu_ns: Option<u64>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub peak_memory: u64,
    pub memory_used: u64,
    pub memory_total: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvaluationStep {
    pub location: Vec<String>,
    pub task: EvaluationTask,
    pub elapsed_ns: u64,
    pub success: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EvaluationTask {
    pub id: [u8; 32],
    pub title: String,
    pub operation: Option<EvaluationOperation>,
    pub transfer: Option<(Vec<String>, Vec<String>)>,
    pub name: String,
    pub progress: Option<(u64, u64)>,
    pub children: Vec<([u8; 32], Option<EvaluationOperation>)>,
}

/// A control message and the id that pairs a response with its request.
#[derive(Debug, Serialize, Deserialize)]
pub struct Tagged<T> {
    pub id: u64,
    pub message: T,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request<B = Region> {
    GetArgs,
    ObserveEvaluation,
    CpuTime,
    EvaluationStep(EvaluationStep),
    EvaluationProgress(EvaluationSnapshot),
    Sleep(u64),
    TaskAdvice {
        task: [u8; 32],
        operation: Option<EvaluationOperation>,
        key: Vec<u8>,
        payload: Vec<u8>,
    },
    TreeProgress {
        task: [u8; 32],
        children: Vec<[u8; 32]>,
        completed: u64,
        total: u64,
    },
    TaskFinished {
        task: [u8; 32],
        operation: Option<EvaluationOperation>,
    },
    Exit(i32),
    Open(String, FileMode),
    Mkdir(String),
    IsDirectory(String),
    Canonicalize(String),
    SetCurrentDirectory(String),
    Rename(String, String),
    Symlink(String, String),
    ReadLink(String),
    RemoveFile(String),
    Stdio(StandardStream),
    Listen {
        ip: [u8; 4],
        port: u16,
    },
    Connect {
        host: String,
        port: u16,
    },
    OpenUri(String),
    StdioStream,
    SshConnect {
        host: String,
        repository: String,
        scheduler: Option<String>,
        collector: Option<String>,
    },
    ReadDirectory(String),
    Exists(String),
    /// Returns the file's contents as [`Response::Bytes`].
    ReadFile(String),
    WriteFile(String, B, FileMode),
    ReadDirectoryUri(String),
    SshSign {
        identity: String,
        namespace: String,
        message: B,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Args(Vec<String>),
    Pipe(PipeData),
    Ack,
    Bool(bool),
    CpuTime(Option<u64>),
    Path(String),
    Err(IoErrorKind),
    Names(Vec<String>),
    Length(usize),
    /// A Buddy allocation whose ownership passes to the receiver.
    Bytes(Region),
    DirectoryEntries(Vec<DirectoryEntry>),
    Signature(String),
    SignatureError(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirectoryEntry {
    pub name: Vec<u8>,
    pub kind: DirectoryEntryKind,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DirectoryEntryKind {
    File,
    Directory,
    Symlink(Vec<u8>),
    Unsupported,
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
pub struct VMToHostDoorBellData {
    pub datamatch: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PipeData {
    pub rx_ptr: usize,
    pub rx_len: usize,
    pub tx_ptr: usize,
    pub tx_len: usize,
    pub rx_avail: VMToHostDoorBellData,
    pub tx_avail: VMToHostDoorBellData,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FileMode {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub create_new: bool,
    pub append: bool,
    pub truncate: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum StandardStream {
    Stdin,
    Stdout,
    Stderr,
}
