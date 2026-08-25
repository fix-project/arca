use crate::*;

unsafe extern "C" {
    fn fix_allocate_memory(index: u16) -> *mut Memory;
    static FIX_NUM_MEMORIES: u16;
}
static mut POSITION: u16 = 0;
const PAGE_SIZE: usize = 65536;

pub fn fix_next_memory() -> Result<&'static mut Memory, FixError> {
    unsafe {
        while POSITION < FIX_NUM_MEMORIES {
            POSITION += 1;
            if let Ok(memory) = Memory::new(POSITION) {
                return Ok(memory);
            }
        }
    }
    Err(FixError::AllOcuppied)
}

#[repr(transparent)]
pub struct Memory(u16);

impl Memory {
    #[doc(hidden)]
    pub const EMPTY: Self = Self(0);

    pub fn new(index: u16) -> Result<&'static mut Self, FixError> {
        let slot = unsafe { fix_allocate_memory(index) };
        if slot.is_null() {
            return Err(FixError::Unavailable);
        }
        let memory = unsafe { &mut *slot };
        memory.0 = index;
        Ok(memory)
    }

    /// Calls the fixshell's create_blob function when resolved.
    /// Borrows the memory until the handle is consumed.
    ///
    /// # Safety
    ///
    /// `length` must be <= size() * PAGE_SIZE
    pub unsafe fn create_blob(&self, length: usize) -> RustHandle<'_> {
        RustHandle::new(Handle::Object(Object::Blob(Blob::Blob(unsafe {
            BlobName::new(encode_args(Producer::CreateBlob, self.0, length))
        }))))
    }

    /// Fills the destination slice by copying bytes from the memory
    ///
    /// # Safety
    ///
    /// The `destination` slice's length must be <= size() * PAGE_SIZE
    pub unsafe fn read(&self, destination: &mut [u8]) {
        unsafe {
            fix_memory_read(
                self.0 as u32,
                destination.as_mut_ptr() as u32,
                destination.len(),
            )
        }
    }

    /// Copies the bytes from the source slice into the memory
    ///
    /// # Safety
    ///
    /// The `source` slice's length must be <= size() * PAGE_SIZE
    pub unsafe fn write(&mut self, source: &[u8]) {
        unsafe { fix_memory_write(self.0 as u32, source.as_ptr() as u32, source.len()) }
    }

    /// Calls the fixshell's attach_blob after resolving the provided `handle`
    ///
    /// # Safety
    ///
    /// `handle` must refer to a blob
    pub unsafe fn attach_blob(&mut self, handle: RustHandle<'_>) {
        unsafe { fix_attach_blob(self.0 as u32, &handle.raw_handle) }
    }

    pub fn size(&self) -> usize {
        unsafe { fix_memory_size(self.0 as u32) }
    }

    pub fn grow(&mut self, num_pages: usize) -> usize {
        unsafe { fix_memory_grow(self.0 as u32, num_pages) }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<&'static mut Self, FixError> {
        let memory = fix_next_memory()?;
        let mapped = memory.size();
        let required = bytes.len().div_ceil(PAGE_SIZE);
        if required > mapped && memory.grow(required - mapped) == usize::MAX {
            return Err(FixError::GrowFailed);
        }
        unsafe { memory.write(bytes) };
        Ok(memory)
    }

    pub fn from_blob(handle: RustHandle<'_>) -> Result<&'static mut Self, FixError> {
        let memory = fix_next_memory()?;
        let mapped = memory.size();
        let required = handle.len().div_ceil(PAGE_SIZE);
        if required > mapped && memory.grow(required - mapped) == usize::MAX {
            return Err(FixError::GrowFailed);
        }
        unsafe { memory.attach_blob(handle) };
        Ok(memory)
    }

    pub fn to_bytes(&self, length: usize) -> Result<Vec<u8>, FixError> {
        if length > self.size() * PAGE_SIZE {
            return Err(FixError::OutOfBounds);
        }
        let mut bytes = alloc::vec![0; length];
        unsafe { self.read(&mut bytes) };
        Ok(bytes)
    }

    pub fn to_blob(&self, length: usize) -> Result<RustHandle<'_>, FixError> {
        if length > self.size() * PAGE_SIZE {
            return Err(FixError::OutOfBounds);
        }
        Ok(unsafe { self.create_blob(length) })
    }
}
