use core::{marker::PhantomData, ops::IndexMut};

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

    fn map_unique(
        &mut self,
        page: UniquePage<Self::Page>,
    ) -> HardwareUnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe {
            self.set_bits(
                Self::PageDescriptor::new(vm::ka2pa(UniquePage::into_raw(page)), Permissions::All)
                    .into_bits(),
            );
        }
        original
    }

    fn map_shared(
        &mut self,
        page: SharedPage<Self::Page>,
    ) -> HardwareUnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe {
            let desc = Self::PageDescriptor::new(
                vm::ka2pa(SharedPage::into_raw(page)),
                Permissions::Executable,
            );
            self.set_bits(desc.into_bits());
        }
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
        unsafe {
            let mut desc = Self::PageDescriptor::new(addr, prot);
            desc.set_global(true);
            self.set_bits(desc.into_bits());
        }
        original
    }

    fn chain_shared(
        &mut self,
        table: SharedPage<Self::Table>,
    ) -> HardwareUnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe {
            self.set_bits(
                Self::TableDescriptor::new(
                    vm::ka2pa(SharedPage::into_raw(table)),
                    Permissions::Executable,
                )
                .into_bits(),
            );
        }
        original
    }

    fn chain_unique(
        &mut self,
        table: UniquePage<Self::Table>,
    ) -> HardwareUnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe {
            self.set_bits(
                Self::TableDescriptor::new(
                    vm::ka2pa(UniquePage::into_raw(table)),
                    Permissions::All,
                )
                .into_bits(),
            );
        }
        original
    }

    fn unmap(&mut self) -> HardwareUnmappedPage<Self::Page, Self::Table> {
        if !self.present() {
            HardwareUnmappedPage::None
        } else if self.leaf() {
            unsafe {
                let descriptor = Self::PageDescriptor::from_bits(self.bits());
                self.set_bits(0);
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
                self.set_bits(0);
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

    fn get_protection(&self) -> Permissions {
        if !self.present() {
            Permissions::None
        } else if self.leaf() {
            unsafe {
                let descriptor = Self::PageDescriptor::from_bits(self.bits());
                descriptor.permissions()
            }
        } else {
            unsafe {
                let descriptor = Self::TableDescriptor::from_bits(self.bits());
                descriptor.permissions()
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
        let prot = self.get_protection();
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
