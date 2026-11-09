#![allow(unsafe_op_in_unsafe_fn)]
use core::{
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
    ops::{IndexMut, Range},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::prelude::*;

mod descriptor;
mod impossible;

pub use descriptor::*;
pub use impossible::Impossible;

// CoW + writable denotes an ownership conversion on an unloaded leaf.
const COPY_ON_WRITE: u64 = 1 << 58;
const TABLE_COPY_ON_WRITE: u64 = 1 << 61;

fn leaf_bits<P: HardwarePage>(bits: u64) -> bool {
    bits & 1 != 0 && (P::SIZE == 4096 || bits & (1 << 7) != 0)
}

pub trait HardwarePage: Sized + Clone {
    type Parent: HardwarePageTable;
    const SIZE: usize = core::mem::size_of::<Self>();
}

impl HardwarePage for Page4KB {
    type Parent = PageTable2MB;
}

impl HardwarePage for Page2MB {
    type Parent = PageTable1GB;
}

impl HardwarePage for Page1GB {
    type Parent = PageTable512GB;
}

#[derive(Debug, Clone)]
pub enum HardwareUnmappedPage<P: HardwarePage, T: HardwarePageTable> {
    None,
    UniquePage(UniquePage<P>),
    SharedPage(SharedPage<P>),
    CopyOnWrite(SharedPage<P>),
    Global(usize),
    UniqueTable(UniquePage<T>),
    SharedTable(SharedPage<T>),
}

pub trait HardwarePageTableEntry: Sized + Clone {
    type Page: HardwarePage + core::fmt::Debug + core::cmp::Eq;
    type Table: HardwarePageTable + core::fmt::Debug + core::cmp::Eq;
    type PageDescriptor: Descriptor + core::fmt::Debug;
    type TableDescriptor: Descriptor + core::fmt::Debug;

    fn bits(&self) -> u64;
    /// # Safety
    /// This entry must be a present leaf.
    unsafe fn clone_leaf(&self) -> Self;

    /// # Safety
    /// This must be a valid bit pattern for a page table entry.
    unsafe fn set_bits(&mut self, bits: u64);

    /// # Safety
    /// This must be a valid bit pattern for a page table entry.
    unsafe fn new(bits: u64) -> Self;

    fn present(&self) -> bool {
        self.bits() & 1 == 1
    }

    fn leaf(&self) -> bool {
        self.present() && ((self.bits() >> 7) & 1 == 1)
    }

    fn writeable(&self) -> bool {
        self.present() && (self.bits() & 2 == 2)
    }

    fn nested(&self) -> bool {
        self.present() && !self.leaf()
    }

    /// # Safety
    /// This pointer must be a valid, aligned pointer for the correct page type.  Mapping this
    /// pointer with these permissions must not violate Rust's aliasing model.  If anything was
    /// mapped here before, it must be explicitly freed.
    unsafe fn map_unchecked(&mut self, page: *const Self::Page, prot: Permissions) {
        self.set_address_unchecked(vm::ka2pa(page), prot)
    }

    /// # Safety
    /// This pointer must be a valid, aligned physical address for the correct page type.  Mapping
    /// this page with these permissions must not violate Rust's aliasing model.  If anything was
    /// mapped here before, it must be explicitly freed.
    unsafe fn set_address_unchecked(&mut self, page: usize, prot: Permissions) {
        let mut descriptor = Self::PageDescriptor::new(page, prot);
        descriptor.set_metadata(self.get_metadata());
        self.set_bits(descriptor.into_bits())
    }

    /// # Safety
    /// This pointer must be a valid, aligned pointer for the correct table type.  Mapping this
    /// pointer with these permissions must not violate Rust's aliasing model.  If anything was
    /// mapped here before, it must be explicitly freed.
    unsafe fn chain_unchecked(&mut self, page: *const Self::Table, prot: Permissions) {
        let mut descriptor = Self::TableDescriptor::new(vm::ka2pa(page), prot);
        descriptor.set_metadata(self.get_metadata());
        self.set_bits(descriptor.into_bits())
    }

    /// # Safety
    /// If anything was mapped here before, it must be explicitly freed.
    unsafe fn clear_unchecked(&mut self) {
        let meta = self.get_metadata();
        let mut descriptor = Self::PageDescriptor::from_bits(0);
        descriptor.set_metadata(meta);
        self.set_bits(descriptor.into_bits());
    }

    fn set_metadata(&mut self, metadata: u16) {
        let bits = self.bits();
        unsafe {
            if bits & 1 == 0 || leaf_bits::<Self::Page>(bits) {
                let mut descriptor = Self::PageDescriptor::from_bits(bits);
                descriptor.set_metadata(metadata);
                self.set_bits(descriptor.into_bits());
            } else {
                let mut descriptor = Self::TableDescriptor::from_bits(bits);
                descriptor.set_metadata(metadata);
                self.set_bits(descriptor.into_bits());
            }
        }
    }

    fn get_metadata(&self) -> u16 {
        let bits = self.bits();
        unsafe {
            if bits & 1 == 0 || leaf_bits::<Self::Page>(bits) {
                let descriptor = Self::PageDescriptor::from_bits(bits);
                descriptor.metadata()
            } else {
                let descriptor = Self::TableDescriptor::from_bits(bits);
                descriptor.metadata()
            }
        }
    }

    fn get_permissions(&self) -> Permissions {
        unsafe {
            if !self.present() {
                Permissions::None
            } else if self.leaf() {
                let descriptor = Self::PageDescriptor::from_bits(self.bits());
                descriptor.permissions()
            } else {
                let descriptor = Self::TableDescriptor::from_bits(self.bits());
                descriptor.permissions()
            }
        }
    }

    fn get_page(&self) -> *const Self::Page {
        unsafe {
            if self.leaf() {
                let descriptor = Self::PageDescriptor::from_bits(self.bits());
                vm::pa2ka(descriptor.address())
            } else {
                core::ptr::null()
            }
        }
    }

    fn get_table(&self) -> *const Self::Table {
        unsafe {
            if self.nested() {
                let descriptor = Self::TableDescriptor::from_bits(self.bits());
                vm::pa2ka(descriptor.address())
            } else {
                core::ptr::null()
            }
        }
    }

    fn get_address(&self) -> Option<usize> {
        unsafe {
            if self.present() {
                Some(if self.leaf() {
                    let descriptor = Self::PageDescriptor::from_bits(self.bits());
                    descriptor.address()
                } else {
                    let descriptor = Self::TableDescriptor::from_bits(self.bits());
                    descriptor.address()
                })
            } else {
                None
            }
        }
    }

    fn map_unique(
        &mut self,
        page: UniquePage<Self::Page>,
    ) -> HardwareUnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe {
            self.map_unchecked(
                UniquePage::into_raw_with_allocator(page).0,
                Permissions::All,
            )
        };
        original
    }

    fn map_shared(
        &mut self,
        page: SharedPage<Self::Page>,
    ) -> HardwareUnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe { self.map_unchecked(SharedPage::into_raw(page), Permissions::Executable) };
        original
    }

    /// # Safety
    /// The physical address being mapped must not cause a violation of Rust's safety model.
    unsafe fn map_global(
        &mut self,
        addr: usize,
        prot: Permissions,
    ) -> HardwareUnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe { self.set_address_unchecked(addr, prot) };
        original
    }

    fn chain_shared(
        &mut self,
        table: SharedPage<Self::Table>,
    ) -> HardwareUnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe { self.chain_unchecked(SharedPage::into_raw(table), Permissions::Executable) };
        original
    }

    fn chain_unique(
        &mut self,
        table: UniquePage<Self::Table>,
    ) -> HardwareUnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe {
            self.chain_unchecked(
                UniquePage::into_raw_with_allocator(table).0,
                Permissions::All,
            )
        };
        original
    }

    fn unmap(&mut self) -> HardwareUnmappedPage<Self::Page, Self::Table> {
        if !self.present() {
            HardwareUnmappedPage::None
        } else if self.leaf() {
            unsafe {
                let descriptor = Self::PageDescriptor::from_bits(self.bits());
                self.clear_unchecked();
                let addr = descriptor.address();
                if descriptor.global() {
                    HardwareUnmappedPage::Global(addr)
                } else if descriptor.writeable() {
                    let page = UniquePage::from_raw_in(vm::pa2ka(addr), BuddyAllocator);
                    HardwareUnmappedPage::UniquePage(page)
                } else {
                    let page = SharedPage::from_raw_in(vm::pa2ka(addr), BuddyAllocator);
                    if descriptor.into_bits() & COPY_ON_WRITE != 0 {
                        HardwareUnmappedPage::CopyOnWrite(page)
                    } else {
                        HardwareUnmappedPage::SharedPage(page)
                    }
                }
            }
        } else {
            unsafe {
                let descriptor = Self::TableDescriptor::from_bits(self.bits());
                self.clear_unchecked();
                if descriptor.writeable() {
                    let addr = descriptor.address();
                    let table = UniquePage::from_raw_in(vm::pa2ka(addr), BuddyAllocator);
                    HardwareUnmappedPage::UniqueTable(table)
                } else {
                    let addr = descriptor.address();
                    let table = SharedPage::from_raw_in(vm::pa2ka(addr), BuddyAllocator);
                    HardwareUnmappedPage::SharedTable(table)
                }
            }
        }
    }

    fn duplicate(&self) -> Self {
        // safety assumption: cloning a shared page or table will increment the reference count
        // without changing the referenced address
        if !self.present() {
            unsafe { Self::new(self.bits()) }
        } else if self.leaf() {
            unsafe { self.clone_leaf() }
        } else {
            unsafe {
                let mut descriptor = Self::TableDescriptor::from_bits(self.bits());
                if descriptor.writeable() {
                    let addr = descriptor.address();
                    let table: UniquePage<Self::Table> =
                        UniquePage::from_raw_in(vm::pa2ka(addr), BuddyAllocator);
                    let new = table.clone();
                    descriptor.set_address(vm::ka2pa(UniquePage::into_raw_with_allocator(new).0));
                    core::mem::forget(table);
                } else {
                    let addr = descriptor.address();
                    let table: SharedPage<Self::Table> =
                        SharedPage::from_raw_in(vm::pa2ka(addr), BuddyAllocator);
                    let new = table.clone();
                    core::mem::forget(table);
                    core::mem::forget(new);
                }
                Self::new(descriptor.into_bits())
            }
        }
    }
}

