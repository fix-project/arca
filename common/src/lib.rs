#![cfg_attr(not(feature = "std"), no_std)]
#![feature(new_zeroed_alloc)]
#![feature(allocator_api)]
#![feature(test)]

use core::{
    alloc::Layout,
    mem::MaybeUninit,
    ptr::NonNull,
    sync::atomic::{AtomicU64, Ordering},
};
use std::alloc::{AllocError, Allocator};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AllocationError {
    #[error("the requested allocation of order {order} at index {index} is already in use")]
    RegionInUse { index: usize, order: u32 },
    #[error("no regions of order {0} are available")]
    SpaceExhausted(u32),
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
pub struct BitSlice<'a> {
    base: &'a AtomicU64,
    length: usize,
}

impl<'a> BitSlice<'a> {
    pub fn new(slice: &'a [AtomicU64], length: usize) -> BitSlice<'a> {
        assert!(length <= core::mem::size_of_val(slice) * 8);
        BitSlice {
            base: &slice[0],
            length,
        }
    }

    pub fn bit(&self, index: usize) -> BitRef<'a> {
        assert!(index < self.length);
        BitRef {
            word: unsafe {
                &(*(self.base as *const AtomicU64)
                    .add(index / (core::mem::size_of::<AtomicU64>() * 8)))
            },
            offset: index % (core::mem::size_of::<AtomicU64>() * 8),
        }
    }

    pub fn clear_first_set(&self) -> Option<usize> {
        for i in 0..self.len().div_ceil(core::mem::size_of::<AtomicU64>() * 8) {
            let byte = unsafe { &*((self.base as *const AtomicU64).add(i)) };
            let mut value = byte.load(Ordering::SeqCst);
            while value.trailing_zeros() as usize != (core::mem::size_of::<AtomicU64>() * 8) {
                let set = 1 << value.trailing_zeros();
                let new = value & !set;
                if let Err(x) =
                    byte.compare_exchange(value, new, Ordering::SeqCst, Ordering::SeqCst)
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
        self.length
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<'a> From<&'a [AtomicU64]> for BitSlice<'a> {
    fn from(value: &'a [AtomicU64]) -> Self {
        Self::new(value, core::mem::size_of_val(value) * 8)
    }
}

impl<'a, const N: usize> From<&'a [AtomicU64; N]> for BitSlice<'a> {
    fn from(value: &'a [AtomicU64; N]) -> Self {
        Self::new(value, core::mem::size_of_val(value) * 8)
    }
}

#[repr(C)]
pub struct AllocatorLevel<'a> {
    // true = available; false = unavailable
    bitmap: BitSlice<'a>,
    order: u32,
    next_level: Option<&'a AllocatorLevel<'a>>,
}

impl<'a> AllocatorLevel<'a> {
    pub fn new(
        bitmap: BitSlice<'a>,
        order: u32,
        next_level: Option<&'a Self>,
    ) -> AllocatorLevel<'a> {
        AllocatorLevel {
            bitmap,
            order,
            next_level,
        }
    }

    pub fn reserve(&self, index: usize) -> Result<usize, AllocationError> {
        if self.bitmap.bit(index).clear(Ordering::SeqCst) {
            Ok(index)
        } else {
            Err(AllocationError::RegionInUse {
                index,
                order: self.order,
            })
        }
    }

    pub fn allocate(&self, size_log2: u32) -> Result<usize, AllocationError> {
        if size_log2 > 0 {
            return self
                .next_level
                .ok_or(AllocationError::SpaceExhausted(self.order))?
                .allocate(size_log2 - 1)
                .map(|x| 2 * x);
        }
        if let Some(index) = self.bitmap.clear_first_set() {
            return Ok(index);
        }
        self.next_level
            .and_then(|next| next.allocate(0).ok())
            .map(|i| 2 * i)
            .inspect(|i| self.free(i + 1, 0))
            .ok_or(AllocationError::SpaceExhausted(self.order))
    }

