use crate::prelude::*;
use common::util::spinlock::SpinLock;
use core::alloc::{Allocator, GlobalAlloc, Layout};
use core::ptr::NonNull;

#[global_allocator]
static ALLOCATOR: Heap = Heap;

struct Heap;
unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if let Some(index) = class(layout) {
            return unsafe { allocate_small(index) };
        }
        unsafe {
            BuddyAllocator
                .allocate(layout)
                .map(|x| &raw mut (*x.as_ptr())[0])
                .unwrap_or(core::ptr::null_mut())
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Slab slots exclude the page header; page-aligned pointers belong to Buddy.
        if !(ptr as usize).is_multiple_of(PAGE) {
            unsafe { deallocate_small(ptr) };
            return;
        }
        unsafe {
            if let Some(ptr) = NonNull::new(ptr) {
                BuddyAllocator.deallocate(ptr, layout);
            };
        }
    }
}

// Heap slabs and externally owned buffers both reside in Buddy memory.
pub fn buffer(bytes: alloc::vec::Vec<u8>) -> common::buffer::Buffer<alloc::vec::Vec<u8>> {
    unsafe { common::buffer::Buffer::from_buddy(bytes) }
}

const PAGE: usize = 4096;
const MIN_CLASS: u32 = 5;
const CLASSES: usize = 6;

struct Slab {
    // Class lists have the lifetime of the kernel.
    owner: *const SpinLock<Class>,
    previous: *mut Slab,
    next: *mut Slab,
    free: *mut u8,
    live: usize,
}

struct Class {
    first: *mut Slab,
    empty: *mut Slab,
}
unsafe impl Send for Class {}

#[macros::core_local]
static CLASSES_HEAD: [SpinLock<Class>; CLASSES] = [const {
    SpinLock::new(Class {
        first: core::ptr::null_mut(),
        empty: core::ptr::null_mut(),
    })
}; CLASSES];

fn class(layout: Layout) -> Option<usize> {
    let order = layout
        .size()
        .max(layout.align())
        .max(1 << MIN_CLASS)
        .next_power_of_two()
        .ilog2();
    let index = (order - MIN_CLASS) as usize;
    (index < CLASSES).then_some(index)
}

unsafe fn link(head: &mut Class, slab: *mut Slab) {
    unsafe {
        (*slab).previous = core::ptr::null_mut();
        (*slab).next = head.first;
        if !head.first.is_null() {
            (*head.first).previous = slab;
        }
        head.first = slab;
    }
}

unsafe fn unlink(head: &mut Class, slab: *mut Slab) {
    unsafe {
        if (*slab).previous.is_null() {
            head.first = (*slab).next;
        } else {
            (*(*slab).previous).next = (*slab).next;
        }
        if !(*slab).next.is_null() {
            (*(*slab).next).previous = (*slab).previous;
        }
    }
}

unsafe fn allocate_small(index: usize) -> *mut u8 {
    let size = 1 << (MIN_CLASS + index as u32);
    let class = &CLASSES_HEAD[index];
    let mut head = class.lock();
    if head.first.is_null() {
        drop(head);
        let layout = Layout::from_size_align(PAGE, PAGE).unwrap();
        let Ok(page) = BuddyAllocator.allocate(layout) else {
            return core::ptr::null_mut();
        };
        let slab = page.as_ptr().cast::<u8>().cast::<Slab>();
        unsafe {
            slab.write(Slab {
                owner: class,
                previous: core::ptr::null_mut(),
                next: core::ptr::null_mut(),
                free: core::ptr::null_mut(),
                live: 0,
            });
            for offset in (core::mem::size_of::<Slab>().next_multiple_of(size)..PAGE).step_by(size)
            {
                let slot = slab.cast::<u8>().add(offset);
                slot.cast::<*mut u8>().write((*slab).free);
                (*slab).free = slot;
            }
            head = class.lock();
            link(&mut head, slab);
        }
    }
    unsafe {
        let slab = head.first;
        if head.empty == slab {
            head.empty = core::ptr::null_mut();
        }
        let result = (*slab).free;
        (*slab).free = result.cast::<*mut u8>().read();
        (*slab).live += 1;
        if (*slab).free.is_null() {
            unlink(&mut head, slab);
        }
        result
    }
}

unsafe fn deallocate_small(pointer: *mut u8) {
    let slab = (pointer as usize & !(PAGE - 1)) as *mut Slab;
    let class = unsafe { &*(*slab).owner };
    let mut head = class.lock();
    let empty = unsafe {
        if (*slab).free.is_null() {
            link(&mut head, slab);
        }
        pointer.cast::<*mut u8>().write((*slab).free);
        (*slab).free = pointer;
        (*slab).live -= 1;
        if (*slab).live == 0 && !head.empty.is_null() {
            unlink(&mut head, slab);
            true
        } else {
            if (*slab).live == 0 {
                head.empty = slab;
            }
            false
        }
    };
    drop(head);
    if empty {
        unsafe {
            BuddyAllocator.deallocate(
                NonNull::new_unchecked(slab.cast()),
                Layout::from_size_align(PAGE, PAGE).unwrap(),
            );
        }
    }
}
