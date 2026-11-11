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

pub const FUNCREF_SIZE: usize = core::mem::size_of::<wasm_rt_funcref_t>();

pub static mut MEMORY_IDX: usize = 0;
pub static mut TABLE_IDX: usize = 0;

pub const REGION_COUNT: usize = 64;
pub const LOCAL_REGION_COUNT: usize = 62;
pub const NUM_MEMORIES: usize = REGION_COUNT;
pub const NUM_TABLES: usize = REGION_COUNT;
pub const PAYLOAD_SIZE: usize = 1 << 32;
pub const SLOT_SIZE: usize = PAYLOAD_SIZE;
pub const LOCAL_BASE: usize = REGION_COUNT * SLOT_SIZE;
pub const HANDLE_BASE: usize = 0x7e00000000;
pub const MAX_PAGES: u64 = (PAYLOAD_SIZE / PAGE_SIZE as usize) as u64;
pub const MAX_FUNCREF_TABLE_ELEMENTS: u32 = (PAYLOAD_SIZE / FUNCREF_SIZE) as u32;

pub static mut MEMORIES: [*mut wasm_rt_memory_t; NUM_MEMORIES] =
    [core::ptr::null_mut(); NUM_MEMORIES];
pub static mut FUNCREF_TABLES: [*mut wasm_rt_funcref_table_t; NUM_TABLES] =
    [core::ptr::null_mut(); NUM_TABLES];

pub static mut MEMORY_ATTACHED: [bool; NUM_MEMORIES] = [false; NUM_MEMORIES];
pub static mut MEMORY_HANDLES: [Option<u32>; NUM_MEMORIES] = [None; NUM_MEMORIES];

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

pub(crate) unsafe fn copy_guest_bytes(source: usize, destination: usize, length: usize) {
    unsafe {
        core::arch::asm!(
            "rep movsb",
            inout("rsi") source => _,
            inout("rdi") destination => _,
            inout("rcx") length => _,
            options(nostack),
        );
    }
}

fn table_bytes(elements: u32, element_size: usize) -> usize {
    (elements as usize * element_size).next_multiple_of(4096)
}

/**
 * Initialize a Memory object with an initial page size of `initial_pages` and
 * a maximum page size of `max_pages`, indexed with an i32 or i64.
 *
 *  ```
 *    wasm_rt_memory_t my_memory;
 *    // 1 initial page (65536 bytes), and a maximum of 2 pages,
 *    // indexed with an i32
 *    wasm_rt_allocate_memory(&my_memory, 1, 2, false, PAGE_SIZE);
 *  ```
 */
#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_allocate_memory(
    memory: *mut wasm_rt_memory_t,
    initial_pages: u64,
    max_pages: u64,
    is64: bool,
    page_size: u32,
) {
    unsafe {
        assert!(!memory.is_null());
        assert!(!is64);
        assert_eq!(page_size, PAGE_SIZE);
        assert!(initial_pages <= max_pages && max_pages <= MAX_PAGES);
        let idx = MEMORY_IDX;
        assert!(idx + TABLE_IDX < REGION_COUNT);
        let data = (SLOT_SIZE * idx) as *mut u8;
        let size = initial_pages * PAGE_SIZE as u64;
        map_region(data, size as usize, __MODE_read_write).expect("memory allocation failed");
        memory.write(wasm_rt_memory_t {
            data,
            page_size,
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
        let idx = TABLE_IDX;
        assert!(idx + MEMORY_IDX < REGION_COUNT);
        let data = (SLOT_SIZE * (REGION_COUNT - idx - 1)) as *mut u8;
        let memory_size = map_region(data, table_bytes(elements, FUNCREF_SIZE), __MODE_read_write)
            .expect("table allocation failed") as i64;
        table.write(wasm_rt_funcref_table_t {
            data: data.cast(),
            memory_size,
            size: elements,
            max_size: max_elements,
        });
        FUNCREF_TABLES[idx] = table;
        TABLE_IDX += 1;
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
 * Free a funcref Table object.
 */
#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_free_funcref_table(table: *mut wasm_rt_funcref_table_t) {
    todo!();
}

#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_trap(trap: wasm_rt_trap_t) -> ! {
    crate::continuation::trap(trap)
}

pub unsafe fn retain_memory(index: usize, handle: crate::handles::Reference) {
    unsafe {
        if let Some(previous) = MEMORY_HANDLES[index].take() {
            crate::descriptors::close(previous);
        }
        MEMORY_HANDLES[index] = Some(crate::descriptors::insert(handle));
    }
}
pub unsafe fn release_memory(index: usize) {
    unsafe {
        if let Some(previous) = MEMORY_HANDLES[index].take() {
            crate::descriptors::close(previous);
        }
    }
}
