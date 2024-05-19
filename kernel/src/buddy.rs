#![allow(dead_code)]

use core::cell::OnceCell;
use core::ops::{Deref, DerefMut};

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

pub static PHYSICAL_ALLOCATOR: SpinLock<OnceCell<BuddyAllocator>> = SpinLock::new(OnceCell::new());

pub unsafe fn init(mmap: multiboot::MemoryMap) {
    let cell = PHYSICAL_ALLOCATOR.lock();
    let buddy = BuddyAllocator::new(mmap);
    cell.set(buddy)
        .expect("could not initialize physical memory allocator");
}

impl BuddyAllocator {
    const MIN_ALLOCATION: usize = 4096;
    const LOG2_MIN_ALLOCATION: usize = 12;

    fn new(mmap: multiboot::MemoryMap) -> Self {
        let max_address = vm::ka2pa(mmap.fold(core::ptr::null(), |a, x| {
            core::cmp::max(a, unsafe { x.base().add(x.len()) })
        }));
        let address_space_size = (max_address as usize).next_power_of_two();
        let log2_address_space_size = address_space_size.trailing_zeros() as usize;
        // TODO: something is being miscalculated here...
        let metadata_space = address_space_size / Self::MIN_ALLOCATION / 4;
        let metadata_align = metadata_space.trailing_zeros() as usize;

        let low_memory_cutoff = 16 * 1024 * 1024;
        let alignment_mask = metadata_align - 1;
        let free_bitmap_p = vm::pa2ka(mmap
            .filter(|x| x.available())
            .map(|x| {
                let base = x.base() as usize;
                (base, base + x.len())
            })
            .filter(|x| x.1 >= low_memory_cutoff)
            .map(|x| (core::cmp::max(x.0, low_memory_cutoff), x.1))
            .map(|x| ((x.0 + alignment_mask) & !alignment_mask, x.1))
            .filter(|x| x.0 < x.1)
            .find(|x| x.1 - x.0 >= metadata_space)
            .map(|x| x.0)
            .expect(
                "could not find satisfactory memory region for physical memory allocator metadata",
            ) as *mut u8);
        let free_bitmap =
            unsafe { core::slice::from_raw_parts_mut(free_bitmap_p as *mut u8, metadata_space) };
        log::info!(
            "using {:p}-{:p} for page allocator metadata",
            free_bitmap_p,
            unsafe { free_bitmap_p.add(metadata_space) }
        );
        free_bitmap.fill(0);

        let mut alloc = BuddyAllocator {
            log2_address_space_size,
            free_bitmap,
            free_lists: [FreeBlock {
                next: core::ptr::null_mut(),
                prev: core::ptr::null_mut(),
            }; 64],
        };

        for map in mmap {
            if map.available() {
                let start = vm::ka2pa(map.base()) as usize;
                let end = start + map.len();
                if end < low_memory_cutoff {
                    continue;
                }
                let start = core::cmp::max(start, low_memory_cutoff);
                let start =
                    (start + Self::LOG2_MIN_ALLOCATION - 1) & !(Self::LOG2_MIN_ALLOCATION - 1);
                let length = end - start;
                let length = (length / Self::MIN_ALLOCATION) * Self::MIN_ALLOCATION;
                for i in 0..length / Self::MIN_ALLOCATION {
                    let addr = (start + i * Self::MIN_ALLOCATION) as *mut u8;
                    if (addr as usize) >= vm::ka2pa(free_bitmap_p) as usize
                        && (addr as usize) < vm::ka2pa(free_bitmap_p) as usize + metadata_space
                    {
                        continue;
                    }
                    unsafe { alloc.free_block(addr, Self::LOG2_MIN_ALLOCATION) };
                }
            }
        }

        alloc
    }

    fn get_index(&self, addr: *const u8, log2_size: usize) -> usize {
        let addr = vm::ka2pa(addr) as usize;
        assert!(addr.trailing_zeros() as usize >= log2_size);
        let index_in_level = addr >> log2_size;
        let level_offset = (1 << (self.log2_address_space_size - log2_size)) - 1;
        level_offset + index_in_level
    }

