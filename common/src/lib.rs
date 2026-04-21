#![cfg_attr(not(feature = "std"), no_std)]
#![allow(stable_features, unused_features)]
#![feature(allocator_api)]
#![feature(fn_traits)]
#![cfg_attr(feature = "std", feature(layout_for_ptr))]
#![feature(negative_impls)]
#![feature(ptr_metadata)]
#![cfg_attr(test, feature(test))]
#![feature(unboxed_closures)]
#![cfg_attr(feature = "thread_local_cache", feature(thread_local))]

pub mod buddy;
pub mod refcnt;
pub use buddy::BuddyAllocator;
pub mod arrayvec;
pub mod bitpack;
pub mod controlreg;
pub mod elfloader;
pub mod ipaddr;
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
    pub const TCP_CONNECT: u64 = 5;
    pub const TCP_LISTEN: u64 = 6;
    pub const TCP_ACCEPT: u64 = 7;
    pub const TCP_CLOSE: u64 = 8;
    pub const TCP_SEND: u64 = 9;
    pub const TCP_RECV: u64 = 10;
    pub const FILE_OPEN: u64 = 11;
    pub const FILE_CLOSE: u64 = 12;
    pub const FILE_READ: u64 = 13;
    pub const FILE_WRITE: u64 = 14;
}