#[repr(transparent)]
pub struct Entry<P: HardwarePage, T: HardwarePageTable>
where
    Entry<P, T>: HardwarePageTableEntry,
{
    bits: AtomicU64,
    _page: PhantomData<P>,
    _table: PhantomData<T>,
}

impl<P: HardwarePage, T: HardwarePageTable> PartialEq for Entry<P, T>
where
    Self: HardwarePageTableEntry,
{
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other) || self.bits() == other.bits()
    }
}
impl<P: HardwarePage, T: HardwarePageTable> Eq for Entry<P, T> where Self: HardwarePageTableEntry {}

impl<P: HardwarePage, T: HardwarePageTable> Entry<P, T>
where
    Self: HardwarePageTableEntry,
{
    fn load_bits(&self) -> u64 {
        loop {
            let bits = self.bits.load(Ordering::Acquire);
            let leaf = P::SIZE == 4096 || bits & (1 << 7) != 0;
            if !leaf || bits & (COPY_ON_WRITE | 3) != (COPY_ON_WRITE | 3) {
                return bits;
            }
            core::hint::spin_loop();
        }
    }

    fn clone_leaf_entry(&self) -> Self {
        loop {
            let bits = self.load_bits();
            let descriptor =
                unsafe { <Self as HardwarePageTableEntry>::PageDescriptor::from_bits(bits) };
            if descriptor.global() {
                return unsafe { Self::new(bits) };
            }
            if !descriptor.writeable() {
                let page: SharedPage<<Self as HardwarePageTableEntry>::Page> =
                    unsafe { SharedPage::from_raw(vm::pa2ka(descriptor.address())) };
                let clone = page.clone();
                core::mem::forget(page);
                core::mem::forget(clone);
                return unsafe { Self::new(bits) };
            }
            if self
                .bits
                .compare_exchange(
                    bits,
                    bits | COPY_ON_WRITE,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_err()
            {
                continue;
            }
            // Writable mappings are uniquely owned and are cloned while unloaded.
            let page: UniquePage<<Self as HardwarePageTableEntry>::Page> =
                unsafe { UniquePage::from_raw_in(vm::pa2ka(descriptor.address()), BuddyAllocator) };
            let page: SharedPage<<Self as HardwarePageTableEntry>::Page> = page.into();
            let clone = page.clone();
            core::mem::forget(page);
            core::mem::forget(clone);
            let bits = (bits & !2) | COPY_ON_WRITE;
            self.bits.store(bits, Ordering::Release);
            return unsafe { Self::new(bits) };
        }
    }
}

impl<P: HardwarePage, T: HardwarePageTable> core::fmt::Debug for Entry<P, T>
where
    Entry<P, T>: HardwarePageTableEntry,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let prot = self.get_permissions();
        if !self.present() {
            f.debug_struct("None").finish()
        } else if self.leaf() {
            unsafe {
                let descriptor =
                    <Self as HardwarePageTableEntry>::PageDescriptor::from_bits(self.bits());
                let addr = descriptor.address() as *mut ();
                if descriptor.global() {
                    f.debug_tuple("Global").field(&addr).field(&prot).finish()
                } else if descriptor.writeable() {
                    f.debug_tuple("UniquePage")
                        .field(&addr)
                        .field(&prot)
                        .finish()
                } else {
                    f.debug_tuple("SharedPage")
                        .field(&addr)
                        .field(&prot)
                        .finish()
                }
            }
        } else {
            unsafe {
                let descriptor =
                    <Self as HardwarePageTableEntry>::TableDescriptor::from_bits(self.bits());
                let addr = descriptor.address() as *mut ();
                if descriptor.writeable() {
                    f.debug_tuple("UniqueTable")
                        .field(&addr)
                        .field(&prot)
                        .finish()
                } else {
                    f.debug_tuple("SharedTable")
                        .field(&addr)
                        .field(&prot)
                        .finish()
                }
            }
        }
    }
}

