#![cfg_attr(not(test), no_std)]
#![feature(allocator_api)]
#![feature(new_range_api)]
#![feature(test)]
#![feature(alloc_layout_extra)]
#![feature(ptr_metadata)]
#![feature(ptr_sub_ptr)]
#![feature(slice_from_ptr_range)]
#![feature(new_zeroed_alloc)]

pub mod buddy;
pub mod refcnt;
pub use buddy::BuddyAllocator;
pub mod controlreg;

#[repr(C)]
pub struct LogRecord {
    pub level: u8,
    pub target: (usize, usize),
    pub file: Option<(usize, usize)>,
    pub line: Option<u32>,
    pub module_path: Option<(usize, usize)>,
    pub message: (usize, usize),
}
