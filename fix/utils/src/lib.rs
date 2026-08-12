#![no_std]
extern crate alloc;
use alloc::{vec, vec::Vec};
use core::hint::black_box;
use externref::{ExternRef, Resource, externref};

pub struct Handle(());

#[externref]
#[link(wasm_import_module = "fixpoint")]
unsafe extern "C" {
    fn get_length(handle: &Resource<Handle>) -> u32;
    fn attach_blob(memory_idx: u32, handle: &Resource<Handle>);
    #[link_name = "create_blob"]
    fn fixpoint_create_blob(memory_idx: u32, length: u32) -> Resource<Handle>;
    fn attach_tree(table_idx: u32, handle: &Resource<Handle>);
    #[link_name = "create_tree"]
    fn fixpoint_create_tree(table_idx: u32, length: u32) -> Resource<Handle>;
}

#[unsafe(export_name = "_instrument_memory_1_read")]
extern "C" fn memory_1_read(dst: u32, length: u32) {
    black_box((dst, length));
}

#[unsafe(export_name = "_instrument_memory_1_write")]
extern "C" fn memory_1_write(src: u32, length: u32) {
    black_box((src, length));
}

#[unsafe(export_name = "_instrument_memory_1_size")]
extern "C" fn memory_1_size() -> u32 {
    black_box(0_u32)
}

#[unsafe(export_name = "_instrument_memory_1_grow")]
extern "C" fn memory_1_grow(pages: u32) -> u32 {
    black_box(pages)
}

#[unsafe(export_name = "_instrument_table_1_size")]
extern "C" fn table_1_size() -> u32 {
    black_box(0_u32)
}

#[unsafe(export_name = "_instrument_table_1_grow")]
extern "C" fn table_1_grow(elements: u32) -> u32 {
    black_box(elements)
}

#[externref]
extern "C" fn _instrument_table_1_read(_index: u32) -> Resource<Handle> {
    panic!()
}

#[externref]
extern "C" fn _instrument_table_1_write(_index: u32, _handle: &Resource<Handle>) {}

unsafe extern "C" {
    #[link_name = "_instrument_table_1_read"]
    fn wrapper_table_1_read(index: u32) -> ExternRef;
    #[link_name = "_instrument_table_1_write"]
    fn wrapper_table_1_write(index: u32, handle: ExternRef);
}

unsafe fn table_1_read(index: u32) -> Resource<Handle> {
    unsafe {
        ExternRef::guard();
        Resource::new_non_null(wrapper_table_1_read(index))
    }
}

unsafe fn table_1_write(index: u32, handle: &Resource<Handle>) {
    unsafe {
        ExternRef::guard();
        wrapper_table_1_write(index, Resource::raw(Some(handle)));
    }
}

pub fn get_blob(handle: Resource<Handle>) -> Vec<u8> {
    unsafe {
        let length = get_length(&handle);
        attach_blob(1, &handle);
        let mut blob = vec![0; length as usize];
        memory_1_read(blob.as_mut_ptr() as u32, length);
        blob
    }
}

pub fn create_blob(blob: &[u8]) -> Resource<Handle> {
    unsafe {
        let length = blob.len() as u32;
        let required = length.div_ceil(65536);
        let mapped = memory_1_size();
        if mapped < required {
            memory_1_grow(required - mapped);
        }
        memory_1_write(blob.as_ptr() as u32, length);
        fixpoint_create_blob(1, length)
    }
}

pub fn get_tree(handle: Resource<Handle>) -> Vec<Resource<Handle>> {
    unsafe {
        attach_tree(1, &handle);
        (0..table_1_size()).map(|i| table_1_read(i)).collect()
    }
}

pub fn create_tree(tree: &[Resource<Handle>]) -> Resource<Handle> {
    unsafe {
        let length = tree.len() as u32;
        let mapped = table_1_size();
        if mapped < length {
            table_1_grow(length - mapped);
        }
        for (index, handle) in tree.iter().enumerate() {
            table_1_write(index as u32, handle);
        }
        fixpoint_create_tree(1, length)
    }
}
