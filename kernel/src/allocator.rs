use core::alloc::GlobalAlloc;

/**
 * TODO:
 * - coalesce blocks
 * - allocate more precise amounts from the buddy allocator
 */
use crate::{
    buddy::{Page1GB, Page2MB, Page4KB},
    spinlock::SpinLock,
};

struct HeapAllocator {
    head: SpinLock<Block>,
}

#[repr(C, align(32))]
struct Block {
    prev: *mut Block,
    next: *mut Block,
    size: usize,
}

impl HeapAllocator {
    const fn new() -> HeapAllocator {
        HeapAllocator {
            head: SpinLock::new(Block {
                prev: core::ptr::null_mut(),
                next: core::ptr::null_mut(),
                size: 0,
            }),
        }
    }
}

unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        log::debug!("requested allocation: {layout:?}");
        let mut head = self.head.lock();
        let p = find_allocation(&mut *head, layout);
        log::debug!("allocated: {p:p}({layout:?})");
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        log::debug!("freeing: {:p}({:?})", ptr, layout);
        let mut head = self.head.lock();
        let current: *mut Block = core::mem::transmute(ptr);
        *current = Block {
            next: head.next,
            prev: &mut *head,
            size: layout.size(),
        };
        head.next = current;
    }
}

#[global_allocator]
static ALLOCATOR: HeapAllocator = HeapAllocator::new();

unsafe fn find_allocation(head: *mut Block, layout: core::alloc::Layout) -> *mut u8 {
    assert!(!head.is_null());
    let Block {
        ref mut prev,
        ref mut next,
        ref mut size,
    } = *head;
    let current: *mut u8 = core::mem::transmute(head);
    let initial_padding = current.align_offset(core::cmp::max(
        layout.align(),
        core::mem::align_of::<Block>(),
    ));
    let needed_size = initial_padding + layout.size();
    if needed_size > *size {
        // this block is too small, continue to next block
        if next.is_null() {
            let (ptr, size) = if needed_size <= Page4KB::LENGTH {
                let page = match Page4KB::new() {
                    Some(page) => page,
                    None => return core::ptr::null_mut(),
                };
                (page.into_raw(), Page4KB::LENGTH)
            } else if needed_size <= Page2MB::LENGTH {
                let page = match Page2MB::new() {
                    Some(page) => page,
                    None => return core::ptr::null_mut(),
                };
                (page.into_raw(), Page2MB::LENGTH)
            } else if needed_size <= Page1GB::LENGTH {
                let page = match Page1GB::new() {
                    Some(page) => page,
                    None => return core::ptr::null_mut(),
                };
                (page.into_raw(), Page1GB::LENGTH)
            } else {
                return core::ptr::null_mut();
            };
            let block: *mut Block = core::mem::transmute(ptr);
            *block = Block {
                prev: head,
                next: core::ptr::null_mut(),
                size,
            };
            *next = block;
        }
        find_allocation(*next, layout)
    } else {
        if initial_padding >= core::mem::size_of::<Block>() {
            // the padding is big enough to be its own block, so we split it and recurse on the
            // second half
            split_allocation(head, initial_padding);
            return find_allocation(*next, layout);
        }
        // since Block is aligned larger than its size, the padding now should always be zero
        assert!(initial_padding == 0);
        // this block is big enough
        let needed_size_split =
            layout.size() + layout.padding_needed_for(core::mem::align_of::<Block>());
        let leftover_size = *size - needed_size_split;
        if leftover_size >= core::mem::size_of::<Block>() {
            // split this block and add to the linked list
            split_allocation(head, needed_size_split);
        }
        // remove self from the linked list
        if !(*prev).is_null() {
            (**prev).next = *next;
        }
        if !(*next).is_null() {
            (**next).prev = *prev;
        }
        current
    }
}

unsafe fn split_allocation(head: *mut Block, bytes: usize) {
    let Block {
        prev: _,
        ref mut next,
        ref mut size,
    } = *head;
    log::trace!("splitting {head:p} into {bytes} + {}", *size - bytes);
    assert!(*size > bytes);
    let created = head.byte_add(bytes);
    (*created).size = *size - bytes;
    (*created).next = *next;
    (*created).prev = head;
    *next = created;
    if !(*next).is_null() {
        (**next).prev = created;
    }
    *size = bytes;
}

#[cfg(test)]
mod tests {
    #[test_case]
    pub fn test_alloc() {
        let mut x = alloc::vec![];
        x.push(1);
    }

    #[test_case]
    pub fn test_realloc() {
        let mut x = alloc::vec![];
        for _ in 1..1024 {
            x.push(1);
        }
    }
}
