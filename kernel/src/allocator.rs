use core::{
    alloc::{Allocator, GlobalAlloc},
    ptr::NonNull,
};

use common::BuddyAllocator;

use crate::prelude::*;

pub static PHYSICAL_ALLOCATOR: OnceLock<BuddyAllocator> = OnceLock::new();

#[global_allocator]
static ALLOCATOR: SystemAllocator = SystemAllocator;

struct SystemAllocator;

unsafe impl GlobalAlloc for SystemAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let allocator = PHYSICAL_ALLOCATOR.wait();
        allocator
            .allocate(layout)
            .map(|p| &raw mut (*p.as_ptr())[0])
            .unwrap_or(core::ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let allocator = PHYSICAL_ALLOCATOR.wait();
        if let Some(p) = NonNull::new(ptr) {
            allocator.deallocate(p, layout);
        }
    }
}
