#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use core::{
    slice::from_raw_parts_mut,
    sync::atomic::{AtomicUsize, Ordering},
};

use arcane::{__MODE_read_write, arca_compat_mmap};
use user::error;

include!(concat!(env!("OUT_DIR"), "/wasm_rt.rs"));

unsafe extern "C" {
    pub fn wasm_rt_init();
    pub fn wasm_rt_module_size() -> usize;
    pub fn wasm_rt_free();
}

pub const EXTERNREF_SIZE: usize = core::mem::size_of::<wasm_rt_externref_t>();
pub const FUNCREF_SIZE: usize = core::mem::size_of::<wasm_rt_funcref_t>();

/*
 *  The virtual memory layout is divided into 4 GiB pieces or "slots" and allocated to
 *  memories, externref tables, and funcref tables in the order they are listed.
 *
 * idx                          base address
 * [0, NUM_MEMORIES)            SLOT_SIZE * idx
 * [0, NUM_TABLES)              SLOT_SIZE * (NUM_MEMORIES + idx)
 * [0, NUM_FUNCREF_TABLES)      SLOT_SIZE * (NUM_MEMORIES + NUM_TABLES + idx)
 */
pub static mut MEMORY_IDX: usize = 0;
pub static mut TABLE_IDX: usize = 0;
pub static mut FUNCREF_TABLE_IDX: usize = 0;

pub const NUM_MEMORIES: usize = 64;
pub const NUM_TABLES: usize = 32;
pub const NUM_FUNCREF_TABLES: usize = 32;
pub const SLOT_SIZE: usize = 1 << 32;
pub const MAX_PAGES: u64 = (SLOT_SIZE / PAGE_SIZE as usize) as u64;
pub const MAX_TABLE_ELEMENTS: u32 = (SLOT_SIZE / EXTERNREF_SIZE) as u32;
pub const MAX_FUNCREF_TABLE_ELEMENTS: u32 = (SLOT_SIZE / FUNCREF_SIZE) as u32;

pub static mut MEMORIES: [*mut wasm_rt_memory_t; NUM_MEMORIES] =
    [core::ptr::null_mut(); NUM_MEMORIES];
pub static mut TABLES: [*mut wasm_rt_externref_table_t; NUM_TABLES] =
    [core::ptr::null_mut(); NUM_TABLES];
pub static mut FUNCREF_TABLES: [*mut wasm_rt_funcref_table_t; NUM_FUNCREF_TABLES] =
    [core::ptr::null_mut(); NUM_FUNCREF_TABLES];

/**
 * Initialize a Memory object with an initial page size of `initial_pages` and
 * a maximum page size of `max_pages`, indexed with an i32 or i64.
 *
 *  ```
 *    wasm_rt_memory_t my_memory;
 *    // 1 initial page (65536 bytes), and a maximum of 2 pages,
 *    // indexed with an i32
 *    wasm_rt_allocate_memory(&my_memory, 1, 2, false);
 *  ```
 */
#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_allocate_memory(
    memory: *mut wasm_rt_memory_t,
    initial_pages: u64,
    max_pages: u64,
    is64: bool,
) {
    unsafe {
        let idx = MEMORY_IDX;
        MEMORY_IDX += 1;
        assert!(idx < NUM_MEMORIES);
        MEMORIES[idx] = memory;
        assert!(!is64);
        assert!(max_pages <= MAX_PAGES);
        let data = (SLOT_SIZE * idx) as *mut u8;
        let size = initial_pages * PAGE_SIZE as u64;
        let memory_size = arca_compat_mmap(data as *mut _, size as usize, __MODE_read_write);
        assert_eq!(memory_size, size as i64); // don't expect kernel to overallocate when requesting units of Wasm page size
        memory.write(wasm_rt_memory_t {
            data,
            pages: initial_pages,
            max_pages,
            size,
            is64,
        });
    }
}

/**
 * Grow a Memory object by `pages`, and return the previous page count. If
 * this new page count is greater than the maximum page count, the grow fails
 * and 0xffffffffu (UINT32_MAX) is returned instead.
 *
 *  ```
 *    wasm_rt_memory_t my_memory;
 *    ...
 *    // Grow memory by 10 pages.
 *    uint32_t old_page_size = wasm_rt_grow_memory(&my_memory, 10);
 *    if (old_page_size == UINT32_MAX) {
 *      // Failed to grow memory.
 *    }
 *  ```
 */
