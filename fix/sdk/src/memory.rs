use crate::{Blob, Error, Handle, Object, Tree, Value, ffi};
use core::marker::PhantomData;

pub enum Unmapped {}
pub enum ReadOnly {}
pub enum ReadWrite {}

pub struct Memory<S = Unmapped> {
    index: u32,
    length: usize,
    state: PhantomData<S>,
}

impl<S> Memory<S> {
    fn transition<T>(self, length: usize) -> Memory<T> {
        let index = self.index;
        core::mem::forget(self);
        Memory {
            index,
            length,
            state: PhantomData,
        }
    }
    pub fn index(&self) -> u32 {
        self.index
    }
    pub fn detach(self) -> Memory<Unmapped> {
        unsafe { ffi::detach_blob(self.index, 0) };
        self.transition(0)
    }
}
impl Memory<Unmapped> {
    /// # Safety
    /// The declared Memory must be empty and exclusively owned by this object.
    /// Memories 0 and 1 belong to Rust and the SDK's Blob staging area.
    pub unsafe fn from_index(index: u32) -> Self {
        assert!(index >= 2);
        Self {
            index,
            length: 0,
            state: PhantomData,
        }
    }
    pub fn attach_tree_bytes(
        self,
        tree: &impl Value<Type = Object<Tree>>,
    ) -> Result<Memory<ReadOnly>, Error> {
        let length = usize::try_from(tree.len()?)?
            .checked_mul(32)
            .ok_or(Error::OutOfBounds)?;
        unsafe { ffi::attach_tree_bytes(tree.handle().as_raw(), self.index) };
        Ok(self.transition(length))
    }
    pub fn initialize(self, pages: u32) -> Result<Memory<ReadWrite>, Error> {
        let length = (pages as usize)
            .checked_mul(65536)
            .ok_or(Error::OutOfBounds)?;
        let old = unsafe { memory_grow(self.index, pages) };
        if old == u32::MAX {
            return Err(Error::GrowFailed);
        }
        assert_eq!(old, 0, "expected unmapped memory");
        Ok(self.transition(length))
    }
    pub fn attach(self, blob: &impl Value<Type = Object<Blob>>) -> Memory<ReadOnly> {
        let length = usize::try_from(unsafe { ffi::len(blob.handle().as_raw()) })
            .expect("Blob exceeds address space");
        unsafe { ffi::attach_blob(self.index, blob.handle().as_raw()) };
        self.transition(length)
    }
}
impl Memory<ReadOnly> {
    pub fn make_mut(self) -> Memory<ReadWrite> {
        unsafe { ffi::detach_blob(self.index, 1) };
        let length = self.length.next_multiple_of(65536);
        self.transition(length)
    }
}
macro_rules! readable {
    ($($state:ty),*) => {$(impl Memory<$state> {
        pub fn len(&self)->usize { self.length }
        pub fn is_empty(&self)->bool { self.length==0 }
        pub fn read_into(&self, offset:usize, bytes:&mut [u8])->Result<(),Error> {
            if offset>self.length || bytes.len()>self.length-offset { return Err(Error::OutOfBounds); }
            unsafe { memory_copy_to_0(self.index,bytes.as_mut_ptr() as u32,offset.try_into()?,bytes.len().try_into()?) };
            Ok(())
        }
        #[cfg(feature="alloc")]
        pub fn read(&self)->alloc::vec::Vec<u8> {
            let mut bytes=alloc::vec![0;self.length]; self.read_into(0,&mut bytes).unwrap(); bytes
        }
    })*};
}
readable!(ReadOnly, ReadWrite);
impl Memory<ReadWrite> {
    pub fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Error> {
        if offset > self.length || bytes.len() > self.length - offset {
            return Err(Error::OutOfBounds);
        }
        unsafe {
            memory_copy_from_0(
                self.index,
                offset.try_into()?,
                bytes.as_ptr() as u32,
                bytes.len().try_into()?,
            )
        };
        Ok(())
    }
    pub fn grow(&mut self, pages: u32) -> Result<(), Error> {
        let additional = (pages as usize)
            .checked_mul(65536)
            .ok_or(Error::OutOfBounds)?;
        let length = self
            .length
            .checked_add(additional)
            .ok_or(Error::OutOfBounds)?;
        if unsafe { memory_grow(self.index, pages) } == u32::MAX {
            return Err(Error::GrowFailed);
        }
        self.length = length;
        Ok(())
    }
    pub fn create_blob(&self, length: usize) -> Result<fix::Blob<crate::backend::Shell>, Error> {
        if length > self.length {
            return Err(Error::OutOfBounds);
        }
        let value = unsafe { Handle::from_raw(ffi::create_blob(self.index, length.try_into()?)) };
        Ok(fix::Blob::try_from(fix::Value::from_inner(value))?)
    }
}
impl<S> Drop for Memory<S> {
    fn drop(&mut self) {
        unsafe { ffi::detach_blob(self.index, 0) }
    }
}

#[link(wasm_import_module = "__wasm_postprocessor")]
unsafe extern "C" {
    fn memory_grow(index: u32, pages: u32) -> u32;
    fn memory_copy_to_0(index: u32, destination: u32, source: u32, length: u32);
    fn memory_copy_from_0(index: u32, destination: u32, source: u32, length: u32);
}
