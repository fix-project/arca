use core::{
    alloc::Layout,
    cell::UnsafeCell,
    marker::PhantomPinned,
    mem::MaybeUninit,
    ops::Deref,
    pin::Pin,
    ptr::NonNull,
    range::Range,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};
extern crate alloc;
use alloc::alloc::{AllocError, Allocator};

#[cfg(feature = "std")]
use alloc::alloc::Global;

use snafu::prelude::*;

use crate::util::initcell::LazyLock;

static BUDDY: LazyLock<BuddyAllocatorImpl> = LazyLock::new(|| {
    #[cfg(feature = "std")]
    {
        BuddyAllocatorImpl::new(1 << 32)
    }
    #[cfg(not(feature = "std"))]
    {
        todo!()
    }
});

#[cfg(feature = "std")]
pub fn init(size: usize) {
    LazyLock::set(&BUDDY, BuddyAllocatorImpl::new(size))
        .expect("buddy allocator was already initialized");
}

pub fn export() -> BuddyAllocatorRawData {
    BUDDY.clone().into_raw_parts()
}

/// # Safety
/// The argument to this function must have come from a call to export.
pub unsafe fn import(raw: BuddyAllocatorRawData) {
    LazyLock::set(&BUDDY, BuddyAllocatorImpl::from_raw_parts(raw))
        .expect("buddy allocator was already initialized");
}