#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_grow_memory(memory: *mut wasm_rt_memory_t, pages: u64) -> u32 {
    let memory = unsafe { &mut *memory };
    let current = memory.pages;
    if current + pages >= memory.max_pages {
        return u32::MAX;
    }
    let start = unsafe { memory.data.byte_add(current as usize * PAGE_SIZE as usize) };
    let size = pages * PAGE_SIZE as u64;
    unsafe {
        let memory_size = arca_compat_mmap(start as *mut _, size as usize, __MODE_read_write);
        assert_eq!(memory_size, size as i64); // don't expect kernel to overallocate when using units of Wasm page size
        memory.pages += pages;
        memory.size += size;
    }
    current as u32
}

/**
 * Initialize an externref Table object with an element count
 * of `elements` and a maximum size of `max_elements`.
 * Usage as per wasm_rt_allocate_funcref_table.
 */
#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_allocate_externref_table(
    table: *mut wasm_rt_externref_table_t,
    elements: u32,
    mut max_elements: u32,
) {
    unsafe {
        let idx = TABLE_IDX;
        TABLE_IDX += 1;
        assert!(idx < NUM_TABLES);
        TABLES[idx] = table;
        max_elements = max_elements.min(MAX_TABLE_ELEMENTS);
        let data = (SLOT_SIZE * (NUM_MEMORIES + idx)) as *mut u8;
        let memory_size = arca_compat_mmap(
            data as *mut _,
            (elements * EXTERNREF_SIZE as u32).next_multiple_of(PAGE_SIZE) as usize,
            __MODE_read_write,
        );
        table.write(wasm_rt_externref_table_t {
            data: data as *mut _,
            memory_size,
            size: elements,
            max_size: max_elements,
        });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_grow_externref_table(
    table: *mut wasm_rt_externref_table_t,
    delta: u32,
    init: wasm_rt_externref_t,
) -> u32 {
    let table = unsafe { &mut *table };
    if table
        .size
        .checked_add(delta)
        .is_none_or(|tot| tot > table.max_size)
    {
        return u32::MAX;
    }

    let desired_bytes = (table.size + delta) as usize * EXTERNREF_SIZE;
    let cur_bytes = table.memory_size as usize;
    if desired_bytes > cur_bytes {
        unsafe {
            let start = table.data.byte_add(cur_bytes);
            let bytes_needed = desired_bytes - cur_bytes;
            table.memory_size += arca_compat_mmap(start as *mut _, bytes_needed, __MODE_read_write);
        }
    }
    unsafe { from_raw_parts_mut(table.data.add(table.size as usize), delta as usize).fill(init) };

    let old_element_count = table.size;
    table.size += delta;
    old_element_count
}

// TODO: wasm_rt_grow_funcref_table

/**
 * Initialize an funcref Table object with an element count
 * of `elements` and a maximum size of `max_elements`.
 * Usage as per wasm_rt_allocate_funcref_table.
 */
#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_allocate_funcref_table(
    table: *mut wasm_rt_funcref_table_t,
    elements: u32,
    mut max_elements: u32,
) {
    unsafe {
        let idx = FUNCREF_TABLE_IDX;
        FUNCREF_TABLE_IDX += 1;
        assert!(idx < NUM_FUNCREF_TABLES);
        FUNCREF_TABLES[idx] = table;
        max_elements = max_elements.min(MAX_FUNCREF_TABLE_ELEMENTS);
        let data = (SLOT_SIZE * (NUM_MEMORIES + NUM_TABLES + idx)) as *mut u8;
        let memory_size = arca_compat_mmap(
            data as *mut _,
            elements as usize * FUNCREF_SIZE,
            __MODE_read_write,
        );
        table.write(wasm_rt_funcref_table_t {
            data: data as *mut _,
            memory_size,
            size: elements,
            max_size: max_elements,
        });
    }
}

/**
 * Free a Memory object.
 */
#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_free_memory(memory: *mut wasm_rt_memory_t) {
    todo!();
}

/**
 * Free an externref Table object.
 */
#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_free_externref_table(table: *mut wasm_rt_externref_table_t) {
    todo!();
}

/**
 * Free an externref Table object.
 */
#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_free_funcref_table(table: *mut wasm_rt_funcref_table_t) {
    todo!();
}

/**
 * Stop execution immediately and jump back to the call to `wasm_rt_impl_try`.
 * The result of `wasm_rt_impl_try` will be the provided trap reason.
 *
 * This is typically called by the generated code, and not the embedder.
 */
#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_trap(trap: wasm_rt_trap_t) {
    panic!("wasm rt trap: {trap}");
}
