use super::*;

unsafe extern "C" {
    fn fix_sdk_attach_table(index: u32) -> u32;
    fn fix_sdk_attach_memory(index: u32) -> u32;
    fn fix_sdk_attach_tree_bytes(index: u32) -> u32;
    fn fix_sdk_detach_table(index: u32);
    fn fix_sdk_detach_memory(index: u32);
    fn fix_sdk_table_entry(table: u32, index: u32);
    fn fix_sdk_memory_read(memory: u32, offset: u32, destination: *mut u8, length: u32);
}

pub struct Table {
    index: u32,
}
pub struct Memory {
    index: u32,
}
pub struct AttachedTree<'a> {
    table: &'a mut Table,
    len: usize,
}
pub struct AttachedBlob<'a> {
    memory: &'a mut Memory,
    len: usize,
}
#[derive(Clone, Copy)]
pub struct AttachedElement<'a> {
    tree: &'a AttachedTree<'a>,
    index: usize,
}

impl Table {
    /// # Safety
    /// The index must identify a declared externref Table, exclusively owned by
    /// this object. Tables 0–2 are reserved by the Rust SDK.
    pub unsafe fn from_index(index: u32) -> Self {
        assert!(index >= 3);
        Self { index }
    }
    pub fn attach(
        &mut self,
        value: &impl Value<Type = Object<Tree>>,
    ) -> Result<AttachedTree<'_>, Error> {
        let len = usize::try_from(value.len()?)?;
        value.build_at(None)?;
        status(unsafe { fix_sdk_attach_table(self.index) })?;
        Ok(AttachedTree { table: self, len })
    }
}
impl Memory {
    /// # Safety
    /// The index must identify a declared Memory, exclusively owned by this
    /// object. Memories 0–2 are reserved by the Rust SDK.
    pub unsafe fn from_index(index: u32) -> Self {
        assert!(index >= 3);
        Self { index }
    }
    pub fn attach(
        &mut self,
        value: &impl Value<Type = Object<Blob>>,
    ) -> Result<AttachedBlob<'_>, Error> {
        let len = usize::try_from(value.len()?)?;
        value.build_at(None)?;
        status(unsafe { fix_sdk_attach_memory(self.index) })?;
        Ok(AttachedBlob { memory: self, len })
    }
    pub fn attach_tree_bytes(
        &mut self,
        value: &impl Value<Type = Object<Tree>>,
    ) -> Result<AttachedBlob<'_>, Error> {
        let len = usize::try_from(value.len()?)?
            .checked_mul(32)
            .ok_or(Error::OutOfBounds)?;
        value.build_at(None)?;
        status(unsafe { fix_sdk_attach_tree_bytes(self.index) })?;
        Ok(AttachedBlob { memory: self, len })
    }
}
impl AttachedTree<'_> {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn get(&self, index: usize) -> Handle<Any, AttachedElement<'_>> {
        Handle::new(AttachedElement { tree: self, index })
    }
    pub fn iter(&self) -> impl Iterator<Item = Handle<Any, AttachedElement<'_>>> + Clone {
        (0..self.len).map(|index| self.get(index))
    }
}
impl Lookup for AttachedElement<'_> {
    fn load(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        if self.index >= self.tree.len {
            return Err(Error::OutOfBounds);
        }
        unsafe { fix_sdk_table_entry(self.tree.table.index, u32::try_from(self.index)?) };
        handle::follow(path)
    }
}
impl AttachedBlob<'_> {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn read_into(&self, offset: usize, destination: &mut [u8]) -> Result<(), Error> {
        if offset > self.len || destination.len() > self.len - offset {
            return Err(Error::OutOfBounds);
        }
        unsafe {
            fix_sdk_memory_read(
                self.memory.index,
                u32::try_from(offset)?,
                destination.as_mut_ptr(),
                u32::try_from(destination.len())?,
            )
        };
        Ok(())
    }
}
impl Drop for AttachedTree<'_> {
    fn drop(&mut self) {
        unsafe { fix_sdk_detach_table(self.table.index) };
    }
}
impl Drop for AttachedBlob<'_> {
    fn drop(&mut self) {
        unsafe { fix_sdk_detach_memory(self.memory.index) };
    }
}
