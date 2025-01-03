#![cfg_attr(not(feature = "std"), no_std)]
#![feature(allocator_api)]
#![feature(test)]

use core::{
    alloc::{AllocError, Allocator, Layout},
    marker::PhantomData,
    mem::MaybeUninit,
    ptr::NonNull,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

use snafu::prelude::*;

#[derive(Snafu, Debug)]
pub enum AllocationError {
    #[snafu(display(
        "the requested allocation of order {order} at index {index} is already in use"
    ))]
    RegionInUse { index: usize, order: u32 },
    #[snafu(display(
        "the requested reservation of size {size} at index {index} is invalid at order {order}"
    ))]
    InvalidReservation {
        index: usize,
        size: usize,
        order: u32,
    },
    #[snafu(display("no regions of order {order} are available"))]
    SpaceExhausted { order: u32 },
}

#[repr(C)]
pub struct BitRef<'a> {
    word_offset: isize,
    _phantom: PhantomData<&'a AtomicU64>,
    offset: usize,
}

impl<'a> BitRef<'a> {
    fn word(&self, start: *const ()) -> &AtomicU64 {
        (unsafe { &*((start as isize + self.word_offset) as *const AtomicU64) }) as _
    }

    pub fn new(start: *const (), word: &'a AtomicU64, offset: usize) -> BitRef<'a> {
        assert!(offset < core::mem::size_of::<AtomicU64>() * 8);
        BitRef {
            _phantom: PhantomData,
            word_offset: word as *const AtomicU64 as isize - start as isize,
            offset,
        }
    }

    pub fn load(&self, start: *const (), ordering: Ordering) -> bool {
        ((self.word(start).load(ordering) >> self.offset) & 1) == 1
    }

    pub fn set(&self, start: *const (), ordering: Ordering) -> bool {
        loop {
            let old = self.word(start).load(ordering);
            let new = old | (1 << self.offset);
            if let Ok(word) =
                self.word(start)
                    .compare_exchange(old, new, ordering, Ordering::SeqCst)
            {
                return ((word >> self.offset) & 1) == 1;
            }
        }
    }

    pub fn clear(&self, start: *const (), ordering: Ordering) -> bool {
        loop {
            let old = self.word(start).load(ordering);
            let new = old & !(1 << self.offset);
            if let Ok(word) =
                self.word(start)
                    .compare_exchange(old, new, ordering, Ordering::SeqCst)
            {
                return ((word >> self.offset) & 1) == 1;
            }
        }
    }

    pub fn store(&self, start: *const (), value: bool, ordering: Ordering) -> bool {
        if value {
            self.set(start, ordering)
        } else {
            self.clear(start, ordering)
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct BitSlice<'a> {
    _phantom: PhantomData<&'a AtomicU64>,
    base_offset: isize,
    length: usize,
}

impl<'a> BitSlice<'a> {
    pub fn base(&self, start: *const ()) -> &AtomicU64 {
        unsafe { &*((start as isize + self.base_offset) as *const AtomicU64) }
    }

    pub fn new(start: *const (), slice: &'a [AtomicU64], length: Option<usize>) -> BitSlice<'a> {
        let length = length.unwrap_or_else(|| core::mem::size_of_val(slice) * 8);
        assert!(length <= core::mem::size_of_val(slice) * 8);
        BitSlice {
            _phantom: PhantomData,
            base_offset: &slice[0] as *const AtomicU64 as isize - start as isize,
            length,
        }
    }

    pub fn bit(&self, _start: *const (), index: usize) -> BitRef<'a> {
        assert!(index < self.length);
        BitRef {
            _phantom: PhantomData,
            word_offset: self.base_offset
                + (index / (core::mem::size_of::<AtomicU64>() * 8)) as isize,
            offset: index % (core::mem::size_of::<AtomicU64>() * 8),
        }
    }

    pub fn clear_first_set(&self, start: *const ()) -> Option<usize> {
        for i in 0..self.len().div_ceil(core::mem::size_of::<AtomicU64>() * 8) {
            let byte = unsafe { &*((self.base(start) as *const AtomicU64).wrapping_add(i)) };
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

#[repr(C)]
#[derive(Debug)]
pub struct AllocatorLevel<'a> {
    // true = available; false = unavailable
    bitmap: BitSlice<'a>,
    order: u32,
    next_level: Option<isize>,
    _phantom: PhantomData<&'a AllocatorLevel<'a>>,
}

impl<'a> AllocatorLevel<'a> {
    fn next_level(&self, start: *const ()) -> Option<&'a Self> {
        self.next_level
            .map(|x| unsafe { &*((start as isize + x) as *const Self) })
    }

    pub fn new(
        start: *const (),
        bitmap: BitSlice<'a>,
        order: u32,
        next_level: Option<&'a Self>,
    ) -> AllocatorLevel<'a> {
        AllocatorLevel {
            bitmap,
            order,
            next_level: next_level.map(|x| x as *const Self as isize - start as isize),
            _phantom: PhantomData,
        }
    }

    pub fn reserve(
        &self,
        start: *const (),
        size_log2: u32,
        index: usize,
    ) -> Result<usize, AllocationError> {
        if size_log2 > 0 {
            if index % 2 != 0 {
                return Err(AllocationError::InvalidReservation {
                    index,
                    size: 1 << size_log2,
                    order: self.order,
                });
            }
            return self
                .next_level(start)
                .ok_or(AllocationError::SpaceExhausted { order: self.order })?
                .reserve(start, size_log2 - 1, index / 2)
                .map(|x| 2 * x);
        }
        if self.bitmap.bit(start, index).clear(start, Ordering::SeqCst) {
            Ok(index)
        } else {
            self.next_level(start)
                .ok_or(AllocationError::RegionInUse {
                    index,
                    order: self.order,
                })?
                .reserve(start, 0, index / 2)?;
            self.free(start, index ^ 1, 0);
            Ok(index)
        }
    }

    pub fn allocate(&self, start: *const (), size_log2: u32) -> Result<usize, AllocationError> {
        if size_log2 > 0 {
            return self
                .next_level(start)
                .ok_or(AllocationError::SpaceExhausted { order: self.order })?
                .allocate(start, size_log2 - 1)
                .map(|x| 2 * x);
        }
        if let Some(index) = self.bitmap.clear_first_set(start) {
            return Ok(index);
        }
        self.next_level(start)
            .and_then(|next| next.allocate(start, 0).ok())
            .map(|i| (2 * i) + 1) // bias allocations towards the upper end of the address space
            .inspect(|i| self.free(start, i - 1, 0))
            .ok_or(AllocationError::SpaceExhausted { order: self.order })
    }

    pub fn free(&self, start: *const (), index: usize, size_log2: u32) {
        if size_log2 > 0 {
            assert_eq!(index % 2, 0);
            return self
                .next_level(start)
                .unwrap()
                .free(start, index / 2, size_log2 - 1);
        }
        if let Some(next) = self.next_level(start) {
            let buddy = index ^ 1;
            if self.bitmap.bit(start, buddy).clear(start, Ordering::SeqCst) {
                next.free(start, index / 2, 0);
                return;
            }
        }
        assert!(!self.bitmap.bit(start, index).set(start, Ordering::SeqCst));
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct BuddyAllocatorInner<'a> {
    _phantom: PhantomData<&'a AllocatorLevel<'a>>,
    level_offset: isize,
    max_align: usize,
    max_level: u32,
    min_level: u32,
    raw_size: usize,
    used_size: AtomicUsize,
    refcnt: AtomicUsize,
}

#[repr(C)]
#[derive(Debug)]
pub struct BuddyAllocator<'a> {
    start: *mut (),
    inner: &'a BuddyAllocatorInner<'a>,
}

unsafe impl Send for BuddyAllocator<'_> {}

impl<'a> BuddyAllocator<'a> {
    #[cfg(feature = "std")]
    pub fn new(base: &'a mut [u8]) -> BuddyAllocator<'a> {
        let raw_size = core::mem::size_of_val(base);
        let min_level = 12;
        let max_level = raw_size.ilog2();
        let start: *mut () = &mut base[0] as *mut u8 as *mut ();
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

            let slice = BitSlice::new(start, bitmap, None);
            let new_level = Box::new(AllocatorLevel::new(start, slice, i, temp_level));
            if i == max_level {
                new_level.free(start, 0, 0);
            }
            temp_level = Some(Box::leak(new_level));
        }

        // create a buddy allocator using the heap-backed version
        let inner = BuddyAllocatorInner {
            _phantom: PhantomData,
            level_offset: temp_level.unwrap() as *const AllocatorLevel<'a> as isize
                - start as isize,
            max_level,
            min_level,
            raw_size,
            max_align,
            used_size: AtomicUsize::new(0),
            refcnt: AtomicUsize::new(1),
        };
        let temp = BuddyAllocator {
            start,
            inner: &inner,
        };

        // allocate the relevant space within the buddy allocator itself
        let size = core::mem::size_of_val(temp_backing);
        let backing = unsafe {
            core::slice::from_raw_parts_mut(
                temp.allocate_raw(size) as *mut AtomicU64,
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

            let slice = BitSlice::new(start, bitmap, None);
            let new_level = AllocatorLevel::new(start, slice, i, level);
            let ptr = temp.allocate_raw(core::mem::size_of_val(&new_level));
            unsafe {
                let ptr = ptr as *mut MaybeUninit<AllocatorLevel<'static>>;
                let ptr = (*ptr).write(new_level) as *mut AllocatorLevel<'static>;
                level = Some(&*ptr);
            }
        }

        let inner = temp.allocate::<BuddyAllocatorInner>(BuddyAllocatorInner {
            _phantom: PhantomData,
            level_offset: level.unwrap() as *const AllocatorLevel<'static> as isize
                - start as isize,
            max_level,
            min_level,
            raw_size,
            max_align,
            used_size: AtomicUsize::new(temp.inner.used_size.load(Ordering::SeqCst)),
            refcnt: AtomicUsize::new(1),
        });
        let real = BuddyAllocator {
            start,
            inner: unsafe { &*inner },
        };

        // transfer the allocation information to the new backing region
        for i in 0..backing.len() {
            backing[i].store(temp_backing[i].load(Ordering::SeqCst), Ordering::SeqCst);
        }

        while let Some(level) = temp_level {
            temp_level = level.next_level(start);
            unsafe {
                core::mem::drop(Box::from_raw(
                    level as *const AllocatorLevel<'static> as *mut AllocatorLevel<'static>,
                ))
            };
        }

        unsafe {
            core::mem::drop(Box::from_raw(
                temp_backing as *const [AtomicU64] as *mut [AtomicU64],
            ));
        }

        real
    }

    fn level(&self) -> &'a AllocatorLevel<'a> {
        unsafe {
            let p = (self.start as isize + self.inner.level_offset) as *const AllocatorLevel<'a>;
            &*p
        }
    }

    pub fn reserve_raw(&self, address: usize, size: usize) -> *mut () {
        let level = size.next_power_of_two().ilog2();
        let level = core::cmp::max(self.inner.min_level, level);
        let size_log2 = level - self.inner.min_level;
        let index = address >> 12;
        self.level()
            .reserve(self.start, size_log2, index)
            .inspect(|_| {
                self.inner.used_size.fetch_add(1 << level, Ordering::SeqCst);
            })
            .map_or(core::ptr::null_mut(), |i| unsafe {
                self.start.byte_add(i * (1 << self.inner.min_level))
            })
    }

    pub fn reserve_uninit<T: Sized>(&self, address: usize) -> *mut MaybeUninit<T> {
        let size = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        assert!(align <= self.inner.max_align);
        let size = core::cmp::max(size, align);
        let raw = self.reserve_raw(address, size);
        raw as *mut MaybeUninit<T>
    }

    pub fn reserve<T: Sized>(&self, address: usize, value: T) -> *mut T {
        let uninit = self.reserve_uninit::<T>(address);
        unsafe {
            (*uninit).write(value);
            (*uninit).assume_init_mut()
        }
    }

    pub fn allocate_raw(&self, size: usize) -> *mut () {
        let level = size.next_power_of_two().ilog2();
        let level = core::cmp::max(self.inner.min_level, level);
        let size_log2 = level - self.inner.min_level;
        self.level()
            .allocate(self.start, size_log2)
            .inspect(|_| {
                self.inner.used_size.fetch_add(1 << level, Ordering::SeqCst);
            })
            .map_or(core::ptr::null_mut(), |i| unsafe {
                self.start.byte_add(i * (1 << self.inner.min_level))
            })
    }

    pub fn allocate_uninit<T: Sized>(&self) -> *mut MaybeUninit<T> {
        let size = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        assert!(align <= self.inner.max_align);
        let size = core::cmp::max(size, align);
        self.allocate_raw(size) as *mut MaybeUninit<T>
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
        let level = core::cmp::max(self.inner.min_level, level);
        let size_log2 = level - self.inner.min_level;
        let index = (ptr as usize - self.start as usize) / (1 << self.inner.min_level) as usize;
        self.level().free(self.start, index, size_log2);
        self.inner.used_size.fetch_sub(1 << level, Ordering::SeqCst);
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

    pub fn base_address(&self) -> *mut () {
        self.start
    }

    pub fn to_offset<T>(&self, allocation: *const T) -> isize {
        let base = self.base_address();
        let current = allocation as *const ();
        current as isize - base as isize
    }

    pub fn from_offset<T>(&self, offset: isize) -> *const T {
        let base = self.base_address();
        (base as isize + offset) as *const T
    }

    pub fn used_size(&self) -> usize {
        self.inner.used_size.load(Ordering::SeqCst)
    }

    pub fn total_size(&self) -> usize {
        1 << self.inner.max_level
    }

    pub fn usage(&self) -> f64 {
        self.used_size() as f64 / self.total_size() as f64
    }

    pub unsafe fn into_raw_parts(self) -> (*mut (), *const BuddyAllocatorInner<'a>) {
        let BuddyAllocator { start, inner } = self;
        core::mem::forget(self);
        (start, inner)
    }

    pub unsafe fn from_raw_parts(start: *mut (), inner: *const BuddyAllocatorInner<'a>) -> Self {
        BuddyAllocator {
            start,
            inner: &*inner,
        }
    }

    pub fn destroy(&mut self) -> Option<&'a mut [u8]> {
        if self.inner.refcnt.fetch_sub(1, Ordering::SeqCst) == 1 {
            unsafe {
                Some(core::slice::from_raw_parts_mut(
                    self.start as *mut u8,
                    self.inner.raw_size,
                ))
            }
        } else {
            None
        }
    }
}

impl<'a> Clone for BuddyAllocator<'a> {
    fn clone(&self) -> Self {
        self.inner.refcnt.fetch_add(1, Ordering::SeqCst);
        Self {
            start: self.start,
            inner: self.inner,
        }
    }
}

impl<'a> Drop for BuddyAllocator<'a> {
    fn drop(&mut self) {
        self.destroy();
    }
}

unsafe impl<'a> Allocator for BuddyAllocator<'a> {
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
