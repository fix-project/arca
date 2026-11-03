#[cfg(not(target_os = "linux"))]
mod arca;
#[cfg(not(target_os = "linux"))]
pub use arca::*;

#[cfg(target_os = "linux")]
mod native;
#[cfg(target_os = "linux")]
pub use native::*;

impl ImmutableBytes {
    pub(crate) fn read_handles(
        &self,
    ) -> Result<alloc::boxed::Box<[fixhandle::Handle]>, crate::StorageError> {
        use bytes::Buf;
        use fixhandle::{HANDLE_SIZE, Handle};

        if !self.len().is_multiple_of(HANDLE_SIZE) {
            return Err(crate::StorageError::NotFound);
        }
        let mut handles = alloc::vec::Vec::with_capacity(self.len() / HANDLE_SIZE);
        let mut bytes = self.clone().into_buf();
        while bytes.has_remaining() {
            let mut cell = [0; HANDLE_SIZE];
            bytes.copy_to_slice(&mut cell);
            handles
                .push(unsafe { Handle::parse(cell) }.map_err(|_| crate::StorageError::NotFound)?);
        }
        Ok(handles.into_boxed_slice())
    }
}
