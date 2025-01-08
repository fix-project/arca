use core::{
    alloc::Layout,
    cell::UnsafeCell,
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
struct BitRef<'a> {
    word: &'a mut u64,
    offset: usize,
}

#[allow(unused)]
impl<'a> BitRef<'a> {
    pub fn new(word: &'a mut u64, offset: usize) -> BitRef<'a> {
        assert!(offset < core::mem::size_of::<u64>() * 8);
        BitRef { word, offset }
    }

    pub fn read(&self) -> bool {
        ((*self.word >> self.offset) & 1) == 1
    }

    pub fn set(&mut self) -> bool {
        let old = *self.word;
        *self.word |= 1 << self.offset;
        (old >> self.offset) & 1 == 1
    }

    pub fn clear(&mut self) -> bool {
        let old = *self.word;
        *self.word &= !(1 << self.offset);
        (old >> self.offset) & 1 == 1
    }

    pub fn write(&mut self, value: bool) -> bool {
        if value {
            self.set()
        } else {
            self.clear()
        }
    }
}

#[repr(C)]
#[derive(Debug)]
struct BitSlice<'a> {
    bits: usize,
    data: &'a mut [u64],
}

#[allow(unused)]
impl<'a> BitSlice<'a> {
    pub fn new(bits: usize, data: &'a mut [u64]) -> BitSlice<'a> {
        assert!(bits <= core::mem::size_of_val(data) * 8);
        BitSlice { bits, data }
    }

    pub fn bit(&mut self, index: usize) -> BitRef<'_> {
        assert!(index < self.bits);
        BitRef::new(
            &mut self.data[index / (core::mem::size_of::<u64>() * 8)],
            index % (core::mem::size_of::<u64>() * 8),
        )
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
struct AllocatorLevel<'a> {
    base: *mut (),
    order: u32,
    bits: usize,
    free: &'a mut Option<usize>,
    // true = available; false = unavailable
    bitmap: &'a mut [u64],
}

#[repr(C)]
struct ListNode {
    prev: Option<usize>,
    next: Option<usize>,
}

impl<'a> AllocatorLevel<'a> {
    pub fn new(
        base: *mut (),
        order: u32,
        bits: usize,
        free: &'a mut Option<usize>,
        bitmap: &'a mut [u64],
    ) -> Self {
        Self {
            base,
            order,
            bits,
            free,
            bitmap,
        }
    }

    fn index_to_ptr(&self, index: usize) -> *mut () {
        self.base.wrapping_byte_add(index * (1 << self.order))
    }

    unsafe fn node(&self, index: usize) -> *mut ListNode {
        self.index_to_ptr(index) as *mut ListNode
    }

    fn bits(&mut self) -> BitSlice<'_> {
        BitSlice::new(self.bits, self.bitmap)
    }

    pub fn reserve(&mut self, index: usize) -> bool {
        if !self.bits().bit(index).clear() {
            return false;
        }
        unsafe {
            let node = &mut *self.node(index);
            if let Some(prev) = node.prev {
                let prev = &mut *self.node(prev);
                prev.next = node.next;
            } else {
                assert_eq!(*self.free, Some(index));
                *self.free = node.next;
            }
            if let Some(next) = node.next {
                let next = &mut *self.node(next);
                next.prev = node.prev;
            }
        };
        true
    }

    pub fn allocate(&mut self) -> Option<usize> {
        if let Some(index) = self.free {
            let index = *index;
            self.reserve(index);
            Some(index)
        } else {
            None
        }
    }