pub type PageTable2MBEntry = Entry<Page4KB, Impossible>;
pub type PageTable1GBEntry = Entry<Page2MB, PageTable2MB>;
pub type PageTable512GBEntry = Entry<Page1GB, PageTable1GB>;
pub type PageTable256TBEntry = Entry<Impossible, PageTable512GB>;

pub type PageTable2MB = [PageTable2MBEntry; 512];
pub type PageTable1GB = [PageTable1GBEntry; 512];
pub type PageTable512GB = [PageTable512GBEntry; 512];
pub type PageTable256TB = [PageTable256TBEntry; 512];

/// # Safety
///
/// All implementors of this trait must be valid page tables on the hardware. The Entry and Parent
/// fields must be consistent.
pub unsafe trait HardwarePageTable:
    Sized + Clone + IndexMut<usize, Output = Self::Entry>
{
    type Entry: HardwarePageTableEntry + core::fmt::Debug + Eq;
    type Parent: HardwarePageTable;

    const SIZE: usize;

    fn new() -> UniquePage<Self> {
        unsafe { UniquePage::<Self>::new_zeroed_in(BuddyAllocator).assume_init() }
    }
}

unsafe impl HardwarePageTable for PageTable2MB {
    type Entry = PageTable2MBEntry;
    type Parent = PageTable1GB;
    const SIZE: usize = 1 << 21;
}

unsafe impl HardwarePageTable for PageTable1GB {
    type Entry = PageTable1GBEntry;
    type Parent = PageTable512GB;
    const SIZE: usize = 1 << 30;
}

unsafe impl HardwarePageTable for PageTable512GB {
    type Entry = PageTable512GBEntry;
    type Parent = PageTable256TB;
    const SIZE: usize = 1 << 39;
}

unsafe impl HardwarePageTable for PageTable256TB {
    type Entry = PageTable256TBEntry;
    type Parent = Impossible;
    const SIZE: usize = 1 << 48;
}

impl HardwarePageTableEntry for PageTable2MBEntry {
    type Page = Page4KB;
    type Table = Impossible;

    type PageDescriptor = PageDescriptor;
    type TableDescriptor = TableDescriptor;

