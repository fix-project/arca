use core::alloc::GlobalAlloc;

use common::BuddyAllocator;

use crate::prelude::*;

pub static PHYSICAL_ALLOCATOR: OnceLock<BuddyAllocator> = OnceLock::new();

#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();
        let size = core::cmp::max(size, align);

        let allocator = PHYSICAL_ALLOCATOR.wait();
        allocator.allocate_raw(size) as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let size = layout.size();
        let align = layout.align();
        let size = core::cmp::max(size, align);

        let allocator = PHYSICAL_ALLOCATOR.wait();
        allocator.free_raw(ptr as *mut (), size)
    }
}