    pub fn free(&self, index: usize, size_log2: u32) {
        if size_log2 > 0 {
            assert_eq!(index % 2, 0);
            return self.next_level.unwrap().free(index / 2, size_log2 - 1);
        }
        if let Some(next) = self.next_level {
            let buddy = index ^ 1;
            if self.bitmap.bit(buddy).clear(Ordering::SeqCst) {
                next.free(index / 2, 0);
                return;
            }
        }
        assert!(!self.bitmap.bit(index).set(Ordering::SeqCst));
    }
}

#[repr(C)]
pub struct BuddyAllocator<'a> {
    start: *mut (),
    level: &'a AllocatorLevel<'a>,
    max_align: usize,
    max_level: u32,
    min_level: u32,
    raw_size: usize,
}

impl<'a> BuddyAllocator<'a> {
    #[cfg(feature = "std")]
    pub fn new(base: &'a mut [u8]) -> &'a mut BuddyAllocator<'a> {
        let raw_size = core::mem::size_of_val(base);
        let min_level = 12;
        let max_level = raw_size.ilog2();
        let start: *mut () = unsafe { core::mem::transmute(&mut base[0] as *mut u8) };
        let max_align = 1 << (start as usize).trailing_zeros();

        // allocate this on the heap for now
        let mut backing = Vec::new();
        for level in (min_level..(max_level + 1)).rev() {
            let size = backing.len();
            let words = (1 << (level - min_level)) / (core::mem::size_of::<AtomicU64>() * 8);
            let words = core::cmp::max(words, 1);
            backing.resize_with(size + words, || AtomicU64::new(0));
        }

        let temp_backing = Box::leak(backing.into_boxed_slice());
        let mut current = 0;
        let mut temp_level: Option<&AllocatorLevel<'_>> = None;
        for i in (min_level..(max_level + 1)).rev() {
            let bits: usize = 1 << (max_level - i);
            let words = bits.div_ceil(core::mem::size_of::<AtomicU64>() * 8);

            let bitmap: &[AtomicU64] = &temp_backing[current..current + words];
            current += words;

            let slice = BitSlice::from(bitmap);
            let new_level = Box::new(AllocatorLevel::new(slice, i, temp_level));
            if i == max_level {
                new_level.free(0, 0);
            }
            temp_level = Some(Box::leak(new_level));
        }

        // create a buddy allocator using the heap-backed version
        let temp = BuddyAllocator {
            start,
            level: temp_level.unwrap(),
            max_level,
            min_level,
            raw_size,
            max_align,
        };

        // allocate the relevant space within the buddy allocator itself
        let size = core::mem::size_of_val(temp_backing);
        let backing = unsafe {
            core::slice::from_raw_parts_mut(
                core::mem::transmute::<*mut (), *mut AtomicU64>(temp.allocate_raw(size)),
                temp_backing.len(),
            )
        };
        let mut current = 0;
        let mut level: Option<&'static AllocatorLevel<'static>> = None;
        for i in (min_level..(max_level + 1)).rev() {
            let bits: usize = 1 << (max_level - i);
            let words = bits.div_ceil(core::mem::size_of::<AtomicU64>() * 8);

            let bitmap: &'static [AtomicU64] = &backing[current..current + words];
            current += words;

            let slice = BitSlice::from(bitmap);
            let new_level = Box::new(AllocatorLevel::new(slice, i, level));
            level = Some(Box::leak(new_level));
        }

        let real = temp.allocate::<BuddyAllocator>(BuddyAllocator {
            start,
            level: level.unwrap(),
            max_level,
            min_level,
            raw_size,
            max_align,
        });

        // transfer the allocation information to the new backing region
        for i in 0..backing.len() {
            backing[i].store(temp_backing[i].load(Ordering::SeqCst), Ordering::SeqCst);
        }

        while let Some(level) = temp_level {
            temp_level = level.next_level;
            unsafe {
                core::mem::drop(Box::from_raw(core::mem::transmute::<
                    *const AllocatorLevel<'static>,
                    *mut AllocatorLevel<'static>,
                >(level)))
            };
        }

        unsafe {
            core::mem::drop(Box::from_raw(core::mem::transmute::<
                *const [AtomicU64],
                *mut [AtomicU64],
            >(temp_backing)));
        }

        unsafe { &mut *real }
    }

