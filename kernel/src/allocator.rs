use core::alloc::GlobalAlloc;

use crate::initcell::InitCell;

pub static PHYSICAL_ALLOCATOR: InitCell<common::BuddyAllocator> = InitCell::empty();

#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let allocator = &PHYSICAL_ALLOCATOR;
        let size = layout.size();
        let align = layout.align();
        let size = core::cmp::max(size, align);
        allocator.allocate_raw(size) as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        // TODO: there's a bug somewhere in the buddy allocator leading to crashes

        // let allocator = &PHYSICAL_ALLOCATOR;
        // let size = layout.size();
        // let align = layout.align();
        // let size = core::cmp::max(size, align);
        // allocator.free_raw(ptr as *mut (), size)
    }
}
