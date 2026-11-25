extern crate alloc;

use super::*;

pub mod arca;

pub(crate) fn memory_limit(
    storage: &dyn Storage,
    fields: &[Handle],
) -> Result<usize, ExecutionError> {
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
    InvalidMemoryLimit,
    Admission(kernel::memory::AdmissionError),
    Storage(StorageError),
}

impl From<StorageError> for ExecutionError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl From<arca::trust::TrustError> for ExecutionError {
    fn from(error: arca::trust::TrustError) -> Self {
        match error {
            arca::trust::TrustError::Storage(error) => Self::Storage(error),
            _ => Self::InvalidApplication,
        }
    }
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
            Self::Storage(error) => alloc::format!("could not read application data: {error:?}"),
            Self::Admission(error) => alloc::format!(
                "memory admission rejected: requested {} bytes, {} bytes available; retry when capacity is available",
                error.required,
                error.available
            ),
            Self::InvalidApplication => match callable {
                Some(callable) => alloc::format!(
                    "cannot apply combination {}: callable {} is not a function",
                    name(combination),
                    name(callable)
                ),
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