pub fn wait() {
    LazyLock::wait(&BUDDY);
}

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

    pub fn data(&self) -> &[u64] {
        self.data
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
#[derive(Default)]
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
                assert_eq!(prev.next, Some(index));
                prev.next = node.next;
            } else {
                assert_eq!(*self.free, Some(index));
                *self.free = node.next;
            }
            if let Some(next) = node.next {
                let next = &mut *self.node(next);
                assert_eq!(next.prev, Some(index));
                next.prev = node.prev;
            }
            *node = Default::default();
        };
        true
    }

    pub fn allocate(&mut self) -> Option<usize> {
        if let Some(index) = self.free {
            let index = *index;
            assert!(self.reserve(index));
            Some(index)
        } else {
            None
        }
    }

    pub fn free(&mut self, index: usize) {
        unsafe {
            let node = &mut *self.node(index);
            node.next = None;
            node.prev = None;
            if let Some(head) = *self.free {
                let next = &mut *self.node(head);
                assert!(next.prev.is_none());
                next.prev = Some(index);
                node.next = Some(head);
            }
            *self.free = Some(index);
        };
        assert!(
            !self.bits().bit(index).set(),
            "allocation {index} at level {} was already free!",
            self.order
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
    request_count: [AtomicUsize; 64],
}

#[repr(C)]
#[derive(Debug)]
struct AllocatorInner {
    meta: AllocatorMetadata,
    upin: PhantomPinned,
    lock: AtomicU64,
    free: UnsafeCell<[Option<usize>; 64]>,
    data: UnsafeCell<[u64]>,
}

impl AllocatorInner {
    #[cfg(feature = "std")]
    pub fn new(slice: &mut [u8]) -> Pin<Box<AllocatorInner>> {
        Self::new_in(slice, Global)
    }

    #[cfg(feature = "std")]
    pub fn new_in<A: Allocator>(slice: &mut [u8], allocator: A) -> Pin<Box<AllocatorInner, A>> {
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
                .extend(Layout::new::<PhantomPinned>())
                .unwrap()
                .0
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
            let p: *mut AllocatorInner =
                core::ptr::from_raw_parts_mut(&raw mut (*p.as_ptr())[0], space);
            (*p).free.get().write([None; 64]);
            (&raw mut (*p).meta).write(AllocatorMetadata {
                refcnt: AtomicUsize::new(1),
                used_size: AtomicUsize::new(0),
                raw_size,
                total_size,
                max_align,
                level_range,
                request_count: [const { AtomicUsize::new(0) }; 64],
            });
            let mut top = AllocatorLevel::new(
                slice.as_ptr() as *mut _,
                max_level,
                1,
                &mut (*(*p).free.get())[max_level as usize],
                &mut (&mut (*(*p).data.get()))[0..1],
            );
            top.free(0);
            Pin::new_unchecked(Box::from_raw_in(p, allocator))
        }
    }

    pub fn size_of_level_bits(&self, level: u32) -> usize {
        assert!(self.meta.level_range.contains(&level));
        let inverse = self.meta.level_range.end - level - 1;
        1 << inverse
    }

    pub fn size_of_level_words(&self, level: u32) -> usize {
        self.size_of_level_bits(level)
            .div_ceil(core::mem::size_of::<AtomicU64>() * 8)
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
    #[inline(never)]
    pub fn with_level<T>(
        self: Pin<&Self>,
        base: *mut (),
        level: u32,
        f: impl FnOnce(&mut AllocatorLevel<'_>) -> T,
    ) -> T {
        let start = self.offset_of_level_words(level);
        let size = self.size_of_level_words(level);
        let (slice, free) = unsafe {
            // safe since we have the lock
            let start = &raw mut (*self.data.get())[start];
            let slice = core::slice::from_raw_parts_mut(start, size);
            let free = &mut (*self.free.get())[level as usize];
            (slice, free)
        };
        let mut allocator_level =
            AllocatorLevel::new(base, level, self.size_of_level_bits(level), free, slice);
        f(&mut allocator_level)
    }

    unsafe fn try_lock(self: Pin<&Self>) -> bool {
        self.lock
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    unsafe fn lock(self: Pin<&Self>) {
        while !self.try_lock() {
            core::hint::spin_loop();
        }
    }

    unsafe fn unlock(self: Pin<&Self>) {
        self.lock.store(0, Ordering::SeqCst);
    }

    pub fn reserve(
        self: Pin<&Self>,
        base: *mut (),
        index: usize,
        size_log2: u32,
    ) -> Result<usize, AllocationError> {
        unsafe {
            self.lock();
            let result = self.reserve_unchecked(base, index, size_log2);
            self.unlock();
            result
        }
    }

    unsafe fn reserve_unchecked(
        self: Pin<&Self>,
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
                match self.reserve_unchecked(base, index / 2, size_log2 + 1) {
                    Ok(_) => {
                        self.free_unchecked(base, index ^ 1, size_log2, Some(level));
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

    unsafe fn allocate_unchecked(
        self: Pin<&Self>,
        base: *mut (),
        size_log2: u32,
    ) -> Result<usize, AllocationError> {
        assert_ne!(self.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(size_log2, self.meta.level_range.start);
        if size_log2 >= self.meta.level_range.end {
            return Err(AllocationError::SpaceExhausted {
                size: 1 << size_log2,
            });
        }
        self.with_level(base, size_log2, |level: &mut AllocatorLevel<'_>| {
            if let Some(index) = level.allocate() {
                Ok(index)
            } else {
                match self.allocate_unchecked(base, size_log2 + 1) {
                    Ok(index) => {
                        let index = 2 * index;
                        // bias allocations towards the upper end of the address space
                        self.free_unchecked(base, index, size_log2, Some(level));
                        Ok(index + 1)
                    }
                    Err(_) => Err(AllocationError::SpaceExhausted {
                        size: 1 << size_log2,
                    }),
                }
            }
        })
    }

    unsafe fn free_unchecked(
        self: Pin<&Self>,
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
                self.free_unchecked(base, index / 2, size_log2 + 1, None);
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
pub struct BuddyAllocatorImpl {
    base: *mut (),
    size: usize,
    caching: bool,
    inner: Pin<&'static AllocatorInner>,
    refcnt: Pin<&'static [AtomicUsize]>,
}

impl core::fmt::Debug for BuddyAllocatorImpl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "BuddyAllocatorImpl @ {:p}", self.inner)
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct BuddyAllocatorRawData {
    pub base: *mut (),
    pub size: usize,
    pub inner_offset: usize,
    pub inner_size: usize,
    pub refcnt_offset: usize,
    pub refcnt_size: usize,
}

unsafe impl Send for BuddyAllocatorImpl {}
unsafe impl Sync for BuddyAllocatorImpl {}

impl BuddyAllocatorImpl {
    pub const MIN_ALLOCATION: usize = 1 << 12;

    #[cfg(feature = "std")]
    pub fn new(size: usize) -> BuddyAllocatorImpl {
        let mmap = crate::mmap::Mmap::new(size);
        // allocate on the normal heap

        let ptr = mmap.into_raw();
        let slice = unsafe { &mut *ptr };

        let inner = AllocatorInner::new(slice);
        let (base, size) = ptr.to_raw_parts();

        let refcnt_size = slice.len() / Self::MIN_ALLOCATION;

        let refcnt = unsafe {
            let data: Box<[MaybeUninit<AtomicUsize>]> = Box::new_zeroed_slice(refcnt_size);
            data.assume_init()
        };
        let refcnt = Box::into_pin(refcnt);

        #[allow(clippy::missing_transmute_annotations)]
        let temp = BuddyAllocatorImpl {
            base,
            size,
            caching: false,
            inner: unsafe { core::mem::transmute(inner.as_ref()) },
            refcnt: unsafe { core::mem::transmute(refcnt.as_ref()) },
        };

        // prevent physical zero page from being allocated
        assert_eq!(temp.to_offset(temp.reserve_raw(0, 4096)), 0);
        // reserve kernel pages
        let mut pages = alloc::vec![];
        for i in 0..8 {
            let p = temp.reserve_raw(0x100000 * (i + 1), 0x100000);
            assert!(!p.is_null());
            pages.push(p);
        }

        let new_inner = AllocatorInner::new_in(slice, &temp);
        let new_refcnt = unsafe {
            let data = Box::new_zeroed_slice_in(refcnt_size, &temp);
            data.assume_init()
        };

        let new_inner = unsafe {
            let new_inner = Box::leak(Pin::into_inner_unchecked(new_inner));
            let src = &raw const *inner;
            let dst = &raw mut *new_inner;
            core::ptr::copy_nonoverlapping(
                src as *mut u8,
                dst as *mut u8,
                core::mem::size_of_val_raw(src),
            );
            &*(new_inner as *mut AllocatorInner)
        };
        let new_refcnt = unsafe {
            let new_refcnt = Box::leak(new_refcnt);
            let src = &raw const *refcnt;
            let dst = &raw mut *new_refcnt;
            let (src, src_size) = src.to_raw_parts();
            let (dst, dst_size) = dst.to_raw_parts();
            assert_eq!(src_size, dst_size);
            core::ptr::copy_nonoverlapping(
                src as *mut u8,
                dst as *mut u8,
                core::mem::size_of_val_raw(src),
            );
            &*(new_refcnt as *mut [AtomicUsize])
        };

        let allocator = BuddyAllocatorImpl {
            base,
            size: slice.len(),
            caching: cfg!(feature = "cache"),
            inner: Pin::static_ref(new_inner),
            refcnt: Pin::static_ref(new_refcnt),
        };
        for page in pages {
            allocator.free_raw(page, 0x100000);
        }
        core::mem::forget(temp);
        let _ = inner;
        let _ = refcnt;
        allocator
    }

    pub fn set_caching(&mut self, enable: bool) {
        if cfg!(feature = "cache") {
            self.caching = enable;
        }
    }

    pub fn reserve_raw(&self, address: usize, size: usize) -> *mut () {
        let level = size.next_power_of_two().ilog2();
        assert_ne!(self.inner.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(self.inner.meta.level_range.start, level);
        let index = address >> size_log2;
        self.inner.meta.request_count[size_log2 as usize].fetch_add(1, Ordering::SeqCst);
        let ptr = self
            .inner
            .reserve(self.base, index, size_log2)
            .inspect(|x| log::debug!("reserved {x:#x}@{size_log2}"))
            .inspect(|_| {
                self.inner
                    .meta
                    .used_size
                    .fetch_add(1 << size_log2, Ordering::SeqCst);
            })
            .map_or(core::ptr::null_mut(), |i| {
                self.base.wrapping_byte_add(i * (1 << size_log2))
            });
        if !ptr.is_null() {
            unsafe {
                (*self.refcnt(ptr)).store(0, Ordering::SeqCst);
            }
        }
        ptr
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
        unsafe {
            self.inner.lock();
            let result = self.allocate_raw_unchecked(size);
            self.inner.unlock();
            result
        }
    }

    pub fn try_allocate_many_raw(&self, size: usize, ptrs: &mut [*mut ()]) -> Option<usize> {
        unsafe {
            if !self.inner.try_lock() {
                return None;
            }
            for (i, item) in ptrs.iter_mut().enumerate() {
                let result = self.allocate_raw_unchecked(size);
                if result.is_null() {
                    return Some(i);
                }
                *item = result;
            }
            self.inner.unlock();
            Some(ptrs.len())
        }
    }

    pub fn allocate_many_raw(&self, size: usize, ptrs: &mut [*mut ()]) -> usize {
        unsafe {
            self.inner.lock();
            for (i, item) in ptrs.iter_mut().enumerate() {
                let result = self.allocate_raw_unchecked(size);
                if result.is_null() {
                    return i;
                }
                *item = result;
            }
            self.inner.unlock();
            ptrs.len()
        }
    }

    /// # Safety
    ///
    /// This allocator must be locked by the current thread.
    pub unsafe fn allocate_raw_unchecked(&self, size: usize) -> *mut () {
        let level = size.next_power_of_two().ilog2();
        assert_ne!(self.inner.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(self.inner.meta.level_range.start, level);
        self.inner.meta.request_count[size_log2 as usize].fetch_add(1, Ordering::SeqCst);
        let ptr = self
            .inner
            .allocate_unchecked(self.base, size_log2)
            .inspect(|x| log::debug!("allocated {x:#x}@{size_log2}"))
            .inspect(|_| {
                self.inner
                    .meta
                    .used_size
                    .fetch_add(1 << size_log2, Ordering::SeqCst);
            })
            .map_or(core::ptr::null_mut(), |i| {
                self.base.wrapping_byte_add(i * (1 << size_log2))
            });
        if !ptr.is_null() {
            unsafe {
                (*self.refcnt(ptr)).store(0, Ordering::SeqCst);
            }
        }
        ptr
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn free_raw(&self, ptr: *mut (), size: usize) {
        unsafe {
            self.inner.lock();
            self.free_raw_unchecked(ptr, size);
            self.inner.unlock();
        }
    }

    pub fn free_many_raw(&self, size: usize, ptrs: &[*mut ()]) {
        unsafe {
            self.inner.lock();
            for item in ptrs.iter() {
                self.free_raw_unchecked(*item, size);
            }
            self.inner.unlock();
        }
    }

    pub fn try_free_many_raw(&self, size: usize, ptrs: &[*mut ()]) -> Option<()> {
        unsafe {
            if !self.inner.try_lock() {
                return None;
            }
            for item in ptrs.iter() {
                self.free_raw_unchecked(*item, size);
            }
            self.inner.unlock();
        }
        Some(())
    }

    /// # Safety
    ///
    /// This allocator must be locked by the current thread.
    pub unsafe fn free_raw_unchecked(&self, ptr: *mut (), size: usize) {
        if ptr.is_null() {
            return;
        }
        assert_eq!((*self.refcnt(ptr)).load(Ordering::SeqCst), 0);
        let level = size.next_power_of_two().ilog2();
        assert_ne!(self.inner.meta.level_range.start, 0);
        let size_log2 = core::cmp::max(self.inner.meta.level_range.start, level);
        self.inner.meta.request_count[size_log2 as usize].fetch_add(1, Ordering::SeqCst);
        let index = (ptr as usize - self.base as usize) / (1 << size_log2) as usize;
        log::debug!("freed {index:#x}@{size_log2}");
        self.inner.free_unchecked(self.base, index, size_log2, None);
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

    pub fn from_offset<T>(&self, offset: usize) -> *mut T {
        assert!(offset & 0xFFFF800000000000 == 0);
        let base = self.base;
        debug_assert!(base as usize & 0xFFF == 0);
        (base as usize + offset) as *mut T
    }

    pub fn refcnt<T: ?Sized>(&self, allocation: *const T) -> *const AtomicUsize {
        if allocation.is_null() {
            return core::ptr::null();
        }
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

    pub fn requests(&self, results: &mut [usize; 64]) {
        for (y, x) in results.iter_mut().zip(self.inner.meta.request_count.iter()) {
            *y = x.load(Ordering::SeqCst);
        }
    }

    pub fn into_raw_parts(self) -> BuddyAllocatorRawData {
        let BuddyAllocatorImpl {
            base,
            size,
            caching: _caching,
            inner,
            refcnt,
        } = self;
        let p = inner.get_ref() as *const AllocatorInner;
        let q = refcnt.get_ref() as *const [AtomicUsize];
        let (metadata, inner_size) = p.to_raw_parts();
        let (refcnt, refcnt_size) = q.to_raw_parts();

        core::mem::forget(self);
        let inner_offset = metadata as usize - base as usize;
        let refcnt_offset = refcnt as usize - base as usize;
        BuddyAllocatorRawData {
            base,
            size,
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
            size,
            inner_offset,
            inner_size,
            refcnt_offset,
            refcnt_size,
        } = raw;
        let inner: *const AllocatorInner =
            core::ptr::from_raw_parts_mut(base.byte_add(inner_offset), inner_size);
        let refcnt = core::ptr::from_raw_parts_mut(base.byte_add(refcnt_offset), refcnt_size);
        BuddyAllocatorImpl {
            base,
            size,
            caching: cfg!(feature = "cache"),
            inner: Pin::static_ref(&*inner),
            refcnt: Pin::static_ref(&*refcnt),
        }
    }

    pub fn base(&self) -> *mut () {
        self.base
    }

    pub fn len(&self) -> usize {
        self.size
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Clone for BuddyAllocatorImpl {
    fn clone(&self) -> Self {
        self.inner.meta.refcnt.fetch_add(1, Ordering::SeqCst);
        Self {
            base: self.base,
            size: self.size,
            caching: self.caching,
            inner: self.inner,
            refcnt: self.refcnt,
        }
    }
}

impl Drop for BuddyAllocatorImpl {
    fn drop(&mut self) {
        let copies = self.inner.meta.refcnt.fetch_sub(1, Ordering::SeqCst);
        if copies == 1 {
            #[cfg(feature = "std")]
            unsafe {
                let ptr = core::ptr::from_raw_parts_mut(self.base, self.size);
                let mmap = crate::mmap::Mmap::from_raw(ptr);
                core::mem::forget(mmap);
                // core::mem::drop(mmap);
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BuddyAllocator;

impl BuddyAllocator {
    pub const MIN_ALLOCATION: usize = BuddyAllocatorImpl::MIN_ALLOCATION;
}

impl Deref for BuddyAllocator {
    type Target = BuddyAllocatorImpl;

    fn deref(&self) -> &Self::Target {
        &BUDDY
    }
}

// TODO: this is not safe if there are multiple allocators
#[cfg(all(not(feature = "std"), feature = "core_local_cache"))]
pub mod cache {
    use crate::{arrayvec::ArrayVec, util::initcell::LazyLock, util::spinlock::SpinLock};
    use macros::core_local;

    pub static CAPACITY: usize = 16384;
    pub static INCREMENT: usize = 512;
    pub static WATERMARK_LOW: usize = CAPACITY / 4;
    pub static WATERMARK_HIGH: usize = 3 * CAPACITY / 4;

    #[core_local]
    pub static ALLOCATION_CACHE: LazyLock<SpinLock<ArrayVec<*mut (), CAPACITY>>> =
        LazyLock::new(|| SpinLock::new(ArrayVec::new()));
}

#[cfg(all(feature = "std", feature = "thread_local_cache"))]
pub mod cache {
    use std::sync::LazyLock;

    use crate::{arrayvec::ArrayVec, util::spinlock::SpinLock};

    pub static CAPACITY: usize = 1024;
    pub static INCREMENT: usize = 64;
    pub static WATERMARK_LOW: usize = CAPACITY / 4;
    pub static WATERMARK_HIGH: usize = 3 * CAPACITY / 4;

    #[thread_local]
    pub static ALLOCATION_CACHE: LazyLock<SpinLock<ArrayVec<*mut (), CAPACITY>>> =
        LazyLock::new(|| SpinLock::new(ArrayVec::new()));
}

#[cfg(feature = "cache")]
impl BuddyAllocatorImpl {
    fn allocate_from_cache(&self, size: usize) -> Result<NonNull<[u8]>, AllocError> {
        let mut cache = cache::ALLOCATION_CACHE.lock();
        if cache.is_empty() {
            let mut space = [core::ptr::null_mut(); cache::INCREMENT];
            let count = self.allocate_many_raw(size, &mut space);
            for entry in space.iter().take(count) {
                cache.push(*entry).unwrap();
            }
        }
        let raw = cache.pop().ok_or(AllocError)?;
        let converted = core::ptr::slice_from_raw_parts_mut(raw as *mut u8, size);
        NonNull::new(converted).ok_or(AllocError)
    }

    fn free_to_cache(&self, ptr: *mut (), size: usize) {
        let mut cache = cache::ALLOCATION_CACHE.lock();
        if cache.len() == cache.capacity() {
            let mut space = [core::ptr::null_mut(); cache::INCREMENT];
            for spot in space.iter_mut() {
                *spot = cache.pop().unwrap();
            }
            self.free_many_raw(size, &space);
        }
        cache.push(ptr).unwrap();
    }

    pub fn try_replenish(&self) -> bool {
        let Some(mut cache) = cache::ALLOCATION_CACHE.try_lock() else {
            return false;
        };
        let size = 4096;
        if cache.len() >= cache::WATERMARK_HIGH {
            let mut space: [*mut (); cache::INCREMENT] = [core::ptr::null_mut(); cache::INCREMENT];
            for spot in space.iter_mut() {
                *spot = cache.pop().unwrap();
            }
            if self.try_free_many_raw(size, &space).is_none() {
                return false;
            }
            return true;
        }
        if cache.len() < cache::WATERMARK_HIGH {
            let mut space = [core::ptr::null_mut(); cache::INCREMENT];
            let Some(count) = self.try_allocate_many_raw(size, &mut space) else {
                return false;
            };
            for entry in space.iter().take(count) {
                cache.push(*entry).unwrap();
            }
            return true;
        }
        true
    }
}

unsafe impl Allocator for BuddyAllocatorImpl {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let size = layout.size();
        let align = layout.align();
        let size = core::cmp::max(size, align);

        #[cfg(feature = "cache")]
        {
            if self.caching && size <= 4096 && align <= 4096 {
                return self.allocate_from_cache(size);
            }
        }

        let raw = self.allocate_raw(size);
        let converted = core::ptr::slice_from_raw_parts_mut(raw as *mut u8, size);
        NonNull::new(converted).ok_or(AllocError)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let size = layout.size();
        let align = layout.align();
        let size = core::cmp::max(size, align);

        #[cfg(feature = "cache")]
        {
            if self.caching && size <= 4096 && align <= 4096 {
                self.free_to_cache(ptr.as_ptr() as *mut (), size);
                return;
            }
        }

        self.free_raw(ptr.as_ptr() as *mut (), size)
    }
}

unsafe impl Allocator for BuddyAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        BUDDY.allocate(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        BUDDY.deallocate(ptr, layout)
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
        let allocator = BuddyAllocatorImpl::new(0x10000000);

        let test = Box::new_in(10, allocator.clone());
        assert_eq!(*test, 10);

        let mut v = Vec::new_in(allocator.clone());
        for i in 0..10000 {
            v.push(i);
        }
    }

    #[bench]
    fn bench_allocate_free(b: &mut Bencher) {
        let allocator = BuddyAllocatorImpl::new(0x100000000);
        b.iter(|| {
            let x: Box<[MaybeUninit<u8>], BuddyAllocatorImpl> =
                Box::new_uninit_slice_in(128, allocator.clone());
            core::mem::drop(x);
        });
    }

    #[bench]
    fn bench_allocate_free_no_cache(b: &mut Bencher) {
        let mut allocator = BuddyAllocatorImpl::new(0x100000000);
        allocator.set_caching(false);
        b.iter(|| {
            let x: Box<[MaybeUninit<u8>], BuddyAllocatorImpl> =
                Box::new_uninit_slice_in(128, allocator.clone());
            core::mem::drop(x);
        });
    }

    #[bench]
    fn bench_contended_allocate_free(b: &mut Bencher) {
        let allocator = BuddyAllocatorImpl::new(0x100000000);
        let f = || {
            let x: Box<[MaybeUninit<u8>], BuddyAllocatorImpl> =
                Box::new_uninit_slice_in(128, allocator.clone());
            core::mem::drop(x);
        };
        use core::sync::atomic::AtomicBool;
        use std::sync::Arc;
        std::thread::scope(|s| {
            let flag = Arc::new(AtomicBool::new(true));
            for _ in 0..16 {
                let flag = flag.clone();
                s.spawn(move || {
                    while flag.load(Ordering::SeqCst) {
                        f();
                    }
                });
            }
            b.iter(f);
            flag.store(false, Ordering::SeqCst);
        });
    }

    #[bench]
    #[ignore]
    fn bench_contended_allocate_free_no_cache(b: &mut Bencher) {
        let mut allocator = BuddyAllocatorImpl::new(0x100000000);
        allocator.set_caching(false);
        let f = || {
            let x: Box<[MaybeUninit<u8>], BuddyAllocatorImpl> =
                Box::new_uninit_slice_in(128, allocator.clone());
            core::mem::drop(x);
        };
        use core::sync::atomic::AtomicBool;
        use std::sync::Arc;
        std::thread::scope(|s| {
            let flag = Arc::new(AtomicBool::new(true));
            for _ in 0..16 {
                let flag = flag.clone();
                s.spawn(move || {
                    while flag.load(Ordering::SeqCst) {
                        f();
                    }
                });
            }
            b.iter(f);
            flag.store(false, Ordering::SeqCst);
        });
    }

    #[test]
    fn stress_test() {
        use std::hash::{BuildHasher, Hasher, RandomState};
        let mut allocator = BuddyAllocatorImpl::new(0x10000000);
        allocator.set_caching(false);
        let mut v = vec![];
        let random = |limit: usize| {
            let x: u64 = RandomState::new().build_hasher().finish();
            x as usize % limit
        };
        for _ in 0..100000 {
            let used_before = allocator.used_size();
            let remaining = allocator.total_size() - used_before;
            let size = random(core::cmp::min(1 << 21, remaining / 2));
            let alloc =
                Box::<[u8], BuddyAllocatorImpl>::new_uninit_slice_in(size, allocator.clone());
            let used_after = allocator.used_size();
            assert!(used_after >= used_before + size);
            if !v.is_empty() && size % 3 == 0 {
                let number = random(v.len());
                for _ in 0..number {
                    let index = random(v.len());
                    v.remove(index);
                }
            }
            v.push(alloc);
        }
    }
}
