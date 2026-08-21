#![cfg_attr(target_arch = "wasm32", no_std)]
#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

use core::marker::PhantomData;
use fixhandle::{
    BitPack, Blob, BlobName, Encode, Handle, Object, RawName, Ref, Thunk, Tree, TreeName,
};
pub use macros::{fix_entrypoint, num_memories, num_tables};

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Producer {
    // Combination = 0, used on C side
    // table_index, entry_index
    TableGet = 1,
    // memory_index, length
    CreateBlob = 2,
    // table_index, length
    CreateTree = 3,
}

fn encode_args(producer: Producer, index: u16, entry: usize) -> RawName {
    let mut bytes = [0; 32];
    // 4 bytes for entry/length argument
    bytes[24..28].copy_from_slice(&(entry as u32).to_le_bytes());
    // two bytes for table/memory index
    bytes[28..30].copy_from_slice(&index.to_le_bytes());
    // two bits for producer type
    bytes[30..32].copy_from_slice(&((producer as u16) << 12).to_le_bytes());
    RawName::forge(bytes)
}

#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct RustHandle<'a> {
    pub raw_handle: [u8; 32],
    source: PhantomData<&'a ()>, // Lifetimes for memories/tables to prevent overwriting
}

impl<'a> RustHandle<'a> {
    fn new(handle: Handle) -> Self {
        Self {
            raw_handle: handle.pack(),
            source: PhantomData,
        }
    }

    fn unpack(self) -> Handle {
        Handle::unpack(self.raw_handle)
    }

    pub fn len(&self) -> usize {
        unsafe { fix_len(&self.raw_handle) }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

unsafe extern "C" {
    fn fix_memory_slot(index: u16) -> *mut Memory;
    fn fix_table_slot(index: u16) -> *mut Table;
}

#[repr(transparent)]
pub struct Memory(u16);

impl Memory {
    #[doc(hidden)]
    pub const EMPTY: Self = Self(0);

    pub fn new(index: u16) -> Option<&'static mut Self> {
        let slot = unsafe { fix_memory_slot(index) };
        if !slot.is_null() {
            let memory = unsafe { &mut *slot };
            memory.0 = index;
            return Some(memory);
        }
        None
    }

    // Borrows the memory until the handle is consumed
    pub fn create_blob(&self, length: usize) -> RustHandle<'_> {
        RustHandle::new(Handle::Object(Object::Blob(Blob::Blob(unsafe {
            BlobName::new(encode_args(Producer::CreateBlob, self.0, length))
        }))))
    }

    pub fn read(&self, destination: &mut [u8]) {
        unsafe {
            fix_memory_read(
                self.0 as u32,
                destination.as_mut_ptr() as u32,
                destination.len(),
            )
        }
    }

    pub fn write(&mut self, source: &[u8]) {
        unsafe { fix_memory_write(self.0 as u32, source.as_ptr() as u32, source.len()) }
    }

    pub fn size(&self) -> usize {
        unsafe { fix_memory_size(self.0 as u32) }
    }

    pub fn grow(&mut self, num_pages: usize) -> usize {
        unsafe { fix_memory_grow(self.0 as u32, num_pages) }
    }

    pub fn attach_blob(&mut self, handle: RustHandle<'_>) {
        unsafe { fix_attach_blob(self.0 as u32, &handle.raw_handle) }
    }
}

#[repr(transparent)]
pub struct Table(u16);

impl Table {
    #[doc(hidden)]
    pub const EMPTY: Self = Self(0);

    pub fn new(index: u16) -> Option<&'static mut Self> {
        let slot = unsafe { fix_table_slot(index) };
        if !slot.is_null() {
            let table = unsafe { &mut *slot };
            table.0 = index;
            return Some(table);
        }
        None
    }

    // Borrows the table until the handle is consumed
    pub fn create_tree(&self, length: usize) -> RustHandle<'_> {
        RustHandle::new(Handle::Object(Object::Tree(Tree::Tree(unsafe {
            TreeName::new(encode_args(Producer::CreateTree, self.0, length))
        }))))
    }

    pub fn get(&self, entry: usize) -> RustHandle<'_> {
        assert!(entry < self.size());
        RustHandle::new(Handle::Object(Object::Tree(Tree::Tree(unsafe {
            TreeName::new(encode_args(Producer::TableGet, self.0, entry))
        }))))
    }

    pub fn set(&mut self, entry: usize, handle: RustHandle<'_>) {
        assert!(entry < self.size());
        unsafe { fix_table_set(self.0 as u32, entry, &handle.raw_handle) }
    }

    pub fn size(&self) -> usize {
        unsafe { fix_table_size(self.0 as u32) }
    }

    pub fn grow(&mut self, entries: usize) -> usize {
        unsafe { fix_table_grow(self.0 as u32, entries) }
    }

    pub fn attach_tree(&mut self, handle: RustHandle<'_>) {
        unsafe { fix_attach_tree(self.0 as u32, &handle.raw_handle) }
    }
}

pub fn create_ref<'a>(handle: RustHandle<'a>) -> RustHandle<'a> {
    RustHandle::new(Handle::Ref(match handle.unpack() {
        Handle::Ref(reference) => reference,
        Handle::Object(Object::Blob(blob)) => Ref::Blob(blob),
        Handle::Object(Object::Tree(tree)) => Ref::Tree(tree),
        _ => panic!("create_ref: handle does not refer to an Object"),
    }))
}

pub fn create_identification_thunk<'a>(handle: RustHandle<'a>) -> RustHandle<'a> {
    let reference = match handle.unpack() {
        Handle::Ref(reference) => reference,
        Handle::Object(Object::Blob(blob)) => Ref::Blob(blob),
        Handle::Object(Object::Tree(tree)) => Ref::Tree(tree),
        _ => panic!("create_identification_thunk: handle does not refer to an Object or Ref"),
    };
    RustHandle::new(Thunk::Identification(reference).into())
}

pub fn create_application_thunk<'a>(handle: RustHandle<'a>) -> RustHandle<'a> {
    RustHandle::new(Thunk::Application(handle.unpack().unwrap_object().unwrap_tree()).into())
}

pub fn create_selection_thunk<'a>(handle: RustHandle<'a>) -> RustHandle<'a> {
    RustHandle::new(Thunk::Selection(handle.unpack().unwrap_object().unwrap_tree()).into())
}

pub fn create_strict_encode<'a>(handle: RustHandle<'a>) -> RustHandle<'a> {
    RustHandle::new(Encode::Strict(handle.unpack().unwrap_thunk()).into())
}

pub fn create_shallow_encode<'a>(handle: RustHandle<'a>) -> RustHandle<'a> {
    RustHandle::new(Encode::Shallow(handle.unpack().unwrap_thunk()).into())
}

unsafe extern "C" {
    pub fn fix_memory_read(memory_index: u32, destination: u32, length: usize);
    pub fn fix_memory_write(memory_index: u32, source: u32, length: usize);
    pub fn fix_memory_size(memory_index: u32) -> usize;
    pub fn fix_memory_grow(memory_index: u32, num_pages: usize) -> usize;

    pub fn fix_table_size(table_index: u32) -> usize;
    pub fn fix_table_grow(table_index: u32, entries: usize) -> usize;

    pub fn fix_attach_blob(memory_index: u32, handle: *const [u8; 32]);
    pub fn fix_attach_tree(table_index: u32, handle: *const [u8; 32]);
    pub fn fix_len(handle: *const [u8; 32]) -> usize;
    pub fn fix_table_set(table_index: u32, entry_index: usize, handle: *const [u8; 32]);
}