    unsafe fn new(bits: u64) -> Self {
        Self {
            bits: AtomicU64::new(bits),
            _page: PhantomData,
            _table: PhantomData,
        }
    }

    fn bits(&self) -> u64 {
        self.load_bits()
    }

    unsafe fn set_bits(&mut self, bits: u64) {
        *self.bits.get_mut() = bits;
    }

    unsafe fn clone_leaf(&self) -> Self {
        self.clone_leaf_entry()
    }

    fn leaf(&self) -> bool {
        self.present()
    }
}

impl HardwarePageTableEntry for PageTable1GBEntry {
    type Page = Page2MB;
    type Table = PageTable2MB;

    type PageDescriptor = HugePageDescriptor;
    type TableDescriptor = TableDescriptor;

    unsafe fn new(bits: u64) -> Self {
        Self {
            bits: AtomicU64::new(bits),
            _page: PhantomData,
            _table: PhantomData,
        }
    }

    fn bits(&self) -> u64 {
        self.load_bits()
    }

    unsafe fn set_bits(&mut self, bits: u64) {
        *self.bits.get_mut() = bits;
    }

    unsafe fn clone_leaf(&self) -> Self {
        self.clone_leaf_entry()
    }
}

impl HardwarePageTableEntry for PageTable512GBEntry {
    type Page = Page1GB;
    type Table = PageTable1GB;

    type PageDescriptor = HugePageDescriptor;
    type TableDescriptor = TableDescriptor;

    unsafe fn new(bits: u64) -> Self {
        Self {
            bits: AtomicU64::new(bits),
            _page: PhantomData,
            _table: PhantomData,
        }
    }

    fn bits(&self) -> u64 {
        self.load_bits()
    }

    unsafe fn set_bits(&mut self, bits: u64) {
        *self.bits.get_mut() = bits;
    }

    unsafe fn clone_leaf(&self) -> Self {
        self.clone_leaf_entry()
    }
}

impl HardwarePageTableEntry for PageTable256TBEntry {
    type Page = Impossible;
    type Table = PageTable512GB;

    type PageDescriptor = HugePageDescriptor;
    type TableDescriptor = TableDescriptor;

    unsafe fn new(bits: u64) -> Self {
        Self {
            bits: AtomicU64::new(bits),
            _page: PhantomData,
            _table: PhantomData,
        }
    }

    fn bits(&self) -> u64 {
        self.load_bits()
    }

    unsafe fn set_bits(&mut self, bits: u64) {
        *self.bits.get_mut() = bits;
    }

    unsafe fn clone_leaf(&self) -> Self {
        self.clone_leaf_entry()
    }
}

impl Clone for PageTable2MBEntry {
    fn clone(&self) -> Self {
        self.duplicate()
    }
}

impl Clone for PageTable1GBEntry {
    fn clone(&self) -> Self {
        self.duplicate()
    }
}

impl Clone for PageTable512GBEntry {
    fn clone(&self) -> Self {
        self.duplicate()
    }
}

impl Clone for PageTable256TBEntry {
    fn clone(&self) -> Self {
        self.duplicate()
    }
}

impl<P: HardwarePage, T: HardwarePageTable> Drop for Entry<P, T>
where
    Entry<P, T>: HardwarePageTableEntry,
{
    fn drop(&mut self) {
        self.unmap();
    }
}

#[repr(transparent)]
#[derive(Eq, PartialEq)]
pub struct AugmentedPageTable<T: HardwarePageTable>([ManuallyDrop<T::Entry>; 512]);

impl<T: HardwarePageTable> core::fmt::Debug for AugmentedPageTable<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "AugmentedPageTable @ {self:p}")
    }
}

impl<T: HardwarePageTable> AugmentedPageTable<T> {
    pub const SIZE: usize = T::SIZE;

    pub fn new() -> UniquePage<Self> {
        unsafe { UniquePage::<Self>::new_zeroed_in(BuddyAllocator).assume_init() }
    }

    fn bounds(&self) -> Range<usize> {
        let lower = self.0[0].get_metadata() as usize;
        let last = self.0[1].get_metadata() as usize;
        if lower > last || (lower == last && !self.0[lower].present()) {
            lower..lower
        } else {
            lower..last + 1
        }
    }

    fn set_bounds(&mut self, bounds: Range<usize>) {
        debug_assert!(bounds.start <= bounds.end && bounds.end <= 512);
        let (lower, last) = if bounds.is_empty() {
            (0, 0)
        } else {
            (bounds.start, bounds.end - 1)
        };
        self.0[0].set_metadata(lower as u16);
        self.0[1].set_metadata(last as u16);
    }

    pub fn len(&self) -> usize {
        self.bounds().len()
    }

    pub fn offset(&self) -> usize {
        self.bounds().start
    }

    pub fn is_empty(&self) -> bool {
        self.bounds().is_empty()
    }

    fn make_read_only(&mut self) {
        for index in self.bounds() {
            let entry = self.entry_mut(index);
            match entry.unmap() {
                AugmentedUnmappedPage::None => {}
                AugmentedUnmappedPage::UniquePage(page) => {
                    entry.map_shared(page.into());
                }
                AugmentedUnmappedPage::SharedPage(page)
                | AugmentedUnmappedPage::CopyOnWrite(page) => {
                    entry.map_shared(page);
                }
                AugmentedUnmappedPage::UniqueTable(table) => {
                    entry.chain_shared(table.into());
                }
                AugmentedUnmappedPage::SharedTable(table)
                | AugmentedUnmappedPage::CopyOnWriteTable(table) => {
                    entry.chain_shared(table);
                }
                AugmentedUnmappedPage::Global(_) => unreachable!(),
            }
        }
    }

