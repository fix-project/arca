#![cfg_attr(not(feature = "std"), no_std)]
#![feature(allocator_api)]
#![feature(new_range_api)]
#![feature(test)]
#![feature(alloc_layout_extra)]
#![feature(ptr_metadata)]
#![feature(slice_from_ptr_range)]
#![feature(new_zeroed_alloc)]
#![feature(sync_unsafe_cell)]
#![cfg_attr(feature = "thread_local_cache", feature(thread_local))]

pub mod buddy;
pub mod refcnt;
pub use buddy::BuddyAllocator;
pub mod arrayvec;
pub mod controlreg;
pub mod message;
pub mod ringbuffer;
pub mod util;

#[repr(C)]
#[derive(Debug)]
pub struct LogRecord {
    pub level: u8,
    pub target: (usize, usize),
    pub file: Option<(usize, usize)>,
    pub line: Option<u32>,
    pub module_path: Option<(usize, usize)>,
    pub message: (usize, usize),
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct SymtabRecord {
    pub addr: usize,
    pub offset: usize,
    pub found: bool,
    pub file_buffer: (usize, usize),
    pub file_len: usize,
}
