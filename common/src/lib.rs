#![cfg_attr(not(feature = "std"), no_std)]
#![feature(allocator_api)]
#![feature(new_range_api)]
#![feature(test)]
#![feature(alloc_layout_extra)]
#![feature(ptr_metadata)]
#![feature(ptr_sub_ptr)]
#![cfg_attr(test, feature(new_zeroed_alloc))]

use core::{
    alloc::Layout,
    mem::MaybeUninit,
    ptr::NonNull,
    range::Range,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};
extern crate alloc;
use alloc::alloc::{AllocError, Allocator, Global};
use alloc::boxed::Box;

use snafu::prelude::*;

#[derive(Snafu, Debug)]
pub enum AllocationError {
    #[snafu(display(
        "the requested reservation at index {index} of size {size} is already in use"
    ))]
    RegionInUse { index: usize, size: usize },
    #[snafu(display("the requested reservation at index{index} of size {size} is invalid"))]
    InvalidReservation { index: usize, size: usize },
    #[snafu(display("no regions of size {size} are available"))]
    SpaceExhausted { size: usize },
}

#[repr(C)]
pub struct BitRef<'a> {
    word: &'a AtomicU64,
    offset: usize,
}

impl<'a> BitRef<'a> {
    pub fn new(word: &'a AtomicU64, offset: usize) -> BitRef<'a> {
        assert!(offset < core::mem::size_of::<AtomicU64>() * 8);
        BitRef { word, offset }
    }

    pub fn load(&self, ordering: Ordering) -> bool {
        ((self.word.load(ordering) >> self.offset) & 1) == 1
    }

    pub fn set(&self, ordering: Ordering) -> bool {
        loop {
            let old = self.word.load(ordering);
            let new = old | (1 << self.offset);
            if let Ok(word) = self
                .word
                .compare_exchange(old, new, ordering, Ordering::SeqCst)
            {
                return ((word >> self.offset) & 1) == 1;
            }
        }
    }

    pub fn clear(&self, ordering: Ordering) -> bool {
        loop {
            let old = self.word.load(ordering);
            let new = old & !(1 << self.offset);
            if let Ok(word) = self
                .word
                .compare_exchange(old, new, ordering, Ordering::SeqCst)
            {
                return ((word >> self.offset) & 1) == 1;
            }
        }
    }

    pub fn store(&self, value: bool, ordering: Ordering) -> bool {
        if value {
            self.set(ordering)
        } else {
            self.clear(ordering)
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct BitSlice<'a> {
    bits: usize,
    data: &'a [AtomicU64],
}

impl<'a> BitSlice<'a> {
    pub fn new(bits: usize, data: &'a [AtomicU64]) -> BitSlice<'a> {
        assert!(bits <= core::mem::size_of_val(data) * 8);
        BitSlice { bits, data }
    }

    pub fn bit(&self, index: usize) -> BitRef<'_> {
        assert!(index < self.bits);
        BitRef {
            word: &self.data[index / (core::mem::size_of::<AtomicU64>() * 8)],
            offset: index % (core::mem::size_of::<AtomicU64>() * 8),
        }
    }

    pub fn clear_first_set(&self) -> Option<usize> {
        for i in 0..self.len().div_ceil(core::mem::size_of::<AtomicU64>() * 8) {
            let word = &self.data[i];
            let mut value = word.load(Ordering::SeqCst);
            while value.trailing_zeros() as usize != (core::mem::size_of::<AtomicU64>() * 8) {
                let set = 1 << value.trailing_zeros();
                let new = value & !set;
                if let Err(x) =
                    word.compare_exchange(value, new, Ordering::SeqCst, Ordering::SeqCst)
                {
                    value = x;
                } else {
                    return Some(
                        i * (core::mem::size_of::<AtomicU64>() * 8)
                            + value.trailing_zeros() as usize,
                    );
                }
            }
        }
        None
    }

    pub fn len(&self) -> usize {
        self.bits
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[repr(C)]
pub struct AllocatorLevel<'a> {
    bits: usize,
    // true = available; false = unavailable
    bitmap: &'a [AtomicU64],
}

impl<'a> AllocatorLevel<'a> {
    pub fn new(bits: usize, bitmap: &'a [AtomicU64]) -> Self {
        Self { bits, bitmap }
    }

    fn bits(&self) -> BitSlice<'_> {
        BitSlice::new(self.bits, self.bitmap)
    }

    pub fn reserve(&self, index: usize) -> bool {
        self.bits().bit(index).clear(Ordering::SeqCst)
    }

    pub fn allocate(&self) -> Option<usize> {
        self.bits().clear_first_set()
    }

    pub fn free(&self, index: usize) {
        assert!(
            !self.bits().bit(index).set(Ordering::SeqCst),
            "allocation was already free!"
        );
    }
}

#[repr(C)]
#[derive(Debug)]
struct AllocatorMetadata {
    refcnt: AtomicUsize,
    used_size: AtomicUsize,
    raw_size: usize,
    total_size: usize,
    max_align: usize,
    level_range: Range<u32>,
}

#[repr(C)]
pub struct AllocatorInner {
    meta: AllocatorMetadata,
    data: [AtomicU64],
}

impl AllocatorInner {
    pub fn new(slice: &mut [u8]) -> Box<AllocatorInner> {
        Self::new_in(slice, Global)
    }

    pub fn new_in<A: Allocator>(slice: &mut [u8], allocator: A) -> Box<AllocatorInner, A> {
        let raw_size = core::mem::size_of_val(slice);
        let min_level = 12;
        let max_level = raw_size.ilog2();
        let total_size = 1 << max_level;
        let max_align = 1 << (&raw const slice[0] as usize).trailing_zeros();

        let level_range = Range {
            start: min_level,
            end: max_level + 1,
        };

        let levels = max_level - min_level + 1;
        let space = (1 << (levels - 6)) + 6 - 1;

        unsafe {
            let layout = Layout::new::<AllocatorMetadata>()
                .extend(Layout::new::<AtomicU64>().repeat(space).unwrap().0)
                .unwrap()
                .0
                .pad_to_align();
            let p = allocator
                .allocate_zeroed(layout)
                .expect("could not allocate space for allocator bitmap");
            let p: *mut AllocatorInner = core::mem::transmute(p);
            (&raw mut (*p).meta).write(AllocatorMetadata {
                refcnt: AtomicUsize::new(1),
                used_size: AtomicUsize::new(0),
                raw_size,
                total_size,
                max_align,
                level_range,
            });
            (*p).data[0].store(1, Ordering::SeqCst);
            Box::from_raw_in(p, allocator)
        }
    }

    pub fn len_bytes(&self) -> usize {
        let Range { start, end } = self.meta.level_range;
        let levels = end - start;
        let space = (1 << (levels - 6)) + 6 - 1;
        let layout = Layout::new::<AllocatorMetadata>()
            .extend(Layout::new::<AtomicU64>().repeat(space).unwrap().0)
            .unwrap()
            .0
            .pad_to_align();
        layout.size()
    }

    pub fn size_of_level_bits(&self, level: u32) -> usize {
        assert!(self.meta.level_range.contains(&level));
        let inverse = self.meta.level_range.end - level - 1;
        1 << inverse
    }

    pub fn size_of_level_words(&self, level: u32) -> usize {
        self.size_of_level_bits(level)
            .div_ceil(core::mem::size_of::<AtomicU64>())
    }

    pub fn offset_of_level_words(&self, level: u32) -> usize {
        assert!(self.meta.level_range.contains(&level));
        let inverse = self.meta.level_range.end - level - 1;
        if inverse <= 6 {
            inverse as usize
        } else {
            let power = inverse - 6;
            (1 << (power)) + 6 - 1
        }
    }

    pub fn level(&self, level: u32) -> AllocatorLevel<'_> {
        let start = self.offset_of_level_words(level);
        let size = self.size_of_level_words(level);
        let end = start + size;
        AllocatorLevel::new(self.size_of_level_bits(level), &self.data[start..end])
    }

    pub fn reserve(&self, index: usize, size_log2: u32) -> Result<usize, AllocationError> {
        assert_ne!(self.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(size_log2, self.meta.level_range.start);
        if size_log2 >= self.meta.level_range.end {
            return Err(AllocationError::InvalidReservation {
                index,
                size: 1 << size_log2,
            });
        }
        let level = self.level(size_log2);
        if level.reserve(index) {
            Ok(index)
        } else {
            match self.reserve(index / 2, size_log2 + 1) {
                Ok(_) => {
                    self.free(index ^ 1, size_log2);
                    Ok(index)
                }
                Err(_) => Err(AllocationError::RegionInUse {
                    index,
                    size: 1 << size_log2,
                }),
            }
        }
    }

    pub fn allocate(&self, size_log2: u32) -> Result<usize, AllocationError> {
        assert_ne!(self.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(size_log2, self.meta.level_range.start);
        if size_log2 >= self.meta.level_range.end {
            return Err(AllocationError::SpaceExhausted {
                size: 1 << size_log2,
            });
        }
        let level = self.level(size_log2);
        if let Some(index) = level.allocate() {
            Ok(index)
        } else {
            match self.allocate(size_log2 + 1) {
                Ok(index) => {
                    let index = 2 * index;
                    // bias allocations towards the upper end of the address space
                    self.free(index, size_log2);
                    Ok(index + 1)
                }
                Err(_) => Err(AllocationError::SpaceExhausted {
                    size: 1 << size_log2,
                }),
            }
        }
    }

    pub fn free(&self, index: usize, size_log2: u32) {
        assert_ne!(self.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(size_log2, self.meta.level_range.start);
        assert!(size_log2 < self.meta.level_range.end);
        let level = self.level(size_log2);
        if size_log2 == self.meta.level_range.end - 1 {
            level.free(index);
            return;
        }
        let buddy = index ^ 1;
        if level.reserve(buddy) {
            self.free(index / 2, size_log2 + 1);
        } else {
            // Possible race condition: we and our buddy could be trying to free at once.  Both
            // would see the other as being in-use and get here.  This isn't incorrect, as both
            // blocks do get freed, but leads to a missed coalescence.
            level.free(index);
        }
    }
}

#[repr(C)]
pub struct BuddyAllocator<'a> {
    start: *mut (),
    inner: &'a AllocatorInner,
}

unsafe impl Send for BuddyAllocator<'_> {}

impl<'a> BuddyAllocator<'a> {
    #[cfg(feature = "std")]
    pub fn new(base: &'a mut [u8]) -> BuddyAllocator<'a> {
        let start = &raw mut base[0] as *mut ();
        // allocate on the normal heap
        let inner = AllocatorInner::new(base);
        let temp = BuddyAllocator {
            start,
            inner: &inner,
        };

        let new_inner = AllocatorInner::new_in(base, &temp);

        let inner = unsafe {
            let new_inner = Box::into_raw(new_inner);
            let src = &raw const *inner;
            let dst = &raw mut *new_inner;
            let (src, src_size) = src.to_raw_parts();
            let (dst, dst_size) = dst.to_raw_parts();
            assert_eq!(src_size, dst_size);
            core::ptr::copy_nonoverlapping(src as *mut u8, dst as *mut u8, src_size);
            &*new_inner
        };
        // unimplemented!();

        BuddyAllocator { start, inner }
    }

    pub fn reserve_raw(&self, address: usize, size: usize) -> *mut () {
        let level = size.next_power_of_two().ilog2();
        assert_ne!(self.inner.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(self.inner.meta.level_range.start, level);
        let index = address >> size_log2;
        self.inner
            .reserve(index, size_log2)
            .inspect(|_| {
                self.inner
                    .meta
                    .used_size
                    .fetch_add(1 << level, Ordering::SeqCst);
            })
            .map_or(core::ptr::null_mut(), |i| {
                self.start.wrapping_byte_add(i * (1 << size_log2))
            })
    }

    pub fn reserve_uninit<T: Sized>(&self, address: usize) -> *mut MaybeUninit<T> {
        let size = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        assert!(align <= self.inner.meta.max_align);
        let size = core::cmp::max(size, align);
        let raw = self.reserve_raw(address, size);
        raw as *mut MaybeUninit<T>
    }

    pub fn reserve<T: Sized>(&self, address: usize, value: T) -> *mut T {
        let uninit = self.reserve_uninit::<T>(address);
        if uninit.is_null() {
            core::ptr::null_mut()
        } else {
            unsafe { (*uninit).write(value) }
        }
    }

    pub fn allocate_raw(&self, size: usize) -> *mut () {
        let level = size.next_power_of_two().ilog2();
        assert_ne!(self.inner.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(self.inner.meta.level_range.start, level);
        self.inner
            .allocate(size_log2)
            .inspect(|_| {
                self.inner
                    .meta
                    .used_size
                    .fetch_add(1 << level, Ordering::SeqCst);
            })
            .map_or(core::ptr::null_mut(), |i| {
                self.start.wrapping_byte_add(i * (1 << size_log2))
            })
    }

    pub fn allocate_uninit<T: Sized>(&self) -> *mut MaybeUninit<T> {
        let size = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        assert!(align <= self.inner.meta.max_align);
        let size = core::cmp::max(size, align);
        self.allocate_raw(size) as *mut MaybeUninit<T>
    }

    pub fn allocate<T: Sized>(&self, value: T) -> *mut T {
        let uninit = self.allocate_uninit::<T>();
        if uninit.is_null() {
            core::ptr::null_mut()
        } else {
            unsafe { (*uninit).write(value) }
        }
    }

    pub fn free_raw(&self, ptr: *mut (), size: usize) {
        let level = size.next_power_of_two().ilog2();
        assert_ne!(self.inner.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(self.inner.meta.level_range.start, level);
        let index = (ptr as usize - self.start as usize) / (1 << size_log2) as usize;
        self.inner.free(index, size_log2);
        self.inner
            .meta
            .used_size
            .fetch_sub(1 << size_log2, Ordering::SeqCst);
    }

    pub fn free<T: Sized>(&self, ptr: *const T) {
        let size = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        let size = core::cmp::max(size, align);
        self.free_raw(ptr as usize as *mut (), size);
    }

    pub fn free_slice<T>(&self, ptr: *const [T]) {
        let size = core::mem::size_of::<T>() * ptr.len();
        let align = core::mem::align_of::<T>();
        let size = core::cmp::max(size, align);
        self.free_raw(ptr as *const T as *mut (), size);
    }

    pub fn to_offset<T: ?Sized>(&self, allocation: *const T) -> isize {
        let base = self.start;
        let current = allocation as *const ();
        current as isize - base as isize
    }

    pub fn from_offset<T>(&self, offset: isize) -> *const T {
        let base = self.start;
        (base as isize + offset) as *const T
    }

    pub fn used_size(&self) -> usize {
        self.inner.meta.used_size.load(Ordering::SeqCst)
    }

    pub fn total_size(&self) -> usize {
        1 << (self.inner.meta.level_range.end - 1)
    }

    pub fn usage(&self) -> f64 {
        self.used_size() as f64 / self.total_size() as f64
    }

    pub unsafe fn into_raw_parts(self) -> (*mut (), usize, usize) {
        let BuddyAllocator { start, inner } = self;
        let p = inner as *const AllocatorInner;
        let (base, meta) = p.to_raw_parts();

        core::mem::forget(self);
        (start, base.byte_sub_ptr(start), meta)
    }

    pub unsafe fn from_raw_parts(start: *mut (), data_offset: usize, data_size: usize) -> Self {
        let ptr = start.byte_add(data_offset);
        BuddyAllocator {
            start,
            inner: &*core::ptr::from_raw_parts(ptr, data_size),
        }
    }

    pub fn destroy(&mut self) -> Option<&'a mut [u8]> {
        if self.inner.meta.refcnt.fetch_sub(1, Ordering::SeqCst) == 1 {
            unsafe {
                Some(core::slice::from_raw_parts_mut(
                    self.start as *mut u8,
                    self.inner.meta.raw_size,
                ))
            }
        } else {
            None
        }
    }
}

impl Clone for BuddyAllocator<'_> {
    fn clone(&self) -> Self {
        self.inner.meta.refcnt.fetch_add(1, Ordering::SeqCst);
        Self {
            start: self.start,
            inner: self.inner,
        }
    }
}

impl Drop for BuddyAllocator<'_> {
    fn drop(&mut self) {
        self.destroy();
    }
}

unsafe impl Allocator for BuddyAllocator<'_> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let size = layout.size();
        let align = layout.align();
        let size = core::cmp::max(size, align);
        let raw = self.allocate_raw(size);
        let converted = core::ptr::slice_from_raw_parts_mut(raw as *mut u8, size);
        NonNull::new(converted).ok_or(AllocError)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let size = layout.size();
        let align = layout.align();
        let size = core::cmp::max(size, align);
        self.free_raw(ptr.as_ptr() as usize as *mut (), size)
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use super::*;
    use test::Bencher;

    #[test]
    fn test_bitref() {
        let byte = AtomicU64::new(10);
        let r0 = BitRef::new(&byte, 0);
        let r1 = BitRef::new(&byte, 1);
        r0.set(Ordering::SeqCst);
        assert_eq!(byte.load(Ordering::SeqCst), 11);
        r1.clear(Ordering::SeqCst);
        assert_eq!(byte.load(Ordering::SeqCst), 9);
        r0.store(false, Ordering::SeqCst);
        r1.store(true, Ordering::SeqCst);
        assert_eq!(byte.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_bitslice() {
        let words = [const { AtomicU64::new(0) }; 2];
        let slice = BitSlice::new(128, &words);
        let r0 = slice.bit(0);
        let r1 = slice.bit(1);
        let r127 = slice.bit(127);

        r0.set(Ordering::SeqCst);
        assert_eq!(words[0].load(Ordering::SeqCst), 1);
        r1.set(Ordering::SeqCst);
        assert_eq!(words[0].load(Ordering::SeqCst), 3);
        r127.set(Ordering::SeqCst);
        assert_eq!(words[0].load(Ordering::SeqCst), 3);
        assert_eq!(
            words[127 / (core::mem::size_of::<AtomicU64>() * 8)].load(Ordering::SeqCst),
            1 << (127 % (core::mem::size_of::<AtomicU64>() * 8))
        );
    }

    #[test]
    fn test_buddy_allocator() {
        let mut region: Box<[u8; 0x10000000]> = unsafe { Box::new_zeroed().assume_init() };
        let allocator = BuddyAllocator::new(&mut *region);

        let test = Box::new_in(10, &allocator);
        assert_eq!(*test, 10);

        let mut v = Vec::new_in(&allocator);
        for i in 0..10000 {
            v.push(i);
        }
    }

    #[bench]
    fn bench_allocate_free(b: &mut Bencher) {
        let region: Box<[u8; 0x100000000]> = unsafe { Box::new_zeroed().assume_init() };
        let region = Box::leak(region);
        let allocator = BuddyAllocator::new(region);
        b.iter(|| {
            let x: Box<[MaybeUninit<u8>], &BuddyAllocator> =
                Box::new_uninit_slice_in(4096, &allocator);
            core::mem::drop(x);
        });
    }
}
