use core::{
    alloc::{Allocator, GlobalAlloc},
    ptr::NonNull,
};

use crate::prelude::*;

#[global_allocator]
static ALLOCATOR: SystemAllocator = SystemAllocator;

struct SystemAllocator;

unsafe impl GlobalAlloc for SystemAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let result = BuddyAllocator
            .allocate(layout)
            .map(|p| &raw mut (*p.as_ptr())[0])
            .unwrap_or(core::ptr::null_mut());
        debug_assert!(!result.is_null());
        result
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        if let Some(p) = NonNull::new(ptr) {
            BuddyAllocator.deallocate(p, layout);
        }
    }
}
