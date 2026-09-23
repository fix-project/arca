use crate::*;
use alloc::boxed::Box;
use core::{
    ops::Range,
    sync::atomic::{AtomicBool, Ordering},
};

const NUM_MEMORIES: usize = 32;
const NUM_TABLES: usize = 32;
const PAGE_SIZE: usize = 65536;

pub trait Resource: Sized + 'static {
    const RESOURCE_INDICES: Range<u32>;
    // Unit for resource (1 element for tables and PAGE_SIZE for memories)
    const RESOURCE_UNIT: usize;
    type Create<'a>: Resolve;

    fn new(index: u32) -> Result<&'static mut Self, Error>;
    fn size(&self) -> usize;
    fn grow(&mut self, delta: usize) -> usize;

    /// Calls the fixshell's attach function after resolving the provided `operation`
    ///
    /// # Safety
    ///
    /// `operation` must resolve to a blob for memories or a tree for tables
    unsafe fn attach<T: Resolve>(&mut self, operation: T);

    /// Calls the fixshell's create function over the first `length` elements when resolved.
    /// Borrows the resource until the handle is consumed.
    ///
    /// # Safety
    ///
    /// `length` must be <= size() * UNIT
    unsafe fn create(&self, length: usize) -> Self::Create<'_>;

    fn occupy(occupied: &[AtomicBool], index: u32) -> Result<(), Error> {
        // can't get undeclared resource
        if !Self::RESOURCE_INDICES.contains(&index) {
            return Err(Error::Unavailable);
        }
        // can't get already occupied resource
        let slot = (index - Self::RESOURCE_INDICES.start) as usize;
        if occupied[slot].swap(true, Ordering::Relaxed) {
            return Err(Error::Unavailable);
        }
        Ok(())
    }

    fn next() -> Result<&'static mut Self, Error> {
        for index in Self::RESOURCE_INDICES {
            if let Ok(resource) = Self::new(index) {
                return Ok(resource);
            }
        }
        Err(Error::AllOccupied)
    }

    /// Allocates a resource grows it to `length` elements
    fn with_capacity(length: usize) -> Result<&'static mut Self, Error> {
        let resource = Self::next()?;
        let mapped = resource.size();
        let required = length.div_ceil(Self::RESOURCE_UNIT);
        if required > mapped && resource.grow(required - mapped) == usize::MAX {
            return Err(Error::GrowFailed);
        }
        Ok(resource)
    }

    fn from_handle<T: Resolve>(operation: T) -> Result<&'static mut Self, Error> {
        let resource = Self::with_capacity(operation.len())?;
        unsafe { resource.attach(operation) };
        Ok(resource)
    }

    fn to_handle(&self, length: usize) -> Result<Self::Create<'_>, Error> {
        self.check_bounds(length)?;
        Ok(unsafe { self.create(length) })
    }

    fn check_bounds(&self, length: usize) -> Result<(), Error> {
        if length.div_ceil(Self::RESOURCE_UNIT) > self.size() {
            return Err(Error::OutOfBounds);
        }
        Ok(())
    }
}

pub struct Memory(u32);

impl Resource for Memory {
    const RESOURCE_INDICES: Range<u32> = 1..NUM_MEMORIES as u32 + 1;
    const RESOURCE_UNIT: usize = PAGE_SIZE;
    type Create<'a> = CreateBlob<'a>;

    fn new(index: u32) -> Result<&'static mut Self, Error> {
        static OCCUPIED: [AtomicBool; NUM_MEMORIES] =
            [const { AtomicBool::new(false) }; NUM_MEMORIES];
        Self::occupy(&OCCUPIED, index)?;
        Ok(Box::leak(Box::new(Self(index))))
    }

    fn size(&self) -> usize {
        unsafe { wasm_memory_size(self.0) }
    }

    fn grow(&mut self, delta: usize) -> usize {
        unsafe { wasm_memory_grow(self.0, delta) }
    }

    unsafe fn attach<T: Resolve>(&mut self, operation: T) {
        unsafe { attach_blob(self.0, operation) }
    }

    unsafe fn create(&self, length: usize) -> CreateBlob<'_> {
        CreateBlob {
            memory_index: self.0,
            length,
            source: PhantomData,
        }
    }
}

impl Memory {
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

    pub fn from_bytes(bytes: &[u8]) -> Result<&'static mut Self, Error> {
        let memory = Memory::with_capacity(bytes.len())?;
        unsafe { memory.write(bytes) };
        Ok(memory)
    }

    pub fn to_bytes(&self, length: usize) -> Result<Vec<u8>, Error> {
        self.check_bounds(length)?;
        let mut bytes = alloc::vec![0; length];
        unsafe { self.read(&mut bytes) };
        Ok(bytes)
    }
}

pub struct Table(u32);

impl Resource for Table {
    const RESOURCE_INDICES: Range<u32> = 0..NUM_TABLES as u32;
    const RESOURCE_UNIT: usize = 1;
    type Create<'a> = CreateTree<'a>;

    fn new(index: u32) -> Result<&'static mut Self, Error> {
        static OCCUPIED: [AtomicBool; NUM_TABLES] = [const { AtomicBool::new(false) }; NUM_TABLES];
        Self::occupy(&OCCUPIED, index)?;
        Ok(Box::leak(Box::new(Self(index))))
    }

    fn size(&self) -> usize {
        unsafe { wasm_table_size(self.0) }
    }

    fn grow(&mut self, delta: usize) -> usize {
        unsafe { wasm_table_grow(self.0, delta) }
    }

    unsafe fn attach<T: Resolve>(&mut self, operation: T) {
        unsafe { attach_tree(self.0, operation) }
    }

    unsafe fn create(&self, length: usize) -> CreateTree<'_> {
        CreateTree {
            table_index: self.0,
            length,
            source: PhantomData,
        }
    }
}

impl Table {
    /// Gets the externref with index `entry_index` from the table when resolved
    ///
    /// # Safety
    ///
    /// `entry_index` must be < size()
    pub unsafe fn get(&self, entry_index: usize) -> TableGet<'_> {
        TableGet {
            table_index: self.0,
            entry_index,
            source: PhantomData,
        }
    }

    /// Sets index `entry_index` in the table with the externref resolved from
    /// `operation`
    ///
    /// # Safety
    ///
    /// `entry_index` must be < size()
    pub unsafe fn set<T: Resolve>(&mut self, entry_index: usize, operation: T) {
        unsafe { table_set(self.0, entry_index, operation) }
    }

    pub fn from_entries<T: Resolve>(entries: &[T]) -> Result<&'static mut Self, Error> {
        let table = Table::with_capacity(entries.len())?;
        for (entry_index, operation) in entries.iter().enumerate() {
            unsafe { table.set(entry_index, *operation) };
        }
        Ok(table)
    }

    pub fn to_entries(&self, length: usize) -> Result<Vec<TableGet<'_>>, Error> {
        self.check_bounds(length)?;
        let mut entries = Vec::with_capacity(length);
        for entry_index in 0..length {
            entries.push(unsafe { self.get(entry_index) });
        }
        Ok(entries)
    }
}
