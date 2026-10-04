#![allow(
    improper_ctypes,
    improper_ctypes_definitions,
    reason = "userspace uses the System V SIMD ABI; vector layout is asserted in rt"
)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use core::{
    slice::from_raw_parts_mut,
    sync::atomic::{AtomicUsize, Ordering},
};

use arcane::{__MODE_none, __MODE_read_write, arca_compat_mmap};
use user::error;

include!(env!("WASM_BINDINGS"));

unsafe extern "C" {
    pub fn wasm_rt_init();
    pub fn wasm_rt_module_size() -> usize;
    pub fn wasm_rt_module_alignment() -> usize;
    pub fn wasm_rt_free();
}

pub const EXTERNREF_SIZE: usize = core::mem::size_of::<wasm_rt_externref_t>();
const _: () = assert!(
    EXTERNREF_SIZE == fixhandle::HANDLE_SIZE
        && core::mem::align_of::<wasm_rt_externref_t>()
            == core::mem::align_of::<fixhandle::Handle>()
);

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
static mut WASM_TABLE_IDX: usize = 0;
static mut EXTERNREF_POOL: [usize; NUM_TABLES + NUM_FUNCREF_TABLES] =
    [usize::MAX; NUM_TABLES + NUM_FUNCREF_TABLES];

pub unsafe fn externref_pool(index: u32) -> usize {
    unsafe {
        assert!((index as usize) < WASM_TABLE_IDX, "table does not exist");
        let pool = EXTERNREF_POOL[index as usize];
        assert!(pool < NUM_TABLES, "table does not hold externrefs");
        pool
    }
}

pub const NUM_MEMORIES: usize = 64;
pub const NUM_TABLES: usize = 32;
pub const NUM_FUNCREF_TABLES: usize = 31;
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

pub static mut MEMORY_ATTACHED: [bool; NUM_MEMORIES] = [false; NUM_MEMORIES];
pub static mut TABLE_ATTACHED: [bool; NUM_TABLES] = [false; NUM_TABLES];

pub unsafe fn map_region(data: *mut u8, size: usize, mode: u32) -> Option<usize> {
    if size == 0 {
        return Some(0);
    }
    let mapped = unsafe { arca_compat_mmap(data.cast(), size, mode) };
    if mapped < 0 {
        return None;
    }
    assert_eq!(mapped as usize, size);
    Some(size)
}

pub unsafe fn replace_region(data: *mut u8, old_size: usize, size: usize) {
    unsafe {
        map_region(data, old_size, __MODE_none).expect("unmap failed");
        map_region(data, size, __MODE_read_write).expect("map failed");
    }
}

fn table_bytes(elements: u32, element_size: usize) -> usize {
    (elements as usize * element_size).next_multiple_of(PAGE_SIZE as usize)
}

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
        assert!(!memory.is_null());
        assert!(!is64);
        assert!(initial_pages <= max_pages && max_pages <= MAX_PAGES);
        let idx = MEMORY_IDX;
        assert!(idx < NUM_MEMORIES);
        let data = (SLOT_SIZE * idx) as *mut u8;
        let size = initial_pages * PAGE_SIZE as u64;
        map_region(data, size as usize, __MODE_read_write).expect("memory allocation failed");
        memory.write(wasm_rt_memory_t {
            data,
            pages: initial_pages,
            max_pages,
            size,
            is64,
        });
        MEMORIES[idx] = memory;
        MEMORY_IDX += 1;
    }
}

/**
 * Grow a Memory object by `pages`, and return the previous page count. If
 * this new page count is greater than the maximum page count, the grow fails
 * and 0xffffffffffffffffu (UINT64_MAX) is returned instead.
 *
 *  ```
 *    wasm_rt_memory_t my_memory;
 *    ...
 *    // Grow memory by 10 pages.
 *    uint64_t old_page_size = wasm_rt_grow_memory(&my_memory, 10);
 *    if (old_page_size == UINT64_MAX) {
 *      // Failed to grow memory.
 *    }
 *  ```
 */