    #[cfg(feature = "std")]
    /// # Safety
    /// This function can only be called after *all* allocations that came from this allocator have
    /// been freed or forgotten, and if this is the only reference to this allocator.
    pub unsafe fn destroy(&mut self) -> &'a mut [u8] {
        core::slice::from_raw_parts_mut(
            core::mem::transmute::<*mut (), *mut u8>(self.start),
            self.raw_size,
        )
    }

    pub fn allocate_raw(&self, size: usize) -> *mut () {
        let level = size.next_power_of_two().ilog2();
        let level = core::cmp::max(self.min_level, level);
        let size_log2 = level - self.min_level;
        self.level
            .allocate(size_log2)
            .map_or(core::ptr::null_mut(), |i| unsafe {
                self.start.byte_add(i * (1 << self.min_level))
            })
    }

    pub fn allocate_uninit<T: Sized>(&self) -> *mut MaybeUninit<T> {
        let size = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        assert!(align <= self.max_align);
        let size = core::cmp::max(size, align);
        let raw = self.allocate_raw(size);
        unsafe { core::mem::transmute(raw) }
    }

    pub fn allocate<T: Sized>(&self, value: T) -> *mut T {
        let uninit = self.allocate_uninit::<T>();
        unsafe {
            (*uninit).write(value);
            (*uninit).assume_init_mut()
        }
    }

    pub fn free_raw(&self, ptr: *mut (), size: usize) {
        let level = size.next_power_of_two().ilog2();
        let level = core::cmp::max(self.min_level, level);
        let size_log2 = level - self.min_level;
        let index = (ptr as usize - self.start as usize) / (1 << self.min_level) as usize;
        self.level.free(index, size_log2);
    }

    pub fn free<T: Sized>(&self, ptr: *const T) {
        let size = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        let size = core::cmp::max(size, align);
        self.free_raw(ptr as usize as *mut (), size);
    }
}

unsafe impl<'a> Allocator for BuddyAllocator<'a> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let size = layout.size();
        let align = layout.align();
        let size = core::cmp::max(size, align);
        let raw = self.allocate_raw(size);
        let converted = unsafe {
            let raw: *mut u8 = core::mem::transmute(raw);
            core::ptr::slice_from_raw_parts_mut(raw, size)
        };
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
        let slice = BitSlice::from(&words);
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
        let region: Box<[u8; 0x10000000]> = unsafe { Box::new_zeroed().assume_init() };
        let region = Box::leak(region);
        let allocator = BuddyAllocator::new(region);

        let test = Box::new_in(10, &*allocator);
        assert_eq!(*test, 10);

        let mut v = Vec::new_in(&*allocator);
        for i in 0..10000 {
            v.push(i);
        }

        core::mem::drop(test);
        core::mem::drop(v);

        unsafe { core::mem::drop(Box::from_raw(allocator.destroy())) };
    }

    #[bench]
    fn bench_allocate_free(b: &mut Bencher) {
        let region: Box<[u8; 0x10000000]> = unsafe { Box::new_zeroed().assume_init() };
        let region = Box::leak(region);
        let allocator = BuddyAllocator::new(region);
        b.iter(|| {
            core::mem::drop(Box::new_in(0, &*allocator));
        });
        unsafe { core::mem::drop(Box::from_raw(allocator.destroy())) };
    }

    #[bench]
    fn bench_allocate_many(b: &mut Bencher) {
        let region: Box<[u8; 0x10000000]> = unsafe { Box::new_zeroed().assume_init() };
        let region = Box::leak(region);
        let allocator = BuddyAllocator::new(region);
        let mut v = vec![];
        b.iter(|| match Box::try_new_in(0, &*allocator) {
            Ok(x) => v.push(x),
            Err(_) => v.clear(),
        });
        core::mem::drop(v);
        unsafe { core::mem::drop(Box::from_raw(allocator.destroy())) };
    }
}
