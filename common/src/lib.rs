#![cfg_attr(not(feature = "std"), no_std)]
#![feature(alloc_layout_extra)]
#![feature(allocator_api)]
#![feature(box_as_ptr)]
#![feature(fn_traits)]
#![feature(layout_for_ptr)]
#![feature(maybe_uninit_as_bytes)]
#![feature(maybe_uninit_slice)]
#![feature(negative_impls)]
#![feature(new_range_api)]
#![feature(ptr_metadata)]
#![feature(slice_from_ptr_range)]
#![feature(sync_unsafe_cell)]
#![feature(try_trait_v2)]
#![feature(atomic_try_update)]
#![feature(test)]
#![feature(unboxed_closures)]
#![cfg_attr(feature = "std", feature(thread_id_value))]
#![cfg_attr(feature = "thread_local_cache", feature(thread_local))]

pub mod buddy;
pub mod refcnt;
pub use buddy::BuddyAllocator;
pub mod arrayvec;
pub mod controlreg;
pub mod elfloader;
pub mod sendable;
pub mod util;
pub mod vhost;

#[cfg(feature = "std")]
pub mod mmap;

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

pub mod hypercall {
    pub const EXIT: u64 = 0;
    pub const LOG: u64 = 1;
    pub const SYMNAME: u64 = 2;
    pub const MEMSET: u64 = 3;
    pub const MEMCLR: u64 = 4;
}