#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_grow_memory(memory: *mut wasm_rt_memory_t, pages: u64) -> u64 {
    assert!(!memory.is_null());
    for index in 0..NUM_MEMORIES {
        if pages != 0 && unsafe { MEMORIES[index] == memory && MEMORY_ATTACHED[index] } {
            return u64::MAX;
        }
    }
    let memory = unsafe { &mut *memory };
    let current = memory.pages;
    let Some(desired) = current
        .checked_add(pages)
        .filter(|size| *size <= memory.max_pages)
    else {
        return u64::MAX;
    };
    let start = memory.data.wrapping_byte_add(memory.size as usize);
    let size = pages * PAGE_SIZE as u64;
    if unsafe { map_region(start, size as usize, __MODE_read_write) }.is_none() {
        return u64::MAX;
    }
    memory.pages = desired;
    memory.size = desired * PAGE_SIZE as u64;
    current
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
        assert!(!table.is_null());
        max_elements = max_elements.min(MAX_TABLE_ELEMENTS);
        assert!(elements <= max_elements);
        let idx = TABLE_IDX;
        assert!(idx < NUM_TABLES);
        let data = (SLOT_SIZE * (NUM_MEMORIES + idx)) as *mut u8;
        let memory_size = map_region(
            data,
            table_bytes(elements, EXTERNREF_SIZE),
            __MODE_read_write,
        )
        .expect("table allocation failed") as i64;
        table.write(wasm_rt_externref_table_t {
            data: data.cast(),
            memory_size,
            size: elements,
            max_size: max_elements,
        });
        assert!(WASM_TABLE_IDX < NUM_TABLES + NUM_FUNCREF_TABLES);
        EXTERNREF_POOL[WASM_TABLE_IDX] = idx;
        WASM_TABLE_IDX += 1;
        TABLES[idx] = table;
        TABLE_IDX += 1;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_grow_externref_table(
    table: *mut wasm_rt_externref_table_t,
    delta: u32,
    init: wasm_rt_externref_t,
) -> u32 {
    assert!(!table.is_null());
    for index in 0..NUM_TABLES {
        if delta != 0 && unsafe { TABLES[index] == table && TABLE_ATTACHED[index] } {
            return u32::MAX;
        }
    }
    let table = unsafe { &mut *table };
    unsafe {
        grow_table(
            table.data,
            &mut table.size,
            table.max_size,
            &mut table.memory_size,
            delta,
            init,
        )
    }
}

unsafe fn grow_table<T: Copy>(
    data: *mut T,
    size: &mut u32,
    max_size: u32,
    memory_size: &mut i64,
    delta: u32,
    init: T,
) -> u32 {
    let Some(desired) = size.checked_add(delta).filter(|size| *size <= max_size) else {
        return u32::MAX;
    };
    let desired_bytes = table_bytes(desired, core::mem::size_of::<T>());
    let current_bytes = usize::try_from(*memory_size).expect("invalid table allocation size");
    if desired_bytes > current_bytes {
        let start = data.cast::<u8>().wrapping_byte_add(current_bytes);
        if unsafe { map_region(start, desired_bytes - current_bytes, __MODE_read_write) }.is_none()
        {
            return u32::MAX;
        }
        *memory_size = desired_bytes as i64;
    }
    if delta != 0 {
        unsafe { from_raw_parts_mut(data.add(*size as usize), delta as usize).fill(init) };
    }
    let old_size = *size;
    *size = desired;
    old_size
}

#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_grow_funcref_table(
    table: *mut wasm_rt_funcref_table_t,
    delta: u32,
    init: wasm_rt_funcref_t,
) -> u32 {
    assert!(!table.is_null());
    let table = unsafe { &mut *table };
    unsafe {
        grow_table(
            table.data,
            &mut table.size,
            table.max_size,
            &mut table.memory_size,
            delta,
            init,
        )
    }
}

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
        assert!(!table.is_null());
        max_elements = max_elements.min(MAX_FUNCREF_TABLE_ELEMENTS);
        assert!(elements <= max_elements);
        let idx = FUNCREF_TABLE_IDX;
        assert!(idx < NUM_FUNCREF_TABLES);
        let data = (SLOT_SIZE * (NUM_MEMORIES + NUM_TABLES + idx)) as *mut u8;
        let memory_size = map_region(data, table_bytes(elements, FUNCREF_SIZE), __MODE_read_write)
            .expect("table allocation failed") as i64;
        table.write(wasm_rt_funcref_table_t {
            data: data.cast(),
            memory_size,
            size: elements,
            max_size: max_elements,
        });
        assert!(WASM_TABLE_IDX < NUM_TABLES + NUM_FUNCREF_TABLES);
        WASM_TABLE_IDX += 1;
        FUNCREF_TABLES[idx] = table;
        FUNCREF_TABLE_IDX += 1;
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
 * Free a funcref Table object.
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
