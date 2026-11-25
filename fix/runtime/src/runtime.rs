extern crate alloc;

use super::*;
use crate::storage::CanonicalHandle;

pub mod arca;

pub(crate) fn memory_limit(
    storage: &dyn Storage,
    combination: Tree,
) -> Result<usize, ExecutionError> {
    let fields = storage
        .get_tree(combination)
        .map_err(|_| ExecutionError::InvalidApplication)?;
    fields
        .first()
        .and_then(|value| Blob::try_from(*value).ok())
        .and_then(|blob| storage.get_blob(blob).ok())
        .and_then(|bytes| <[u8; 8]>::try_from(bytes.as_ref()).ok())
        .and_then(|bytes| usize::try_from(u64::from_le_bytes(bytes)).ok())
        .ok_or(ExecutionError::InvalidMemoryLimit)
}

/// A Fix runtime. A Runtime is effectively a data store that can additionally execute procedures.
pub trait Runtime {
    fn storage(&self) -> &dyn Storage;
    /// Requires a Value combination; returns `[[], Ref([Apply, combination, result])]`, or
    /// fails without a coupon when the combination is not an application.
    fn execute(&self, combination: Tree) -> Result<Tree, ExecutionError>;
}

/// A rejected invocation has no result or Apply coupon. Admission errors are transient;
/// malformed callables and memory declarations must be corrected before retrying.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionError {
    InvalidApplication,
    ApplicationError(CanonicalHandle),
    InvalidExecution(&'static str),
    NativeException(CanonicalHandle),
    InvalidMemoryLimit,
    Admission(kernel::memory::AdmissionError),
}

impl ExecutionError {
    /// The `Fix.error` request reporting the failure.
    pub fn request(self, storage: &dyn Storage, combination: Handle) -> Tree {
        let name = |value| -> alloc::string::String {
            match storage.canonicalize(value) {
                Ok(value) => alloc::format!("{}", Handle::from(value)),
                Err(error) => alloc::format!("<canonical name unavailable: {error:?}>"),
            }
        };
        let callable = Tree::try_from(combination)
            .ok()
            .and_then(|tree| storage.get_tree(tree).ok())
            .and_then(|fields| fields.get(1).copied());
        let message = match self {
            Self::InvalidMemoryLimit => alloc::format!(
                "combination {} requires an eight-byte memory limit",
                name(combination)
            ),
            Self::Admission(error) => alloc::format!(
                "memory admission rejected: requested {} bytes, {} bytes available; retry when capacity is available",
                error.required,
                error.available
            ),
            Self::InvalidExecution(reason) => {
                alloc::format!(
                    "combination {} failed during execution: {reason}",
                    name(combination)
                )
            }
            Self::ApplicationError(error) => {
                let error = Handle::from(error);
                let detail = Tree::try_from(error)
                    .ok()
                    .and_then(|tag| storage.get_tree(tag).ok())
                    .and_then(|fields| fields.get(1).copied())
                    .and_then(|payload| Blob::try_from(payload).ok())
                    .and_then(|blob| storage.get_blob(blob).ok());
                match detail {
                    Some(bytes) => alloc::format!(
                        "cannot prepare executable for combination {}: {}",
                        name(combination),
                        alloc::string::String::from_utf8_lossy(&bytes)
                    ),
                    None => alloc::format!(
                        "cannot prepare executable for combination {}: {}",
                        name(combination),
                        name(error)
                    ),
                }
            }
            Self::NativeException(reason) => {
                let reason = Blob::try_from(Handle::from(reason))
                    .ok()
                    .and_then(|reason| storage.get_blob(reason).ok());
                let reason = reason
                    .as_deref()
                    .and_then(|bytes| core::str::from_utf8(bytes).ok())
                    .unwrap_or("unknown exception");
                alloc::format!(
                    "combination {} had a native CPU exception: {reason}",
                    name(combination)
                )
            }
            Self::InvalidApplication => match callable {
                Some(callable) => {
                    let mut message = alloc::format!(
                        "cannot apply combination {}: callable {} is not a function",
                        name(combination),
                        name(callable)
                    );
                    if let Ok(tag) = Tree::try_from(callable)
                        && tag.is_tag()
                        && tag.len() == 3
                        && let Ok(fields) = storage.get_tree(tag)
                        && let Ok(kind) = Blob::try_from(fields[2])
                        && let Some(bytes) = kind.literal_bytes()
                        && let Ok(kind) = core::str::from_utf8(bytes)
                    {
                        use core::fmt::Write as _;
                        let _ = write!(message, " (tag kind {kind:?})");
                    }
                    message
                }
                None => alloc::format!(
                    "cannot apply combination {}: no callable at index 1",
                    name(combination)
                ),
            },
        };
        storage.add_tree(&[
            storage.add_blob(b"Fix.error").into(),
            storage.add_blob(message.as_bytes()).into(),
        ])
    }
}
