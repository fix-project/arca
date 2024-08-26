#![allow(dead_code)]

use core::cell::OnceCell;
use core::mem::MaybeUninit;
use core::ptr::NonNull;

use crate::page::Page;
use crate::spinlock::SpinLock;
use crate::{multiboot, vm};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct FreeBlock {
    next: *mut FreeBlock,
    prev: *mut FreeBlock,
}

#[derive(Debug)]
pub struct BuddyAllocator {
    log2_address_space_size: usize,
    free_bitmap: &'static mut [u8],
    free_lists: [FreeBlock; 64],
}

unsafe impl Send for BuddyAllocator {}

pub static PHYSICAL_ALLOCATOR: SpinLock<OnceCell<BuddyAllocator>> = SpinLock::new(OnceCell::new());

pub(crate) unsafe fn init(mmap: multiboot::MemoryMap) {
    let cell = PHYSICAL_ALLOCATOR.lock();
    let buddy = BuddyAllocator::new(mmap);
    cell.set(buddy)
        .expect("could not initialize physical memory allocator");
}

impl BuddyAllocator {
    pub const MIN_ALLOCATION: usize = 4096;
    pub const LOG2_MIN_ALLOCATION: usize = 12;

    fn new(mmap: multiboot::MemoryMap) -> Self {
        let max_address = mmap
            .filter(|x| x.available())
            .fold(0, |a, x| core::cmp::max(a, x.base() + x.len()));
        let address_space_size = max_address.next_power_of_two();
        let log2_address_space_size = address_space_size.trailing_zeros() as usize;
        log::debug!("address space size: {address_space_size:#x} ({log2_address_space_size} bits)");
        // TODO: use the xor trick to half the space consumption
        let metadata_space = address_space_size / Self::MIN_ALLOCATION / 4;
        let metadata_align = metadata_space.trailing_zeros() as usize;

        // TODO: calculate this cutoff using GRUB's multiboot info
        let low_memory_cutoff = 16 * 1024 * 1024;
        let alignment_mask = metadata_align - 1;
        let free_bitmap_ptr = vm::pa2ka(mmap
            .filter(|x| x.available())
            .map(|x| (x.base(), x.base() + x.len()))
            .filter(|x| x.1 >= low_memory_cutoff)
            .map(|x| (core::cmp::max(x.0, low_memory_cutoff), x.1))
            .map(|x| (((x.0 + alignment_mask) & !alignment_mask), x.1))
            .filter(|x| x.0 < x.1)
            .find(|x| x.1 + x.0 >= metadata_space)
            .map(|x| x.0)
            .expect(
                "could not find satisfactory memory region for physical memory allocator metadata",
            ));
        log::debug!(
            "using {:p}+{:#x} for page allocator metadata",
            free_bitmap_ptr,
            metadata_space
        );
        let free_bitmap =
            unsafe { core::slice::from_raw_parts_mut(free_bitmap_ptr, metadata_space) };
        free_bitmap.fill(0);

        let mut alloc = BuddyAllocator {
            log2_address_space_size,
            free_bitmap,
            free_lists: [FreeBlock {
                next: core::ptr::null_mut(),
                prev: core::ptr::null_mut(),
            }; 64],
        };

        let meta_start = free_bitmap_ptr as *mut ();
        log::trace!("{meta_start:p}");
        let meta_end = unsafe { meta_start.byte_add(metadata_space) };
        for map in mmap {
            if map.available() {
                log::trace!("{:x?}", map);
                let start: *const () = vm::pa2ka(map.base());
                let end = unsafe { start.byte_add(map.len()) };
                assert!(end < vm::pa2ka(address_space_size));
                log::trace!("{start:p} {end:p}");
                let cutoff = vm::pa2ka(low_memory_cutoff);
                if end < cutoff {
                    continue;
                }
                let start = core::cmp::max(start, cutoff);
                log::trace!("{start:p} {end:p} {meta_start:p} {meta_end:p}");
                if start >= meta_end || end <= meta_start {
                    alloc.mark_free_between(start, end);
                } else {
                    alloc.mark_free_between(start, meta_start);
                    alloc.mark_free_between(meta_end, end);
                }
            }
        }

        alloc
    }

    pub fn log2_address_space_size(&self) -> usize {
        self.log2_address_space_size
    }

    pub fn address_space_size(&self) -> usize {
        1 << self.log2_address_space_size()
    }

