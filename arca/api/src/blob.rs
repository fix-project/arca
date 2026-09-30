#[cfg(feature = "alloc")]
use alloc::string::String;

use super::prelude::*;

impl<R: Runtime> Blob<R> {
    pub fn new(data: impl AsRef<[u8]>) -> Self {
        R::create_blob(data.as_ref())
    }
}

impl<R: Runtime> Blob<R> {
    pub fn read(&self, offset: usize, buf: &mut [u8]) -> usize {
        R::read_blob(self, offset, buf)
    }

    #[cfg(feature = "alloc")]
    pub fn with_ref<T>(&self, f: impl FnOnce(&[u8]) -> T) -> T {
        R::with_blob_as_ref(self, f)
    }
}

impl<R: Runtime> From<&[u8]> for Blob<R> {
    fn from(value: &[u8]) -> Self {
        Self::new(value)
    }
}

impl<R: Runtime> From<&str> for Blob<R> {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[cfg(feature = "alloc")]
impl<R: Runtime> From<String> for Blob<R> {
    fn from(value: String) -> Self {
        Self::new(&value)
    }
}
