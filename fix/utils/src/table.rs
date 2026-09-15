use crate::*;

unsafe extern "C" {
    fn util_allocate_table(index: u32) -> *mut Table;
    static UTIL_NUM_TABLES: u32;
}
static mut POSITION: u32 = 0;

#[repr(transparent)]
pub struct Table(u32);

impl Table {
    #[doc(hidden)]
    pub const EMPTY: Self = Self(0);

    pub fn new(index: u32) -> Result<&'static mut Self, Error> {
        let slot = unsafe { util_allocate_table(index) };
        if slot.is_null() {
            return Err(Error::Unavailable);
        }
        let table = unsafe { &mut *slot };
        table.0 = index;
        Ok(table)
    }

    pub fn next() -> Result<&'static mut Self, Error> {
        unsafe {
            while POSITION < UTIL_NUM_TABLES {
                POSITION += 1;
                if let Ok(table) = Table::new(POSITION) {
                    return Ok(table);
                }
            }
        }
        Err(Error::AllOccupied)
    }

    /// Calls the fixshell's create_tree function when resolved.
    /// Borrows the table until the handle is consumed
    ///
    /// # Safety
    ///
    /// `length` must be <= size()
    pub unsafe fn create_tree(&self, length: usize) -> CreateTree<'_> {
        CreateTree {
            table_index: self.0,
            length,
            source: PhantomData,
        }
    }

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

    /// Calls the fixshell's attach_tree after resolving the provided `operation`
    ///
    /// # Safety
    ///
    /// `operation` must resolve to a tree
    pub unsafe fn attach_tree<T: Resolve>(&mut self, operation: T) {
        unsafe { attach_tree(self.0, operation) }
    }

    pub fn size(&self) -> usize {
        unsafe { wasm_table_size(self.0) }
    }

    pub fn grow(&mut self, entries: usize) -> usize {
        unsafe { wasm_table_grow(self.0, entries) }
    }

    pub fn from_entries<T: Resolve>(entries: &[T]) -> Result<&'static mut Self, Error> {
        let table = Table::next()?;
        let mapped = table.size();
        let required = entries.len();
        if required > mapped && table.grow(required - mapped) == usize::MAX {
            return Err(Error::GrowFailed);
        }
        for (entry_index, operation) in entries.iter().enumerate() {
            unsafe { table.set(entry_index, *operation) };
        }
        Ok(table)
    }

    pub fn from_tree<T: Resolve>(operation: T) -> Result<&'static mut Self, Error> {
        let table = Table::next()?;
        let mapped = table.size();
        let required = operation.len();
        if required > mapped && table.grow(required - mapped) == usize::MAX {
            return Err(Error::GrowFailed);
        }
        unsafe { table.attach_tree(operation) };
        Ok(table)
    }

    pub fn to_entries(&self, length: usize) -> Result<Vec<TableGet<'_>>, Error> {
        if self.size() < length {
            return Err(Error::OutOfBounds);
        }
        let mut entries = Vec::with_capacity(length);
        for entry_index in 0..length {
            entries.push(unsafe { self.get(entry_index) });
        }
        Ok(entries)
    }

    pub fn to_tree(&self, length: usize) -> Result<CreateTree<'_>, Error> {
        if self.size() < length {
            return Err(Error::OutOfBounds);
        }
        Ok(unsafe { self.create_tree(length) })
    }
}