    pub fn entry(&self, index: usize) -> Option<&AugmentedEntry<T::Entry>> {
        if !self.bounds().contains(&index) {
            return None;
        }
        Some(unsafe {
            &*(&self.0[index] as *const ManuallyDrop<T::Entry>).cast::<AugmentedEntry<T::Entry>>()
        })
    }

    pub fn read(&self, mut offset: usize, mut bytes: &mut [u8]) -> usize {
        let mut read = 0;
        let span = T::SIZE / 512;
        while offset < T::SIZE && !bytes.is_empty() {
            let Some(entry) = self.entry(offset / span) else {
                break;
            };
            if !entry.present() {
                break;
            }
            let count = bytes.len().min(span - offset % span);
            let count = if entry.0.leaf() {
                let descriptor = unsafe {
                    <T::Entry as HardwarePageTableEntry>::PageDescriptor::from_bits(entry.0.bits())
                };
                assert!(!descriptor.global(), "cannot borrow a global mapping");
                let page =
                    unsafe { core::slice::from_raw_parts(entry.0.get_page().cast::<u8>(), span) };
                bytes[..count].copy_from_slice(&page[offset % span..offset % span + count]);
                count
            } else {
                let table = unsafe {
                    &*entry
                        .0
                        .get_table()
                        .cast::<AugmentedPageTable<<T::Entry as HardwarePageTableEntry>::Table>>()
                };
                table.read(offset % span, &mut bytes[..count])
            };
            read += count;
            offset += count;
            bytes = &mut bytes[count..];
            if count == 0 {
                break;
            }
        }
        read
    }

    pub fn entry_mut(&mut self, index: usize) -> &mut AugmentedEntry<T::Entry> {
        assert!(index < 512);
        let bounds = self.bounds();
        let expanded = if bounds.is_empty() {
            index..index + 1
        } else {
            bounds.start.min(index)..bounds.end.max(index + 1)
        };
        if expanded != bounds {
            self.set_bounds(expanded);
        }
        unsafe {
            &mut *(&mut self.0[index] as *mut ManuallyDrop<T::Entry>)
                .cast::<AugmentedEntry<T::Entry>>()
        }
    }

    pub fn unmap(
        &mut self,
        index: usize,
    ) -> AugmentedUnmappedPage<
        <T::Entry as HardwarePageTableEntry>::Page,
        <T::Entry as HardwarePageTableEntry>::Table,
    > {
        let bounds = self.bounds();
        if !bounds.contains(&index) {
            return AugmentedUnmappedPage::None;
        }
        let original = self.entry_mut(index).unmap();
        let mut lower = bounds.start;
        let mut upper = bounds.end;
        if index == lower {
            while lower < upper && !self.0[lower].present() {
                lower += 1;
            }
        }
        if index + 1 == upper {
            while upper > lower && !self.0[upper - 1].present() {
                upper -= 1;
            }
        }
        if (lower..upper) != bounds {
            self.set_bounds(lower..upper);
        }
        original
    }

    pub fn resolve_cow(&mut self, address: usize, invalidate: &mut impl FnMut(bool)) -> bool {
        self.resolve_cow_path(address, false, invalidate)
    }

    fn resolve_cow_path(
        &mut self,
        address: usize,
        ancestor_changed: bool,
        invalidate: &mut impl FnMut(bool),
    ) -> bool {
        if address >= T::SIZE {
            return false;
        }
        let span = T::SIZE / 512;
        let Some(entry) = self.entry(address / span) else {
            return false;
        };
        if !entry.present() {
            return false;
        }
        if entry.0.leaf() {
            if entry.0.bits() & COPY_ON_WRITE == 0 {
                return ancestor_changed && entry.0.writeable();
            }
            let entry = self.entry_mut(address / span);
            unsafe {
                let mut descriptor =
                    <T::Entry as HardwarePageTableEntry>::PageDescriptor::from_bits(entry.0.bits());
                let page: SharedPage<<T::Entry as HardwarePageTableEntry>::Page> =
                    SharedPage::from_raw(vm::pa2ka(descriptor.address()));
                let (page, retained) = SharedPage::into_unique_retained(page);
                descriptor.set_address(vm::ka2pa(UniquePage::into_raw_with_allocator(page).0));
                descriptor.set_writeable(true);
                entry.0.set_bits(descriptor.into_bits() & !COPY_ON_WRITE);
                invalidate(false);
                drop(retained);
            }
            true
        } else {
            let entry = self.entry_mut(address / span);
            let changed = !entry.0.writeable();
            let retained = if changed {
                if entry.0.bits() & TABLE_COPY_ON_WRITE == 0 {
                    return false;
                }
                let AugmentedUnmappedPage::CopyOnWriteTable(table) = entry.unmap() else {
                    unreachable!()
                };
                let (table, retained) = SharedPage::into_unique_retained(table);
                entry.chain_unique(table);
                retained
            } else {
                None
            };
            let resolved = unsafe {
                (&mut *entry
                    .0
                    .get_table()
                    .cast_mut()
                    .cast::<AugmentedPageTable<<T::Entry as HardwarePageTableEntry>::Table>>())
                    .resolve_cow_path(address % span, ancestor_changed || changed, invalidate)
            };
            if changed {
                invalidate(true);
            }
            drop(retained);
            resolved
        }
    }
}

