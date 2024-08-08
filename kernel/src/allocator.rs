use core::alloc::GlobalAlloc;

/**
 * TODO:
 * - allocate more precise amounts from the buddy allocator
 */
use crate::spinlock::SpinLock;

struct HeapAllocator {
    head: SpinLock<Block>,
}

#[repr(C, align(32))]
#[derive(Debug)]
struct Block {
    prev: *mut Block,
    next: *mut Block,
    size: usize,
}

unsafe impl Send for Block {}

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
            next: core::ptr::null_mut(),
            prev: core::ptr::null_mut(),
            size: core::cmp::max(core::mem::size_of::<Block>(), layout.size()),
        };
        insert_into_free_list(&mut *head, current);
    }
}

#[global_allocator]
static ALLOCATOR: HeapAllocator = HeapAllocator::new();

unsafe fn find_allocation(head: *mut Block, layout: core::alloc::Layout) -> *mut u8 {
    assert!(!head.is_null());
    log::trace!("considering block {head:p}, {:?}", *head);
    let Block {
        ref mut prev,
        ref mut next,
        ref mut size,
    } = *head;
    assert!(prev.is_null() || *size != 0);
    let current: *mut u8 = core::mem::transmute(head);
    let initial_padding = current.align_offset(core::cmp::max(
        layout.align(),
        core::mem::align_of::<Block>(),
    ));
    let needed_size = initial_padding + layout.size();
    log::trace!(
        "layout: {layout:?}; initial_padding: {initial_padding}; needed_size: {needed_size}"
    );
    if needed_size > *size {
        // this block is too small, continue to next block
        if next.is_null() {
            // allocate another page
            let size = core::cmp::max(needed_size, crate::buddy::BuddyAllocator::MIN_ALLOCATION)
                .next_power_of_two();
            let (ptr, size) = match crate::buddy::allocate_bytes(size) {
                Some(page) => page.to_raw_parts(),
                None => return core::ptr::null_mut(),
            };
            let block: *mut Block = core::mem::transmute(ptr);
            *block = Block {
                prev: head,
                next: core::ptr::null_mut(),
                size,
            };
            log::trace!("new block: {:?}", block);
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
    assert!(!head.is_null());
    let Block {
        prev: _,
        ref mut next,
        ref mut size,
    } = *head;
    log::trace!("splitting {head:p} into {bytes} + {}", *size - bytes);
    assert!(bytes > 0);
    assert!(*size > bytes);
    let created = head.byte_add(bytes);
    (*created).size = *size - bytes;
    (*created).next = *next;
    (*created).prev = head;
    if !(*next).is_null() {
        (**next).prev = created;
    }
    *next = created;
    log::trace!(
        "split: {head:p}({:?}) and {created:p}({:?})",
        *head,
        *created
    );
    *size = bytes;
}

unsafe fn insert_into_free_list(head: *mut Block, new: *mut Block) {
    assert!(!head.is_null());
    let Block {
        prev: _,
        ref mut next,
        size: _,
    } = *head;
    if next.is_null() {
        // at the end of the list, we have to insert here
        log::trace!("inserting {:p} after {:p}", new, head);
        *next = new;
        (*new).prev = head;
    } else if *next < new {
        // this goes later in the list
        insert_into_free_list(*next, new);
        return;
    } else {
        // this is the right place in the list, insert here
        log::trace!("inserting {:p} between {:p} and {:p}", new, head, *next);
        (*new).next = *next;
        if !next.is_null() {
            (**next).prev = new;
        }
        *next = new;
        (*new).prev = head;
    }
    coalesce_blocks(new);
}

unsafe fn coalesce_blocks(head: *mut Block) {
    assert!(!head.is_null());
    let Block {
        ref mut prev,
        ref mut next,
        ref mut size,
    } = *head;
    if !prev.is_null() && prev.byte_add((**prev).size) == head {
        log::trace!(
            "coalescing {:p}({:?}) <- {:p}({:?})",
            *prev,
            **prev,
            head,
            *head
        );
        (**prev).size += *size;
        (**prev).next = *next;
        if !next.is_null() {
            (**next).prev = *prev;
        }
        log::trace!("coalesced: {:p}({:?})", *prev, **prev);
        coalesce_blocks(*prev);
    } else if !next.is_null() && head.byte_add(*size) == *next {
        log::trace!(
            "coalescing {:p}({:?}) -> {:p}({:?})",
            head,
            *head,
            *next,
            **next
        );
        *size += (**next).size;
        if !(**next).next.is_null() {
            (*(**next).next).prev = head;
        }
        *next = (**next).next;
        log::trace!("coalesced: {:p}({:?})", head, *head);
        coalesce_blocks(head);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    pub fn test_alloc() {
        let mut x = alloc::vec![];
        x.push(1);
    }

    #[test]
    pub fn test_realloc() {
        let mut x = alloc::vec![];
        for i in 1..1024 {
            x.push(i);
        }
    }

    #[bench]
    pub fn bench_alloc(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            core::mem::forget(alloc::vec![1, 2, 3, 4]);
        });
    }

    #[bench]
    pub fn bench_alloc_free(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            core::mem::drop(alloc::vec![1, 2, 3, 4]);
        });
    }
}