    fn get_allocation(&self, index: usize) -> (*mut u8, usize) {
        let leading = 63 - (index + 1).leading_zeros() as usize;
        let log2_size = self.log2_address_space_size - leading;
        let level_offset = ((1 << leading) - 1) as usize;
        let addr = (index - level_offset) << log2_size;
        (addr as *mut u8, log2_size)
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

    unsafe fn free_block(&mut self, addr: *mut u8, log2_size: usize) {
        assert!(log2_size < self.log2_address_space_size);
        log::trace!("freeing block {addr:p}({log2_size})");
        let index = self.get_index(addr, log2_size);
        assert!(!self.is_block_free_bitmap(index));
        self.set_block_free_bitmap(index, true);
        if index != 0 {
            let buddy = ((index + 1) ^ 1) - 1;
            let (buddy_addr, buddy_log2_size) = self.get_allocation(buddy);
            assert!(buddy_log2_size == log2_size);
            if self.is_block_free_bitmap(buddy) {
                log::trace!(
                    "coalescing block {addr:p}({log2_size}) and buddy {buddy_addr:p}({log2_size})"
                );
                // coalesce
                let buddy: &mut FreeBlock =
                    unsafe { core::mem::transmute(&mut *vm::pa2ka_mut(buddy_addr)) };
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

    fn alloc_block(&mut self, log2_size: usize) -> *mut u8 {
        log::trace!("allocating block ({log2_size})");
        if log2_size > self.log2_address_space_size {
            log::error!("block too large");
            return core::ptr::null_mut();
        }
        let list = &mut self.free_lists[log2_size];
        let head = list.next;
        if !head.is_null() {
            log::trace!("found free list");
            let ptr: *mut u8 = unsafe {
                list.next = (*head).next;
                (*list.next).prev = list;
                core::mem::transmute::<*mut FreeBlock, *mut u8>(head)
            };
            let index = self.get_index(ptr, log2_size);
            assert!(self.is_block_free_bitmap(index));
            self.set_block_free_bitmap(index, false);
            log::trace!("allocated {ptr:p}({log2_size})");
            return ptr;
        }
        log::trace!("no blocks on free list, recursing to ({})", log2_size + 1);
        let bigger = self.alloc_block(log2_size + 1);
        if bigger.is_null() {
            log::error!("recursive allocation of ({}) failed", log2_size + 1);
            return bigger;
        }
        log::trace!("splitting block {bigger:p}({})", log2_size + 1);
        let upper = unsafe { bigger.add(1 << log2_size) };
        let lower = bigger;

        let index_h = self.get_index(upper, log2_size);
        let index_l = self.get_index(lower, log2_size);
        assert!(self.is_block_free_bitmap(index_h));
        assert!(self.is_block_free_bitmap(index_l));
        self.set_block_free_bitmap(index_h, false);
        self.set_block_free_bitmap(index_l, false);

        unsafe { self.free_block(lower, log2_size) };
        log::trace!("allocated {upper:p}({log2_size})");
        upper
    }
}

pub fn allocate<const N: usize>() -> Option<Block<N>> {
    let mut lock = PHYSICAL_ALLOCATOR.lock();
    let buddy = lock.get_mut().expect("physical allocator not initialized");
    let base = buddy.alloc_block(N);
    if base.is_null() {
        None
    } else {
        Some(Block { base })
    }
}

pub fn liberate<const N: usize>(block: Block<N>) {
    let mut lock = PHYSICAL_ALLOCATOR.lock();
    let buddy = lock.get_mut().expect("physical allocator not initialized");
    unsafe { buddy.free_block(block.base, N) }
}

#[derive(Debug)]
pub struct Block<const N: usize> {
    base: *mut u8,
}

pub type Page4KB = Block<12>;
pub type Page2MB = Block<21>;
pub type Page1GB = Block<30>;

#[derive(Debug)]
pub enum Page {
    FourKB(Page4KB),
    TwoMB(Page2MB),
    OneGB(Page1GB),
}

impl<const N: usize> Block<N> {
    pub fn new() -> Option<Block<N>> {
        allocate::<N>()
    }

    pub fn kernel(&self) -> *mut u8 {
        vm::pa2ka_mut(self.physical())
    }

    pub fn physical(&self) -> *mut u8 {
        self.base
    }
}

impl<const N: usize> Drop for Block<N> {
    fn drop(&mut self) {
        let mut lock = PHYSICAL_ALLOCATOR.lock();
        let buddy = lock.get_mut().expect("physical allocator not initialized");
        unsafe { buddy.free_block(self.base, N) }
    }
}

impl<const N: usize> Deref for Block<N> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(vm::pa2ka(self.base), 1 << N) }
    }
}

impl<const N: usize> DerefMut for Block<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(vm::pa2ka_mut(self.base), 1 << N) }
    }
}