impl<T: HardwarePageTable> Clone for AugmentedPageTable<T> {
    fn clone(&self) -> Self {
        unsafe {
            let array: [ManuallyDrop<T::Entry>; 512] = MaybeUninit::zeroed().assume_init();
            let mut pt = AugmentedPageTable(array);
            let bounds = self.bounds();
            for index in bounds.clone() {
                let entry =
                    &*core::ptr::from_ref(&self.0[index]).cast::<AugmentedEntry<T::Entry>>();
                pt.0[index] = ManuallyDrop::new(entry.clone().0);
            }
            pt.set_bounds(bounds);
            pt
        }
    }
}

impl<T: HardwarePageTable> Drop for AugmentedPageTable<T> {
    fn drop(&mut self) {
        let bounds = self.bounds();
        for entry in &mut self.0[bounds] {
            unsafe {
                let entry = AugmentedEntry(ManuallyDrop::into_inner(core::ptr::read(entry)));
                drop(entry.into_unmapped());
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum AugmentedUnmappedPage<P: HardwarePage, T: HardwarePageTable> {
    None,
    UniquePage(UniquePage<P>),
    SharedPage(SharedPage<P>),
    CopyOnWrite(SharedPage<P>),
    Global(usize),
    UniqueTable(UniquePage<AugmentedPageTable<T>>),
    SharedTable(SharedPage<AugmentedPageTable<T>>),
    CopyOnWriteTable(SharedPage<AugmentedPageTable<T>>),
}

#[repr(transparent)]
#[derive(Debug)]
pub struct AugmentedEntry<T: HardwarePageTableEntry>(T);

impl<T: HardwarePageTableEntry> AugmentedEntry<T> {
    pub fn table(&self) -> Option<&AugmentedPageTable<T::Table>> {
        if !self.0.nested() {
            return None;
        }
        Some(unsafe { &*self.0.get_table().cast::<AugmentedPageTable<T::Table>>() })
    }

    pub(crate) fn with_table_mut<R>(
        &mut self,
        writable: bool,
        f: impl FnOnce(&mut AugmentedPageTable<T::Table>) -> R,
    ) -> R {
        let (mut table, shared, cow) = match self.unmap() {
            AugmentedUnmappedPage::None => (AugmentedPageTable::new(), false, false),
            AugmentedUnmappedPage::UniqueTable(table) => (table, false, false),
            AugmentedUnmappedPage::SharedTable(table) => {
                (SharedPage::into_unique(table), true, false)
            }
            AugmentedUnmappedPage::CopyOnWriteTable(table) => {
                (SharedPage::into_unique(table), true, true)
            }
            _ => unreachable!(),
        };
        if shared && !cow && writable {
            table.make_read_only();
        }
        let result = f(&mut table);
        if shared && !writable {
            self.chain_shared(table.into());
            if cow {
                unsafe { self.0.set_bits(self.0.bits() | TABLE_COPY_ON_WRITE) };
            }
        } else {
            self.chain_unique(table);
        }
        result
    }

    pub fn present(&self) -> bool {
        self.0.present()
    }

    /// # Safety
    /// The mapping must be identical in every address space until globally invalidated.
    pub(crate) unsafe fn mark_global(&mut self) {
        assert!(self.0.present() && self.0.leaf());
        let mut descriptor = T::PageDescriptor::from_bits(self.0.bits());
        descriptor.set_global(true);
        unsafe { self.0.set_bits(descriptor.into_bits()) };
    }

    pub fn map_unique(
        &mut self,
        page: UniquePage<T::Page>,
    ) -> AugmentedUnmappedPage<T::Page, T::Table> {
        let original = self.unmap();
        unsafe {
            self.0.map_unchecked(
                UniquePage::into_raw_with_allocator(page).0,
                Permissions::All,
            )
        };
        original
    }

    pub fn map_shared(
        &mut self,
        page: SharedPage<T::Page>,
    ) -> AugmentedUnmappedPage<T::Page, T::Table> {
        let original = self.unmap();
        unsafe {
            self.0
                .map_unchecked(SharedPage::into_raw(page), Permissions::Executable)
        };
        original
    }

    /// # Safety
    /// The physical address being mapped must not cause a violation of Rust's safety model.
    pub unsafe fn map_global(
        &mut self,
        addr: usize,
        prot: Permissions,
    ) -> AugmentedUnmappedPage<T::Page, T::Table> {
        let original = self.unmap();
        unsafe { self.0.set_address_unchecked(addr, prot) };
        original
    }

    pub fn map_cow(
        &mut self,
        page: SharedPage<T::Page>,
    ) -> AugmentedUnmappedPage<T::Page, T::Table> {
        let original = self.map_shared(page);
        unsafe {
            self.0.set_bits(self.0.bits() | COPY_ON_WRITE);
        }
        original
    }

    pub fn chain_unique(
        &mut self,
        table: UniquePage<AugmentedPageTable<T::Table>>,
    ) -> AugmentedUnmappedPage<T::Page, T::Table> {
        let original = self.unmap();
        unsafe {
            self.0.chain_unchecked(
                core::mem::transmute::<
                    *mut AugmentedPageTable<<T as HardwarePageTableEntry>::Table>,
                    *const <T as HardwarePageTableEntry>::Table,
                >(UniquePage::into_raw_with_allocator(table).0),
                Permissions::All,
            )
        };
        original
    }

    pub fn chain_shared(
        &mut self,
        table: SharedPage<AugmentedPageTable<T::Table>>,
    ) -> AugmentedUnmappedPage<T::Page, T::Table> {
        let original = self.unmap();
        unsafe {
            self.0.chain_unchecked(
                core::mem::transmute::<
                    *mut AugmentedPageTable<<T as HardwarePageTableEntry>::Table>,
                    *const <T as HardwarePageTableEntry>::Table,
                >(SharedPage::into_raw(table)),
                Permissions::Executable,
            )
        };
        original
    }

    /// # Safety
    ///
    /// This function must not be used to produce a mapping which violates Rust's aliasing rules.
    pub unsafe fn chain_unchecked(
        &mut self,
        table: *const T::Table,
        prot: Permissions,
    ) -> AugmentedUnmappedPage<T::Page, T::Table> {
        let original = self.unmap();
        unsafe { self.0.chain_unchecked(table, prot) };
        original
    }

    pub fn unmap(&mut self) -> AugmentedUnmappedPage<T::Page, T::Table> {
        let bits = self.0.bits();
        if bits & 1 != 0 {
            unsafe { self.0.clear_unchecked() };
        }
        unsafe { Self::take_mapping(bits) }
    }

    fn into_unmapped(self) -> AugmentedUnmappedPage<T::Page, T::Table> {
        let entry = ManuallyDrop::new(self);
        unsafe { Self::take_mapping(entry.0.bits()) }
    }

    /// # Safety
    /// The caller must transfer the mapping's ownership and stop using its bits.
    unsafe fn take_mapping(bits: u64) -> AugmentedUnmappedPage<T::Page, T::Table> {
        if bits & 1 == 0 {
            AugmentedUnmappedPage::None
        } else if leaf_bits::<T::Page>(bits) {
            unsafe {
                let descriptor = T::PageDescriptor::from_bits(bits);
                let addr = descriptor.address();
                if descriptor.global() {
                    AugmentedUnmappedPage::Global(addr)
                } else if descriptor.writeable() {
                    let page = UniquePage::from_raw_in(vm::pa2ka(addr), BuddyAllocator);
                    AugmentedUnmappedPage::UniquePage(page)
                } else {
                    let page = SharedPage::from_raw_in(vm::pa2ka(addr), BuddyAllocator);
                    if descriptor.into_bits() & COPY_ON_WRITE != 0 {
                        AugmentedUnmappedPage::CopyOnWrite(page)
                    } else {
                        AugmentedUnmappedPage::SharedPage(page)
                    }
                }
            }
        } else {
            unsafe {
                let descriptor = T::TableDescriptor::from_bits(bits);
                if descriptor.writeable() {
                    let addr = descriptor.address();
                    let table = UniquePage::from_raw_in(vm::pa2ka(addr), BuddyAllocator);
                    AugmentedUnmappedPage::UniqueTable(table)
                } else {
                    let addr = descriptor.address();
                    let table = SharedPage::from_raw_in(vm::pa2ka(addr), BuddyAllocator);
                    if descriptor.into_bits() & TABLE_COPY_ON_WRITE != 0 {
                        AugmentedUnmappedPage::CopyOnWriteTable(table)
                    } else {
                        AugmentedUnmappedPage::SharedTable(table)
                    }
                }
            }
        }
    }

    pub fn replace(
        &mut self,
        entry: AugmentedUnmappedPage<T::Page, T::Table>,
    ) -> AugmentedUnmappedPage<T::Page, T::Table> {
        match entry {
            AugmentedUnmappedPage::None => self.unmap(),
            AugmentedUnmappedPage::UniquePage(page) => self.map_unique(page),
            AugmentedUnmappedPage::SharedPage(page) => self.map_shared(page),
            AugmentedUnmappedPage::CopyOnWrite(page) => self.map_cow(page),
            AugmentedUnmappedPage::Global(_) => panic!(),
            AugmentedUnmappedPage::UniqueTable(table) => self.chain_unique(table),
            AugmentedUnmappedPage::SharedTable(table) => self.chain_shared(table),
            AugmentedUnmappedPage::CopyOnWriteTable(table) => {
                let original = self.chain_shared(table);
                unsafe {
                    self.0.set_bits(self.0.bits() | TABLE_COPY_ON_WRITE);
                }
                original
            }
        }
    }
}

impl<T: HardwarePageTableEntry> Clone for AugmentedEntry<T> {
    fn clone(&self) -> Self {
        // safety assumption: cloning a shared page or table will increment the reference count
        // without changing the referenced address
        if !self.0.present() {
            unsafe { AugmentedEntry(T::new(self.0.bits())) }
        } else if self.0.leaf() {
            unsafe { AugmentedEntry(self.0.clone_leaf()) }
        } else {
            unsafe {
                let mut descriptor = T::TableDescriptor::from_bits(self.0.bits());
                if descriptor.writeable() {
                    let addr = descriptor.address();
                    let table: UniquePage<AugmentedPageTable<T::Table>> =
                        UniquePage::from_raw_in(vm::pa2ka(addr), BuddyAllocator);
                    let new = table.clone();
                    descriptor.set_address(vm::ka2pa(UniquePage::into_raw_with_allocator(new).0));
                    core::mem::forget(table);
                } else {
                    let addr = descriptor.address();
                    let table: SharedPage<AugmentedPageTable<T::Table>> =
                        SharedPage::from_raw_in(vm::pa2ka(addr), BuddyAllocator);
                    let new = table.clone();
                    core::mem::forget(table);
                    core::mem::forget(new);
                }
                AugmentedEntry(T::new(descriptor.into_bits()))
            }
        }
    }
}

impl<T: HardwarePageTable> AsRef<T> for AugmentedPageTable<T> {
    fn as_ref(&self) -> &T {
        unsafe { core::mem::transmute(self) }
    }
}

pub(crate) type RetiredLeaf = AugmentedUnmappedPage<Page4KB, Impossible>;

impl AugmentedPageTable<PageTable2MB> {
    pub(crate) fn can_map(&self, _: usize, length: usize) -> bool {
        length == 4096
    }

    pub(crate) fn prepare_write_path(&mut self, _: usize, _: usize) -> bool {
        false
    }

    pub(crate) fn can_replace_zero_range(&self, _: Range<usize>) -> bool {
        true
    }

    pub(crate) fn replace_zero_range(
        &mut self,
        range: Range<usize>,
        base: usize,
        writable: bool,
        retire: &mut impl FnMut(usize, RetiredLeaf),
    ) -> bool {
        for index in range.start / 4096..range.end / 4096 {
            let entry = self.entry_mut(index);
            let original = if writable {
                entry.map_cow(SharedPage::zero())
            } else {
                entry.map_shared(SharedPage::zero())
            };
            retire(base + index * 4096, original);
        }
        false
    }
}

macro_rules! zero_range_table {
    ($table:ty) => {
        impl AugmentedPageTable<$table> {
            pub(crate) fn can_map(&self, address: usize, length: usize) -> bool {
                let span = Self::SIZE / 512;
                if length == span {
                    return true;
                }
                let Some(entry) = self.entry(address / span) else {
                    return true;
                };
                if !entry.present() {
                    return true;
                }
                if entry.0.leaf() {
                    return false;
                }
                let child = unsafe {
                    &*entry.0.get_table().cast::<AugmentedPageTable<
                        <<$table as HardwarePageTable>::Entry as HardwarePageTableEntry>::Table,
                    >>()
                };
                child.can_map(address % span, length)
            }

            pub(crate) fn prepare_write_path(&mut self, address: usize, length: usize) -> bool {
                let span = Self::SIZE / 512;
                if length == span {
                    return false;
                }
                let entry = self.entry_mut(address / span);
                let relaxed = entry.present() && !entry.0.writeable();
                entry.with_table_mut(true, |child| {
                    child.prepare_write_path(address % span, length)
                }) || relaxed
            }

            pub(crate) fn can_replace_zero_range(&self, range: Range<usize>) -> bool {
                let span = Self::SIZE / 512;
                for index in range.start / span..range.end.div_ceil(span) {
                    let Some(entry) = self.entry(index) else {
                        continue;
                    };
                    if !entry.present() {
                        continue;
                    }
                    if entry.0.leaf() {
                        return false;
                    }
                    let start = range.start.max(index * span) - index * span;
                    let end = range.end.min((index + 1) * span) - index * span;
                    let child = unsafe {
                        &*entry.0.get_table().cast::<AugmentedPageTable<
                            <<$table as HardwarePageTable>::Entry as HardwarePageTableEntry>::Table,
                        >>()
                    };
                    if !child.can_replace_zero_range(start..end) {
                        return false;
                    }
                }
                true
            }

            pub(crate) fn replace_zero_range(
                &mut self,
                range: Range<usize>,
                base: usize,
                writable: bool,
                retire: &mut impl FnMut(usize, RetiredLeaf),
            ) -> bool {
                let span = Self::SIZE / 512;
                let mut changed = false;
                for index in range.start / span..range.end.div_ceil(span) {
                    let start = range.start.max(index * span) - index * span;
                    let end = range.end.min((index + 1) * span) - index * span;
                    let entry = self.entry_mut(index);
                    let previous = entry.0.get_address();
                    changed |= writable && entry.present() && !entry.0.writeable();
                    changed |= entry.with_table_mut(writable, |child| {
                        child.replace_zero_range(start..end, base + index * span, writable, retire)
                    });
                    changed |= previous.is_some() && previous != entry.0.get_address();
                }
                changed
            }
        }
    };
}

zero_range_table!(PageTable1GB);
zero_range_table!(PageTable512GB);
zero_range_table!(PageTable256TB);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn augmented_bounds_survive_shared_boundary_entries() {
        let mut table = AugmentedPageTable::<PageTable2MB>::new();
        assert!(table.is_empty());
        for index in [0, 1, 255, 511] {
            let mut page =
                unsafe { UniquePage::<Page4KB>::new_zeroed_in(BuddyAllocator).assume_init() };
            page[0] = index as u8;
            table.entry_mut(index).map_unique(page);
        }
        assert_eq!(table.bounds(), 0..512);
        let snapshot = table.clone();
        for index in [0, 1, 255, 511] {
            assert_eq!(
                table.entry(index).unwrap().0.bits() & COPY_ON_WRITE,
                COPY_ON_WRITE
            );
            let mut byte = [0];
            assert_eq!(snapshot.read(index * 4096, &mut byte), 1);
            assert_eq!(byte[0], index as u8);
        }
        for (index, bounds) in [(511, 0..256), (0, 1..256), (255, 1..2), (1, 0..0)] {
            assert!(matches!(
                table.unmap(index),
                AugmentedUnmappedPage::CopyOnWrite(_)
            ));
            assert_eq!(table.bounds(), bounds);
        }
        assert!(table.clone().is_empty());
        let page = unsafe { UniquePage::<Page4KB>::new_zeroed_in(BuddyAllocator).assume_init() };
        table.entry_mut(511).map_unique(page);
        assert_eq!(table.bounds(), 511..512);
        drop(table.unmap(511));
        assert!(table.is_empty());
        assert_eq!(snapshot.bounds(), 0..512);
    }
}
