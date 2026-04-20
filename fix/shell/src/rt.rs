#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use core::sync::atomic::{AtomicUsize, Ordering};

use arcane::{__MODE_read_write, arca_compat_mmap};
use user::error;

include!(concat!(env!("OUT_DIR"), "/wasm_rt.rs"));

unsafe extern "C" {
    pub fn wasm_rt_init();
    pub fn wasm_rt_module_size() -> usize;
    pub fn wasm_rt_free();
}

pub static mut MEMORY_IDX: usize = 0;
pub static mut TABLE_IDX: usize = 0;

pub static mut MEMORIES: [*mut wasm_rt_memory_t; 64] = [core::ptr::null_mut(); 64];
pub static mut TABLES: [*mut wasm_rt_externref_table_t; 64] = [core::ptr::null_mut(); 64];

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
        assert!(idx < 64);
        MEMORIES[idx] = memory;
        assert!(!is64);
        assert!(max_pages <= (1u64 << 32) / PAGE_SIZE as u64);
        let data = ((1 << 32) * idx) as *mut u8;
        let size = initial_pages * PAGE_SIZE as u64;
        arca_compat_mmap(data as *mut _, size as usize, __MODE_read_write);
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
        arca_compat_mmap(start as *mut _, size as usize, __MODE_read_write);
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
        assert!(idx < 63);
        TABLES[idx] = table;
        if max_elements > (1 << (32 - 5)) {
            max_elements = 1 << (32 - 5);
        }
        let data = ((1 << 32) * (64 + idx)) as *mut u8;
        arca_compat_mmap(data as *mut _, (elements * 32) as usize, __MODE_read_write);
        table.write(wasm_rt_externref_table_t {
            data: data as *mut _,
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
 * Stop execution immediately and jump back to the call to `wasm_rt_impl_try`.
 * The result of `wasm_rt_impl_try` will be the provided trap reason.
 *
 * This is typically called by the generated code, and not the embedder.
 */
#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_trap(trap: wasm_rt_trap_t) {
    panic!("wasm rt trap: {trap}");
}