    fn mark_free_between(&mut self, start: *const (), end: *const ()) {
        log::trace!("marking free between {start:p} and {end:p}");
        let start = vm::ka2pa(start);
        let end = vm::ka2pa(end);
        let mut start = (start + Self::LOG2_MIN_ALLOCATION - 1) & !(Self::LOG2_MIN_ALLOCATION - 1);
        let mut length = end - start;
        while length > Self::MIN_ALLOCATION {
            let log2_alignment = start.trailing_zeros();
            let log2_length = 63 - (length + 1).leading_zeros();
            let log2_block_size = core::cmp::min(log2_alignment, log2_length) as usize;
            let block_size = 1usize << log2_block_size;
            unsafe { self.free_block(vm::pa2ka(start), log2_block_size as usize) };
            start += block_size as usize;
            length = end - start;
        }
    }

    fn get_index(&self, addr: *const (), log2_size: usize) -> usize {
        let addr = vm::ka2pa(addr);
        assert!(addr.trailing_zeros() as usize >= log2_size);
        let index_in_level = addr >> log2_size;
        let level_offset = (1 << (self.log2_address_space_size - log2_size)) - 1;
        level_offset + index_in_level
    }

    fn get_allocation(&self, index: usize) -> (*mut (), usize) {
        let leading = 63 - (index + 1).leading_zeros() as usize;
        let log2_size = self.log2_address_space_size - leading;
        let level_offset = ((1 << leading) - 1) as usize;
        let addr = (index - level_offset) << log2_size;
        (vm::pa2ka(addr), log2_size)
    }

    fn is_block_free_bitmap(&self, index: usize) -> bool {
        let byte = index / 8;
        let bit = index % 8;
        (self.free_bitmap[byte] >> bit) & 1 == 1
    }

    fn set_block_free_bitmap(&mut self, index: usize, free: bool) {
        let byte = index / 8;
        let bit = index % 8;
        if free {
            self.free_bitmap[byte] |= 1 << bit;
        } else {
            self.free_bitmap[byte] &= !(1 << bit);
        }
    }

    unsafe fn free_block(&mut self, addr: *mut (), log2_size: usize) {
        assert!(log2_size < self.log2_address_space_size);
        log::trace!("freeing block {addr:p}({log2_size})");
        let index = self.get_index(addr, log2_size);
        log::trace!("freeing block @ index {index}");
        assert!(!self.is_block_free_bitmap(index));
        self.set_block_free_bitmap(index, true);
        if index != 0 {
            let buddy = ((index + 1) ^ 1) - 1;
            let (buddy_addr, buddy_log2_size) = self.get_allocation(buddy);
            assert!(buddy_log2_size == log2_size);
            log::trace!("checking buddy @ index {buddy}");
            if self.is_block_free_bitmap(buddy) {
                log::trace!(
                    "coalescing block {addr:p}({log2_size}) and buddy {buddy_addr:p}({log2_size})"
                );
                // coalesce
                let buddy: &mut FreeBlock = unsafe { core::mem::transmute(&mut *buddy_addr) };
                if !buddy.next.is_null() {
                    unsafe { (*buddy.next).prev = buddy.prev }
                }
                if !buddy.prev.is_null() {
                    unsafe { (*buddy.prev).next = buddy.next }
                }
                let addr = core::cmp::min(addr, buddy_addr);
                self.free_block(addr, log2_size + 1);
                return;
            }
        }
        // add to free list
        log::trace!("adding block @ index {index} to free list");
        let list = &mut self.free_lists[log2_size];
        let node: &mut FreeBlock = unsafe { core::mem::transmute(&mut *addr) };
        let head = list.next;
        if !head.is_null() {
            unsafe { (*head).prev = node }
        }
        node.next = head;
        node.prev = list;
        list.next = node;
        log::trace!("freed block {addr:p}({log2_size})");
    }

    pub const fn allocation_size<T: Sized>() -> usize {
        let size = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        let bigger = if size >= align { size } else { align };
        if bigger <= Self::MIN_ALLOCATION {
            Self::MIN_ALLOCATION
        } else {
            bigger.next_power_of_two()
        }
    }

    pub const fn allocation_align<T: Sized>() -> usize {
        Self::allocation_size::<T>()
    }

