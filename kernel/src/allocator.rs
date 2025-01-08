use core::{alloc::GlobalAlloc, cell::OnceCell};

use crate::spinlock::SpinLock;

// TODO: there must be a better way to deal with initalizing this, without wrapping everything in a
// lock
pub static PHYSICAL_ALLOCATOR: SpinLock<OnceCell<common::BuddyAllocator>> =
    SpinLock::new(OnceCell::new());

#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let lock = PHYSICAL_ALLOCATOR.lock();
        let allocator = lock.get().unwrap();
        let size = layout.size();
        let align = layout.align();
        let size = core::cmp::max(size, align);
        allocator.allocate_raw(size) as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let lock = PHYSICAL_ALLOCATOR.lock();
        let allocator = lock.get().unwrap();
        let size = layout.size();
        let align = layout.align();
        let size = core::cmp::max(size, align);
        allocator.free_raw(ptr as *mut (), size)
    }
}
