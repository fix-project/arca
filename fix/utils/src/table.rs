use crate::*;

unsafe extern "C" {
    fn util_allocate_table(index: u16) -> *mut Table;
    static UTIL_NUM_TABLES: u16;
}
static mut POSITION: u16 = 0;

#[repr(transparent)]
pub struct Table(u16);

impl Table {
    #[doc(hidden)]
    pub const EMPTY: Self = Self(0);

    pub fn new(index: u16) -> Result<&'static mut Self, Error> {
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
    pub unsafe fn create_tree(&self, length: usize) -> RustHandle<'_> {
        RustHandle::new(Handle::Object(Object::Tree(Tree::Tree(unsafe {
            TreeName::new(encode_args(Producer::CreateTree, self.0, length))
        }))))
    }

    /// Gets the externref with index `entry` from the table when resolved
    ///
    /// # Safety
    ///
    /// `entry` must be < size()
    pub unsafe fn get(&self, entry: usize) -> RustHandle<'_> {
        RustHandle::new(Handle::Object(Object::Tree(Tree::Tree(unsafe {
            TreeName::new(encode_args(Producer::TableGet, self.0, entry))
        }))))
    }

    /// Sets index `entry` in the table with the externref resolved from `handle`
    ///
    /// # Safety
    ///
    /// `entry` must be < size()
    pub unsafe fn set(&mut self, entry: usize, handle: RustHandle<'_>) {
        unsafe { util_table_set(self.0 as u32, entry, &handle.raw_handle) }
    }

    /// Calls the fixshell's attach_tree after resolving the provided `handle`
    ///
    /// # Safety
    ///
    /// `handle` must refer to a tree
    pub unsafe fn attach_tree(&mut self, handle: RustHandle<'_>) {
        unsafe { util_attach_tree(self.0 as u32, &handle.raw_handle) }
    }

    pub fn size(&self) -> usize {
        unsafe { wasm_table_size(self.0 as u32) }
    }

    pub fn grow(&mut self, entries: usize) -> usize {
        unsafe { wasm_table_grow(self.0 as u32, entries) }
    }

    pub fn from_entries(entries: &[RustHandle<'_>]) -> Result<&'static mut Self, Error> {
        let table = Table::next()?;
        let mapped = table.size();
        let required = entries.len();
        if required > mapped && table.grow(required - mapped) == usize::MAX {
            return Err(Error::GrowFailed);
        }
        for (entry, handle) in entries.iter().enumerate() {
            unsafe { table.set(entry, *handle) };
        }
        Ok(table)
    }

    pub fn from_tree(handle: RustHandle<'_>) -> Result<&'static mut Self, Error> {
        let table = Table::next()?;
        let mapped = table.size();
        let required = handle.len();
        if required > mapped && table.grow(required - mapped) == usize::MAX {
            return Err(Error::GrowFailed);
        }
        unsafe { table.attach_tree(handle) };
        Ok(table)
    }

    pub fn to_entries(&self, length: usize) -> Result<Vec<RustHandle<'_>>, Error> {
        if self.size() < length {
            return Err(Error::OutOfBounds);
        }
        let mut entries = Vec::with_capacity(length);
        for entry in 0..length {
            entries.push(unsafe { self.get(entry) });
        }
        Ok(entries)
    }

    pub fn to_tree(&self, length: usize) -> Result<RustHandle<'_>, Error> {
        if self.size() < length {
            return Err(Error::OutOfBounds);
        }
        Ok(unsafe { self.create_tree(length) })
    }
}