    pub fn allocate<T>(&mut self) -> *mut MaybeUninit<T> {
        let size = Self::allocation_size::<T>();
        unsafe { core::mem::transmute(self.alloc_block(size.trailing_zeros() as usize)) }
    }

    /// # Safety
    /// This pointer must have come from [allocate], and must have been allocated as a layout-compatible type.
    pub unsafe fn liberate<T>(&mut self, ptr: *mut T) {
        let size = Self::allocation_size::<T>();
        unsafe {
            self.free_block(
                core::mem::transmute::<*mut T, *mut ()>(ptr),
                size.trailing_zeros() as usize,
            )
        };
    }

    fn alloc_block(&mut self, log2_size: usize) -> *mut () {
        log::trace!("allocating block ({log2_size})");
        if log2_size > self.log2_address_space_size {
            log::error!("block too large");
            return core::ptr::null_mut();
        }
        let list = &mut self.free_lists[log2_size];
        let head = list.next;
        if !head.is_null() {
            log::trace!("found free list");
            let ptr: *mut () = unsafe {
                list.next = (*head).next;
                if !list.next.is_null() {
                    (*list.next).prev = list;
                }
                core::mem::transmute::<*mut FreeBlock, *mut ()>(head)
            };
            let index = self.get_index(ptr, log2_size);
            assert!(self.is_block_free_bitmap(index));
            self.set_block_free_bitmap(index, false);
            log::trace!("allocated {ptr:p}({log2_size})");
            return ptr;
        }
        log::trace!("no blocks on free list, recursing to ({})", log2_size + 1);
        let bigger = self.alloc_block(log2_size + 1);
        log::trace!("bigger: {:p}", bigger);
        if bigger.is_null() {
            log::error!("recursive allocation of ({}) failed", log2_size + 1);
            return bigger;
        }
        log::trace!("splitting block {bigger:p}({})", log2_size + 1);
        let upper = unsafe { bigger.byte_add(1 << log2_size) };
        let lower = bigger;

        let index_h = self.get_index(upper, log2_size);
        let index_l = self.get_index(lower, log2_size);
        // assert!(self.is_block_free_bitmap(index_h));
        // assert!(self.is_block_free_bitmap(index_l));
        self.set_block_free_bitmap(index_h, false);
        self.set_block_free_bitmap(index_l, false);

        unsafe { self.free_block(lower, log2_size) };
        log::trace!("allocated {upper:p}({log2_size})");
        upper
    }
}

pub fn allocate<T>() -> Option<NonNull<MaybeUninit<T>>> {
    let mut lock = PHYSICAL_ALLOCATOR.lock();
    let buddy = lock.get_mut().expect("physical allocator not initialized");
    let base = buddy.allocate::<T>();
    NonNull::new(base)
}

pub fn allocate_bytes(n: usize) -> Option<NonNull<[u8]>> {
    let mut lock = PHYSICAL_ALLOCATOR.lock();
    let buddy = lock.get_mut().expect("physical allocator not initialized");
    let size = n.next_power_of_two();
    let base = buddy.alloc_block(size.trailing_zeros() as usize);
    Some(NonNull::slice_from_raw_parts(
        NonNull::new(base as *mut u8)?,
        size,
    ))
}

/// # Safety
/// This pointer must have been allocated using [allocate] as the same type, or a type with
/// the same layout.
pub unsafe fn liberate<T>(ptr: *mut T) {
    let mut lock = PHYSICAL_ALLOCATOR.lock();
    let buddy = lock.get_mut().expect("physical allocator not initialized");
    unsafe { buddy.liberate(ptr) }
}

/// # Safety
/// This slice must have been allocated using [allocate_bytes] with the same size.
pub unsafe fn liberate_bytes(ptr: *mut [u8]) {
    let mut lock = PHYSICAL_ALLOCATOR.lock();
    let buddy = lock.get_mut().expect("physical allocator not initialized");
    let (ptr, len) = ptr.to_raw_parts();
    unsafe { buddy.free_block(ptr, len.trailing_zeros() as usize) }
}

pub type Page4KB = Page<[u8; 1 << 12]>;
pub type Page2MB = Page<[u8; 1 << 21]>;
pub type Page1GB = Page<[u8; 1 << 30]>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_alloc() {
        Page2MB::new();
    }

    #[bench]
    pub fn bench_alloc_free_2mb(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            let _ = Page2MB::new();
        });
    }
}
