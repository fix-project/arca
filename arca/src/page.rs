use super::prelude::*;

impl<R: Runtime> Page<R> {
    pub fn new(len: usize) -> Self {
        R::create_page(len)
    }

    pub fn read(&self, offset: usize, buf: &mut [u8]) -> usize {
        R::read_page(self, offset, buf)
    }

    pub fn write(&mut self, offset: usize, buf: &[u8]) -> usize {
        R::write_page(self, offset, buf)
    }

    #[cfg(feature = "alloc")]
    pub fn with_ref<T>(&self, f: impl FnOnce(&[u8]) -> T) -> T {
        R::with_page_as_ref(self, f)
    }

    #[cfg(feature = "alloc")]
    pub fn with_mut<T>(&mut self, f: impl FnOnce(&mut [u8]) -> T) -> T {
        R::with_page_as_mut(self, f)
    }
}