    pub fn free(&mut self, index: usize) {
        assert!(
            !self.bits().bit(index).set(),
            "allocation was already free!"
        );
        unsafe {
            let node = &mut *self.node(index);
            node.next = *self.free;
            node.prev = None;
            if let Some(next) = node.next {
                let next = &mut *self.node(next);
                next.prev = Some(index);
            }
            *self.free = Some(index);
        };
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
struct AllocatorInner {
    meta: AllocatorMetadata,
    lock: AtomicU64,
    free: UnsafeCell<[Option<usize>; 64]>,
    data: UnsafeCell<[u64]>,
}

impl AllocatorInner {
    pub fn new(slice: &mut [u8]) -> Box<AllocatorInner> {
        Self::new_in(slice, Global)
    }

    pub fn new_in<A: Allocator>(slice: &mut [u8], allocator: A) -> Box<AllocatorInner, A> {
        let raw_size = core::mem::size_of_val(slice);
        let min_level = 12;
        let max_level = raw_size.ilog2();
        assert!(max_level < 64);
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
                .extend(Layout::new::<AtomicU64>())
                .unwrap()
                .0
                .extend(Layout::new::<[Option<usize>; 64]>())
                .unwrap()
                .0
                .extend(Layout::new::<AtomicU64>().repeat(space).unwrap().0)
                .unwrap()
                .0
                .pad_to_align();
            let p = allocator
                .allocate_zeroed(layout)
                .expect("could not allocate space for allocator bitmap");
            let p: *mut AllocatorInner = core::mem::transmute(p);
            (*p).free.get().write([None; 64]);
            (&raw mut (*p).meta).write(AllocatorMetadata {
                refcnt: AtomicUsize::new(1),
                used_size: AtomicUsize::new(0),
                raw_size,
                total_size,
                max_align,
                level_range,
            });
            let mut top = AllocatorLevel::new(
                slice.as_ptr() as *mut _,
                max_level,
                1,
                &mut (*(*p).free.get())[max_level as usize],
                &mut (*(*p).data.get())[0..1],
            );
            top.free(0);
            Box::from_raw_in(p, allocator)
        }
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

    // deadlock safety: always lock levels in increasing order
    pub fn with_level<T>(
        &self,
        base: *mut (),
        level: u32,
        f: impl FnOnce(&mut AllocatorLevel<'_>) -> T,
    ) -> T {
        loop {
            let original = self.lock.load(Ordering::SeqCst);
            let cleared = original & !(1 << level);
            let set = original | (1 << level);
            if self
                .lock
                .compare_exchange(cleared, set, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
        let start = self.offset_of_level_words(level);
        let size = self.size_of_level_words(level);
        let end = start + size;
        let (slice, free) = unsafe {
            // safe since we have the lock
            let start = &raw mut (*self.data.get())[start];
            let end = &raw mut (*self.data.get())[end];
            let slice = core::slice::from_mut_ptr_range(Range { start, end }.into());
            let free = &mut (*self.free.get())[level as usize];
            (slice, free)
        };
        let mut allocator_level =
            AllocatorLevel::new(base, level, self.size_of_level_bits(level), free, slice);
        let result = f(&mut allocator_level);
        loop {
            let original = self.lock.load(Ordering::SeqCst);
            let cleared = original & !(1 << level);
            if self
                .lock
                .compare_exchange(original, cleared, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
        result
    }

    pub fn reserve(
        &self,
        base: *mut (),
        index: usize,
        size_log2: u32,
    ) -> Result<usize, AllocationError> {
        assert_ne!(self.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(size_log2, self.meta.level_range.start);
        if size_log2 >= self.meta.level_range.end {
            return Err(AllocationError::InvalidReservation {
                index,
                size: 1 << size_log2,
            });
        }
        self.with_level(base, size_log2, |level: &mut AllocatorLevel<'_>| {
            if level.reserve(index) {
                Ok(index)
            } else {
                match self.reserve(base, index / 2, size_log2 + 1) {
                    Ok(_) => {
                        self.free(base, index ^ 1, size_log2, Some(level));
                        Ok(index)
                    }
                    Err(_) => Err(AllocationError::RegionInUse {
                        index,
                        size: 1 << size_log2,
                    }),
                }
            }
        })
    }

    pub fn allocate(&self, base: *mut (), size_log2: u32) -> Result<usize, AllocationError> {
        assert_ne!(self.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(size_log2, self.meta.level_range.start);
        if size_log2 >= self.meta.level_range.end {
            return Err(AllocationError::SpaceExhausted {
                size: 1 << size_log2,
            });
        }
        // let level = self.level(size_log2);
        self.with_level(base, size_log2, |level: &mut AllocatorLevel<'_>| {
            if let Some(index) = level.allocate() {
                Ok(index)
            } else {
                match self.allocate(base, size_log2 + 1) {
                    Ok(index) => {
                        let index = 2 * index;
                        // bias allocations towards the upper end of the address space
                        self.free(base, index, size_log2, Some(level));
                        Ok(index + 1)
                    }
                    Err(_) => Err(AllocationError::SpaceExhausted {
                        size: 1 << size_log2,
                    }),
                }
            }
        })
    }

    pub fn free(
        &self,
        base: *mut (),
        index: usize,
        size_log2: u32,
        level: Option<&mut AllocatorLevel<'_>>,
    ) {
        assert_ne!(self.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(size_log2, self.meta.level_range.start);
        assert!(size_log2 < self.meta.level_range.end);
        let body = |level: &mut AllocatorLevel<'_>| {
            if size_log2 == self.meta.level_range.end - 1 {
                level.free(index);
                return;
            }
            let buddy = index ^ 1;
            if level.reserve(buddy) {
                self.free(base, index / 2, size_log2 + 1, None);
            } else {
                level.free(index);
            }
        };
        match level {
            Some(level) => body(level),
            None => self.with_level(base, size_log2, body),
        }
    }
}

#[repr(C)]
pub struct BuddyAllocator<'a> {
    base: *mut (),
    inner: &'a AllocatorInner,
    refcnt: &'a [AtomicUsize],
}

#[repr(C)]
pub struct BuddyAllocatorRawData {
    pub base: *mut (),
    pub inner_offset: usize,
    pub inner_size: usize,
    pub refcnt_offset: usize,
    pub refcnt_size: usize,
}

unsafe impl Send for BuddyAllocator<'_> {}
unsafe impl Sync for BuddyAllocator<'_> {}

impl<'a> BuddyAllocator<'a> {
    pub const MIN_ALLOCATION: usize = 1 << 12;

    pub fn new(slice: &'a mut [u8]) -> BuddyAllocator<'a> {
        let base = &raw mut slice[0] as *mut ();
        // allocate on the normal heap
        let inner = AllocatorInner::new(slice);

        let refcnt_size = slice.len() / Self::MIN_ALLOCATION;

        let refcnt = unsafe {
            let data = Box::new_zeroed_slice(refcnt_size);
            data.assume_init()
        };

        let temp = BuddyAllocator {
            base,
            inner: &inner,
            refcnt: &refcnt,
        };

        let new_inner = AllocatorInner::new_in(slice, &temp);

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
        let new_refcnt = unsafe {
            let data = Box::new_zeroed_slice_in(refcnt_size, &temp);
            data.assume_init()
        };
        let refcnt = unsafe {
            let new_refcnt = Box::into_raw(new_refcnt);
            let src = &raw const *refcnt;
            let dst = &raw mut *new_refcnt;
            let (src, src_size) = src.to_raw_parts();
            let (dst, dst_size) = dst.to_raw_parts();
            assert_eq!(src_size, dst_size);
            core::ptr::copy_nonoverlapping(src as *mut u8, dst as *mut u8, src_size);
            &*new_refcnt
        };

        BuddyAllocator {
            base,
            inner,
            refcnt,
        }
    }

    pub fn reserve_raw(&self, address: usize, size: usize) -> *mut () {
        let level = size.next_power_of_two().ilog2();
        assert_ne!(self.inner.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(self.inner.meta.level_range.start, level);
        let index = address >> size_log2;
        self.inner
            .reserve(self.base, index, size_log2)
            .inspect(|_| {
                self.inner
                    .meta
                    .used_size
                    .fetch_add(1 << size_log2, Ordering::SeqCst);
            })
            .map_or(core::ptr::null_mut(), |i| {
                self.base.wrapping_byte_add(i * (1 << size_log2))
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
            .allocate(self.base, size_log2)
            .inspect(|_| {
                self.inner
                    .meta
                    .used_size
                    .fetch_add(1 << size_log2, Ordering::SeqCst);
            })
            .map_or(core::ptr::null_mut(), |i| {
                self.base.wrapping_byte_add(i * (1 << size_log2))
            })
    }

    pub fn free_raw(&self, ptr: *mut (), size: usize) {
        let level = size.next_power_of_two().ilog2();
        assert_ne!(self.inner.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(self.inner.meta.level_range.start, level);
        let index = (ptr as usize - self.base as usize) / (1 << size_log2) as usize;
        self.inner.free(self.base, index, size_log2, None);
        self.inner
            .meta
            .used_size
            .fetch_sub(1 << size_log2, Ordering::SeqCst);
    }

    pub fn to_offset<T: ?Sized>(&self, allocation: *const T) -> usize {
        let base = self.base;
        let current = allocation as *const ();
        current as usize - base as usize
    }

    pub fn from_offset<T>(&self, offset: usize) -> *const T {
        let base = self.base;
        (base as usize + offset) as *const T
    }

    pub fn refcnt<T: ?Sized>(&self, allocation: *const T) -> *const AtomicUsize {
        let offset = self.to_offset(allocation) / Self::MIN_ALLOCATION;
        &raw const self.refcnt[offset]
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

    pub fn into_raw_parts(self) -> BuddyAllocatorRawData {
        let BuddyAllocator {
            base,
            inner,
            refcnt,
        } = self;
        let p = inner as *const AllocatorInner;
        let q = refcnt as *const [AtomicUsize];
        let (metadata, inner_size) = p.to_raw_parts();
        let (refcnt, refcnt_size) = q.to_raw_parts();

        core::mem::forget(self);
        let inner_offset = metadata as usize - base as usize;
        let refcnt_offset = refcnt as usize - base as usize;
        BuddyAllocatorRawData {
            base,
            inner_offset,
            inner_size,
            refcnt_offset,
            refcnt_size,
        }
    }

    /// # Safety
    ///
    /// The raw data passed to this function must be valid parameters for an allocator (i.e., must
    /// have come from [into_raw_parts]).
    pub unsafe fn from_raw_parts(raw: BuddyAllocatorRawData) -> Self {
        let BuddyAllocatorRawData {
            base,
            inner_offset,
            inner_size,
            refcnt_offset,
            refcnt_size,
        } = raw;
        let inner = core::ptr::from_raw_parts_mut(base.byte_add(inner_offset), inner_size);
        let refcnt = core::ptr::from_raw_parts_mut(base.byte_add(refcnt_offset), refcnt_size);
        BuddyAllocator {
            base,
            inner: &*inner,
            refcnt: &*refcnt,
        }
    }

    pub fn destroy(&mut self) -> Option<&'a mut [u8]> {
        if self.inner.meta.refcnt.fetch_sub(1, Ordering::SeqCst) == 1 {
            unsafe {
                Some(core::slice::from_raw_parts_mut(
                    self.base as *mut u8,
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
            base: self.base,
            inner: self.inner,
            refcnt: self.refcnt,
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
        let mut word = 10;

        let mut r0 = BitRef::new(&mut word, 0);
        r0.set();

        let mut r1 = BitRef::new(&mut word, 1);
        r1.clear();

        let mut r2 = BitRef::new(&mut word, 2);
        r2.write(false);

        let mut r3 = BitRef::new(&mut word, 3);
        r3.write(true);

        assert_eq!(word, 9);
    }

    #[test]
    fn test_bitslice() {
        let mut words = [0; 2];
        let mut slice = BitSlice::new(128, &mut words);
        let mut r0 = slice.bit(0);
        r0.set();

        let mut r1 = slice.bit(1);
        r1.set();

        let mut r127 = slice.bit(127);
        r127.set();

        assert_eq!(words[0], 3);
        assert_eq!(
            words[127 / (core::mem::size_of::<u64>() * 8)],
            1 << (127 % (core::mem::size_of::<u64>() * 8))
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
                Box::new_uninit_slice_in(128, &allocator);
            core::mem::drop(x);
        });
    }
}
