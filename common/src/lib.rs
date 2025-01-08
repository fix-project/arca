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
