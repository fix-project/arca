use core::{
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
    ops::IndexMut,
};

use crate::prelude::*;

mod descriptor;
mod impossible;

pub use descriptor::*;
pub use impossible::Impossible;

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
        self.set_bits(0);
        self.set_metadata(meta);
    }

    fn set_metadata(&mut self, metadata: u16) {
        unsafe {
            if !self.present() || self.leaf() {
                let mut descriptor = Self::PageDescriptor::from_bits(self.bits());
                descriptor.set_metadata(metadata);
                self.set_bits(descriptor.into_bits());
            } else {
                let mut descriptor = Self::TableDescriptor::from_bits(self.bits());
                descriptor.set_metadata(metadata);
                self.set_bits(descriptor.into_bits());
            }
        }
    }

    fn get_metadata(&self) -> u16 {
        unsafe {
            if !self.present() || self.leaf() {
                let descriptor = Self::PageDescriptor::from_bits(self.bits());
                descriptor.metadata()
            } else {
                let descriptor = Self::TableDescriptor::from_bits(self.bits());
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
        unsafe { self.map_unchecked(UniquePage::into_raw(page), Permissions::All) };
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
        unsafe { self.chain_unchecked(UniquePage::into_raw(table), Permissions::All) };
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
                    let page = UniquePage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                    HardwareUnmappedPage::UniquePage(page)
                } else {
                    let page = SharedPage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                    HardwareUnmappedPage::SharedPage(page)
                }
            }
        } else {
            unsafe {
                let descriptor = Self::TableDescriptor::from_bits(self.bits());
                self.clear_unchecked();
                if descriptor.writeable() {
                    let addr = descriptor.address();
                    let table = UniquePage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                    HardwareUnmappedPage::UniqueTable(table)
                } else {
                    let addr = descriptor.address();
                    let table = SharedPage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
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
            unsafe {
                let mut descriptor = Self::PageDescriptor::from_bits(self.bits());
                if !descriptor.global() {
                    let addr = descriptor.address();
                    if descriptor.writeable() {
                        let page: UniquePage<Self::Page> =
                            UniquePage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                        let new = page.clone();
                        descriptor.set_address(vm::ka2pa(UniquePage::into_raw(new)));
                        core::mem::forget(page);
                    } else {
                        let page: SharedPage<Self::Page> =
                            SharedPage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                        let new = page.clone();
                        core::mem::forget(page);
                        core::mem::forget(new);
                    }
                }
                Self::new(descriptor.into_bits())
            }
        } else {
            unsafe {
                let mut descriptor = Self::TableDescriptor::from_bits(self.bits());
                if descriptor.writeable() {
                    let addr = descriptor.address();
                    let table: UniquePage<Self::Table> =
                        UniquePage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                    let new = table.clone();
                    descriptor.set_address(vm::ka2pa(UniquePage::into_raw(new)));
                    core::mem::forget(table);
                } else {
                    let addr = descriptor.address();
                    let table: SharedPage<Self::Table> =
                        SharedPage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                    let table = table.clone();
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
#[derive(Eq, PartialEq)]
pub struct Entry<P: HardwarePage, T: HardwarePageTable>
where
    Entry<P, T>: HardwarePageTableEntry,
{
    bits: u64,
    _page: PhantomData<P>,
    _table: PhantomData<T>,
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
        unsafe { UniquePage::<Self>::new_zeroed_in(&PHYSICAL_ALLOCATOR).assume_init() }
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
            bits,
            _page: PhantomData,
            _table: PhantomData,
        }
    }

    fn bits(&self) -> u64 {
        self.bits
    }

    unsafe fn set_bits(&mut self, bits: u64) {
        self.bits = bits;
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
            bits,
            _page: PhantomData,
            _table: PhantomData,
        }
    }

    fn bits(&self) -> u64 {
        self.bits
    }

    unsafe fn set_bits(&mut self, bits: u64) {
        self.bits = bits;
    }
}

impl HardwarePageTableEntry for PageTable512GBEntry {
    type Page = Page1GB;
    type Table = PageTable1GB;

    type PageDescriptor = HugePageDescriptor;
    type TableDescriptor = TableDescriptor;

    unsafe fn new(bits: u64) -> Self {
        Self {
            bits,
            _page: PhantomData,
            _table: PhantomData,
        }
    }

    fn bits(&self) -> u64 {
        self.bits
    }

    unsafe fn set_bits(&mut self, bits: u64) {
        self.bits = bits;
    }
}

impl HardwarePageTableEntry for PageTable256TBEntry {
    type Page = Impossible;
    type Table = PageTable512GB;

    type PageDescriptor = HugePageDescriptor;
    type TableDescriptor = TableDescriptor;

    unsafe fn new(bits: u64) -> Self {
        Self {
            bits,
            _page: PhantomData,
            _table: PhantomData,
        }
    }

    fn bits(&self) -> u64 {
        self.bits
    }

    unsafe fn set_bits(&mut self, bits: u64) {
        self.bits = bits;
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
#[derive(Debug, Eq, PartialEq)]
pub struct AugmentedPageTable<T: HardwarePageTable>([ManuallyDrop<T::Entry>; 512]);

impl<T: HardwarePageTable> AugmentedPageTable<T> {
    pub fn new() -> UniquePage<Self> {
        unsafe { UniquePage::<Self>::new_zeroed_in(&PHYSICAL_ALLOCATOR).assume_init() }
    }

    fn set_lower(&mut self, lower: usize) {
        debug_assert!(lower < (1 << 10));
        let entry = &mut self.0[0];
        entry.set_metadata(lower as u16 & 0b1111111111);
    }

    fn get_lower(&self) -> usize {
        let entry = &self.0[0];
        entry.get_metadata() as usize
    }

    fn set_upper(&mut self, upper: usize) {
        debug_assert!(upper < (1 << 10));
        let entry = &mut self.0[1];
        entry.set_metadata(upper as u16 & 0b1111111111);
    }

    fn get_upper(&self) -> usize {
        let entry = &self.0[1];
        entry.get_metadata() as usize
    }

    pub fn entry(&self, index: usize) -> Option<&AugmentedEntry<T::Entry>> {
        let lower = self.get_lower();
        let upper = self.get_upper();
        if index < lower || index >= upper {
            return None;
        }
        unsafe {
            let entry: &T::Entry = &self.0[index];
            let entry: &AugmentedEntry<T::Entry> = core::mem::transmute(entry);
            Some(entry)
        }
    }

    pub fn entry_mut(&mut self, index: usize) -> &mut AugmentedEntry<T::Entry> {
        let lower = self.get_lower();
        let upper = self.get_upper();
        if index < lower {
            self.set_lower(index);
        }
        if index >= upper {
            self.set_upper(index + 1);
        }
        // TODO: does this allow overwriting the metadata?
        unsafe {
            let entry: &mut T::Entry = &mut self.0[index];
            let entry: &mut AugmentedEntry<T::Entry> = core::mem::transmute(entry);
            entry
        }
    }

    pub fn unmap(
        &mut self,
        index: usize,
    ) -> AugmentedUnmappedPage<
        <T::Entry as HardwarePageTableEntry>::Page,
        <T::Entry as HardwarePageTableEntry>::Table,
    > {
        let lower = self.get_lower();
        let upper = self.get_upper();
        if index < lower || index >= upper {
            return AugmentedUnmappedPage::None;
        }
        let original = self.entry_mut(index).unmap();
        if index == upper {
            let mut index = index;
            loop {
                if index == lower {
                    self.set_upper(index);
                    break;
                }
                if self.entry(index).is_some() {
                    self.set_upper(index + 1);
                    break;
                }
                index -= 1;
            }
        }
        if index == lower {
            let mut index = index;
            loop {
                if index == upper || self.entry(index).is_some() {
                    self.set_lower(index);
                    break;
                }
                index += 1;
            }
        }
        original
    }
}

impl<T: HardwarePageTable> Clone for AugmentedPageTable<T> {
    fn clone(&self) -> Self {
        unsafe {
            let array: [ManuallyDrop<T::Entry>; 512] = MaybeUninit::zeroed().assume_init();
            let mut pt = AugmentedPageTable(array);
            let lower = self.get_lower();
            let upper = self.get_upper();
            for i in lower..upper {
                let child = self.entry(i).unwrap().clone();
                *pt.entry_mut(i) = child;
            }
            pt.set_lower(lower);
            pt.set_upper(upper);
            pt
        }
    }
}

impl<T: HardwarePageTable> Drop for AugmentedPageTable<T> {
    fn drop(&mut self) {
        unsafe {
            let lower = self.get_lower();
            let upper = self.get_upper();
            for i in lower..upper {
                let mut entry = ManuallyDrop::new(T::Entry::new(0));
                core::mem::swap(&mut entry, &mut self.0[i]);
                ManuallyDrop::drop(&mut entry);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum AugmentedUnmappedPage<P: HardwarePage, T: HardwarePageTable> {
    None,
    UniquePage(UniquePage<P>),
    SharedPage(SharedPage<P>),
    Global(usize),
    UniqueTable(UniquePage<AugmentedPageTable<T>>),
    SharedTable(SharedPage<AugmentedPageTable<T>>),
}

#[repr(transparent)]
#[derive(Debug)]
pub struct AugmentedEntry<T: HardwarePageTableEntry>(T);

impl<T: HardwarePageTableEntry> AugmentedEntry<T> {
    pub fn map_unique(
        &mut self,
        page: UniquePage<T::Page>,
    ) -> AugmentedUnmappedPage<T::Page, T::Table> {
        let original = self.unmap();
        unsafe {
            self.0
                .map_unchecked(UniquePage::into_raw(page), Permissions::All)
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

    pub fn chain_unique(
        &mut self,
        table: UniquePage<AugmentedPageTable<T::Table>>,
    ) -> AugmentedUnmappedPage<T::Page, T::Table> {
        let original = self.unmap();
        unsafe {
            self.0.chain_unchecked(
                core::mem::transmute(UniquePage::into_raw(table)),
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
                core::mem::transmute(SharedPage::into_raw(table)),
                Permissions::Executable,
            )
        };
        original
    }

    pub fn unmap(&mut self) -> AugmentedUnmappedPage<T::Page, T::Table> {
        if !self.0.present() {
            AugmentedUnmappedPage::None
        } else if self.0.leaf() {
            unsafe {
                let descriptor = T::PageDescriptor::from_bits(self.0.bits());
                self.0.clear_unchecked();
                let addr = descriptor.address();
                if descriptor.global() {
                    AugmentedUnmappedPage::Global(addr)
                } else if descriptor.writeable() {
                    let page = UniquePage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                    AugmentedUnmappedPage::UniquePage(page)
                } else {
                    let page = SharedPage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                    AugmentedUnmappedPage::SharedPage(page)
                }
            }
        } else {
            unsafe {
                let descriptor = T::TableDescriptor::from_bits(self.0.bits());
                self.0.clear_unchecked();
                if descriptor.writeable() {
                    let addr = descriptor.address();
                    let table = UniquePage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                    AugmentedUnmappedPage::UniqueTable(table)
                } else {
                    let addr = descriptor.address();
                    let table = SharedPage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                    AugmentedUnmappedPage::SharedTable(table)
                }
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
            unsafe {
                let mut descriptor = T::PageDescriptor::from_bits(self.0.bits());
                if !descriptor.global() {
                    let addr = descriptor.address();
                    if descriptor.writeable() {
                        let page: UniquePage<T::Page> =
                            UniquePage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                        let new = page.clone();
                        descriptor.set_address(vm::ka2pa(UniquePage::into_raw(new)));
                        core::mem::forget(page);
                    } else {
                        let page: SharedPage<T::Page> =
                            SharedPage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                        let new = page.clone();
                        core::mem::forget(page);
                        core::mem::forget(new);
                    }
                }
                AugmentedEntry(T::new(descriptor.into_bits()))
            }
        } else {
            unsafe {
                let mut descriptor = T::TableDescriptor::from_bits(self.0.bits());
                if descriptor.writeable() {
                    let addr = descriptor.address();
                    let table: UniquePage<AugmentedPageTable<T::Table>> =
                        UniquePage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                    let new = table.clone();
                    descriptor.set_address(vm::ka2pa(UniquePage::into_raw(new)));
                    core::mem::forget(table);
                } else {
                    let addr = descriptor.address();
                    let table: SharedPage<AugmentedPageTable<T::Table>> =
                        SharedPage::from_raw_in(vm::pa2ka(addr), &PHYSICAL_ALLOCATOR);
                    let table = table.clone();
                    let new = table.clone();
                    core::mem::forget(table);
                    core::mem::forget(new);
                }
                AugmentedEntry(T::new(descriptor.into_bits()))
            }
        }
    }
}
