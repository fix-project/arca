use crate::*;

unsafe extern "C" {
    fn util_allocate_memory(index: u32) -> *mut Memory;
    static UTIL_NUM_MEMORIES: u32;
}
static mut POSITION: u32 = 0;
const PAGE_SIZE: usize = 65536;

#[repr(transparent)]
pub struct Memory(u32);

impl Memory {
    #[doc(hidden)]
    pub const EMPTY: Self = Self(0);

    pub fn new(index: u32) -> Result<&'static mut Self, Error> {
        let slot = unsafe { util_allocate_memory(index) };
        if slot.is_null() {
            return Err(Error::Unavailable);
        }
        let memory = unsafe { &mut *slot };
        memory.0 = index;
        Ok(memory)
    }

    pub fn next() -> Result<&'static mut Self, Error> {
        unsafe {
            while POSITION < UTIL_NUM_MEMORIES {
                POSITION += 1;
                if let Ok(memory) = Memory::new(POSITION) {
                    return Ok(memory);
                }
            }
        }
        Err(Error::AllOccupied)
    }

    /// Calls the fixshell's create_blob function when resolved.
    /// Borrows the memory until the handle is consumed.
    ///
    /// # Safety
    ///
    /// `length` must be <= size() * PAGE_SIZE
    pub unsafe fn create_blob(&self, length: usize) -> CreateBlob<'_> {
        CreateBlob {
            memory_index: self.0,
            length,
            source: PhantomData,
        }
    }

    /// Fills the destination slice by copying bytes from the memory
    ///
    /// # Safety
    ///
    /// The `destination` slice's length must be <= size() * PAGE_SIZE
    pub unsafe fn read(&self, destination: &mut [u8]) {
        unsafe { wasm_memory_read(self.0, destination.as_mut_ptr() as u32, destination.len()) }
    }

    /// Copies the bytes from the source slice into the memory
    ///
    /// # Safety
    ///
    /// The `source` slice's length must be <= size() * PAGE_SIZE
    pub unsafe fn write(&mut self, source: &[u8]) {
        unsafe { wasm_memory_write(self.0, source.as_ptr() as u32, source.len()) }
    }

    /// Calls the fixshell's attach_blob after resolving the provided `operation`
    ///
    /// # Safety
    ///
    /// `operation` must resolve to a blob
    pub unsafe fn attach_blob<T: Resolve>(&mut self, operation: T) {
        unsafe { attach_blob(self.0, operation) }
    }

    pub fn size(&self) -> usize {
        unsafe { wasm_memory_size(self.0) }
    }

    pub fn grow(&mut self, num_pages: usize) -> usize {
        unsafe { wasm_memory_grow(self.0, num_pages) }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<&'static mut Self, Error> {
        let memory = Memory::next()?;
        let mapped = memory.size();
        let required = bytes.len().div_ceil(PAGE_SIZE);
        if required > mapped && memory.grow(required - mapped) == usize::MAX {
            return Err(Error::GrowFailed);
        }
        unsafe { memory.write(bytes) };
        Ok(memory)
    }

    pub fn from_blob<T: Resolve>(operation: T) -> Result<&'static mut Self, Error> {
        let memory = Memory::next()?;
        let mapped = memory.size();
        let required = operation.len().div_ceil(PAGE_SIZE);
        if required > mapped && memory.grow(required - mapped) == usize::MAX {
            return Err(Error::GrowFailed);
        }
        unsafe { memory.attach_blob(operation) };
        Ok(memory)
    }

    pub fn to_bytes(&self, length: usize) -> Result<Vec<u8>, Error> {
        if length.div_ceil(PAGE_SIZE) > self.size() {
            return Err(Error::OutOfBounds);
        }
        let mut bytes = alloc::vec![0; length];
        unsafe { self.read(&mut bytes) };
        Ok(bytes)
    }

    pub fn to_blob(&self, length: usize) -> Result<CreateBlob<'_>, Error> {
        if length.div_ceil(PAGE_SIZE) > self.size() {
            return Err(Error::OutOfBounds);
        }
        Ok(unsafe { self.create_blob(length) })
    }
}
