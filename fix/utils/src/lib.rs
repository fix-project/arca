#![cfg_attr(target_arch = "wasm32", no_std)]
extern crate alloc;
#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

use alloc::vec::Vec;
use core::marker::PhantomData;
use fixhandle::{
    BitPack, Blob, BlobName, Encode, Handle, Object, RawName, Ref, Thunk, Tree, TreeName,
};
pub use macros::{fix_entrypoint, num_memories, num_tables};

pub mod memory;
pub mod table;

pub use memory::*;
pub use table::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixError {
    AllOcuppied, // All memories/tables occupied
    Unavailable, // Resource unavilable
    GrowFailed,  // Memory/Table growth failed
    OutOfBounds, // Memory/Table access out of bounds
}

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

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FixError> {
        Memory::from_bytes(bytes)?.to_blob(bytes.len())
    }

    pub fn from_entries(entries: &[RustHandle<'_>]) -> Result<Self, FixError> {
        Table::from_entries(entries)?.to_tree(entries.len())
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, FixError> {
        Memory::from_blob(*self)?.to_bytes(self.len())
    }

    pub fn to_entries(&self) -> Result<Vec<RustHandle<'static>>, FixError> {
        Table::from_tree(*self)?.to_entries(self.len())
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
