use core::marker::PhantomData;

use bitfield_struct::bitfield;

use crate::{
    page::Page,
    refcnt::{Page1GB, Page2MB, Page4KB, RcPage},
    vm,
};

extern "C" {
    fn set_pt(page_map: usize);
}

#[core_local]
static mut CURRENT_PAGE_TABLE: Option<RcPage<PageTable256TB>> = None;

pub(crate) unsafe fn set_page_table(page_table: RcPage<PageTable256TB>) {
    set_pt(vm::ka2pa(page_table.as_ptr()));
    CURRENT_PAGE_TABLE.replace(page_table);
}

pub enum Permissions {
    None,
    ReadOnly,
    ReadWrite,
    Executable,
    All,
}

pub trait Descriptor: Default {
    fn new(addr: usize, perm: Permissions) -> Self {
        let mut x: Self = Default::default();
        x.set_address(addr);
        x.set_permissions(perm);
        x
    }

    /// # Safety
    /// This must be a valid bit pattern for this type of descriptor.
    unsafe fn from_bits(x: u64) -> Self;
    fn into_bits(self) -> u64;

    fn set_address(&mut self, addr: usize) -> &mut Self;
    fn address(&self) -> usize;

    fn set_user(&mut self, user: bool) -> &mut Self;
    fn user(&self) -> bool;

    fn set_writeable(&mut self, writeable: bool) -> &mut Self;
    fn writeable(&self) -> bool;

    fn set_execute_disable(&mut self, execute_disable: bool) -> &mut Self;
    fn execute_disable(&self) -> bool;

    fn set_permissions(&mut self, perm: Permissions) -> &mut Self {
        match perm {
            Permissions::None => {
                self.set_user(false);
            }
            Permissions::ReadOnly => {
                self.set_user(true);
                self.set_writeable(false);
                self.set_execute_disable(true);
            }
            Permissions::ReadWrite => {
                self.set_user(true);
                self.set_writeable(true);
                self.set_execute_disable(true);
            }
            Permissions::Executable => {
                self.set_user(true);
                self.set_writeable(false);
                self.set_execute_disable(false);
            }
            Permissions::All => {
                self.set_user(true);
                self.set_writeable(true);
                self.set_execute_disable(false);
            }
        };
        self
    }

    fn permissions(&self) -> Permissions {
        match (self.user(), self.writeable(), self.execute_disable()) {
            (false, _, _) => Permissions::None,
            (true, false, true) => Permissions::ReadOnly,
            (true, true, true) => Permissions::ReadWrite,
            (true, false, false) => Permissions::Executable,
            (true, true, false) => Permissions::All,
        }
    }
}

#[bitfield(u64)]
pub struct TableDescriptor {
    #[bits(default = true)]
    present: bool,
    writeable: bool,
    user: bool,
    write_through: bool,
    cache_disable: bool,
    accessed: bool,
    available_0: bool,
    leaf: bool,
    #[bits(4)]
    available_1: u8,
    #[bits(40)]
    address: usize,
    #[bits(11)]
    available_2: u16,
    execute_disable: bool,
}

#[bitfield(u64)]
pub struct HugePageDescriptor {
    #[bits(default = true)]
    present: bool,
    writeable: bool,
    user: bool,
    write_through: bool,
    cache_disable: bool,
    accessed: bool,
    dirty: bool,
    #[bits(default = true)]
    leaf: bool,
    global: bool,
    #[bits(3)]
    available_0: u8,
    page_attribute_table: bool,
    #[bits(39)]
    address: usize,
    #[bits(7)]
    available_2: u8,
    #[bits(4)]
    protection_key: u8,
    execute_disable: bool,
}

#[bitfield(u64)]
pub struct PageDescriptor {
    #[bits(default = true)]
    present: bool,
    writeable: bool,
    user: bool,
    write_through: bool,
    cache_disable: bool,
    accessed: bool,
    dirty: bool,
    page_attribute_table: bool,
    global: bool,
    #[bits(3)]
    available_0: u8,
    #[bits(40)]
    address: usize,
    #[bits(7)]
    available_2: u8,
    #[bits(4)]
    protection_key: u8,
    execute_disable: bool,
}

impl Descriptor for TableDescriptor {
    unsafe fn from_bits(x: u64) -> Self {
        Self::from_bits(x)
    }

    fn into_bits(self) -> u64 {
        self.into_bits()
    }

    fn set_address(&mut self, addr: usize) -> &mut Self {
        self.set_address(addr >> 12);
        self
    }

    fn address(&self) -> usize {
        self.address() << 12
    }

    fn set_user(&mut self, user: bool) -> &mut Self {
        self.set_user(user);
        self
    }

    fn user(&self) -> bool {
        self.user()
    }

    fn set_writeable(&mut self, writeable: bool) -> &mut Self {
        self.set_writeable(writeable);
        self
    }

    fn writeable(&self) -> bool {
        self.writeable()
    }

    fn set_execute_disable(&mut self, execute_disable: bool) -> &mut Self {
        self.set_execute_disable(execute_disable);
        self
    }

    fn execute_disable(&self) -> bool {
        self.execute_disable()
    }
}

impl Descriptor for HugePageDescriptor {
    unsafe fn from_bits(x: u64) -> Self {
        Self::from_bits(x)
    }

    fn into_bits(self) -> u64 {
        self.into_bits()
    }

    fn set_address(&mut self, addr: usize) -> &mut Self {
        self.set_address(addr >> 13);
        self
    }

    fn address(&self) -> usize {
        self.address() << 13
    }

    fn set_user(&mut self, user: bool) -> &mut Self {
        self.set_user(user);
        self
    }

    fn user(&self) -> bool {
        self.user()
    }

    fn set_writeable(&mut self, writeable: bool) -> &mut Self {
        self.set_writeable(writeable);
        self
    }

    fn writeable(&self) -> bool {
        self.writeable()
    }

    fn set_execute_disable(&mut self, execute_disable: bool) -> &mut Self {
        self.set_execute_disable(execute_disable);
        self
    }

    fn execute_disable(&self) -> bool {
        self.execute_disable()
    }
}

impl Descriptor for PageDescriptor {
    unsafe fn from_bits(x: u64) -> Self {
        Self::from_bits(x)
    }

    fn into_bits(self) -> u64 {
        self.into_bits()
    }

    fn set_address(&mut self, addr: usize) -> &mut Self {
        self.set_address(addr >> 12);
        self
    }

    fn address(&self) -> usize {
        self.address() << 12
    }

    fn set_user(&mut self, user: bool) -> &mut Self {
        self.set_user(user);
        self
    }

    fn user(&self) -> bool {
        self.user()
    }

    fn set_writeable(&mut self, writeable: bool) -> &mut Self {
        self.set_writeable(writeable);
        self
    }

    fn writeable(&self) -> bool {
        self.writeable()
    }

    fn set_execute_disable(&mut self, execute_disable: bool) -> &mut Self {
        self.set_execute_disable(execute_disable);
        self
    }

    fn execute_disable(&self) -> bool {
        self.execute_disable()
    }
}

pub trait HardwarePage: Sized + Clone {}
impl HardwarePage for Page4KB {}
impl HardwarePage for Page2MB {}
impl HardwarePage for Page1GB {}
impl HardwarePage for ! {}

/// # Safety
/// Structs implementing this trait must be valid page tables on this architecture, and must be
/// safely zero-initializable.
pub unsafe trait HardwarePageTable: Sized + Clone {
    fn new() -> Page<Self> {
        unsafe { Page::zeroed().assume_init() }
    }
}

unsafe impl HardwarePageTable for ! {}

pub enum UnmappedPage<P: HardwarePage, T: HardwarePageTable> {
    Null,
    Page(RcPage<P>),
    Global(usize),
    Table(RcPage<T>),
}

pub trait PageTableEntry: Sized + Clone {
    type Page: HardwarePage;
    type Table: HardwarePageTable;
    type PageDescriptor: Descriptor;
    type TableDescriptor: Descriptor;

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

    fn map(
        &mut self,
        page: RcPage<Self::Page>,
        prot: Permissions,
    ) -> UnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe {
            self.set_bits(Self::PageDescriptor::new(vm::ka2pa(page.into_raw()), prot).into_bits());
        }
        original
    }

    /// # Safety
    /// The physical address being mapped must not cause a violation of Rust's safety model.
    unsafe fn map_raw(
        &mut self,
        addr: usize,
        prot: Permissions,
    ) -> UnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe {
            self.set_bits(Self::PageDescriptor::new(addr, prot).into_bits());
        }
        original
    }

    fn chain(
        &mut self,
        table: RcPage<Self::Table>,
        prot: Permissions,
    ) -> UnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe {
            self.set_bits(
                Self::TableDescriptor::new(vm::ka2pa(table.into_raw()), prot).into_bits(),
            );
        }
        original
    }

    fn unmap(&mut self) -> UnmappedPage<Self::Page, Self::Table> {
        if !self.present() {
            UnmappedPage::Null
        } else if self.leaf() {
            unsafe {
                let descriptor = Self::PageDescriptor::from_bits(self.bits());
                self.set_bits(0);
                let addr = descriptor.address();
                let page = RcPage::from_raw(vm::pa2ka(addr));
                UnmappedPage::Page(page)
            }
        } else {
            unsafe {
                let descriptor = Self::TableDescriptor::from_bits(self.bits());
                self.set_bits(0);
                let addr = descriptor.address();
                let table = RcPage::from_raw(vm::pa2ka(addr));
                UnmappedPage::Table(table)
            }
        }
    }

    fn protect(&mut self, prot: Permissions) {
        if !self.present() {
            panic!("Attempting to set protections on nonexistent page.");
        } else if self.leaf() {
            unsafe {
                let mut descriptor = Self::PageDescriptor::from_bits(self.bits());
                descriptor.set_permissions(prot);
                self.set_bits(descriptor.into_bits());
            }
        } else {
            unsafe {
                let mut descriptor = Self::TableDescriptor::from_bits(self.bits());
                descriptor.set_permissions(prot);
                self.set_bits(descriptor.into_bits());
            }
        }
    }

    fn duplicate(&self) -> Self {
        // safety assumption: cloning a page or table will increment the reference count without
        // changing the referenced address
        if !self.present() {
        } else if self.leaf() {
            unsafe {
                let descriptor = Self::PageDescriptor::from_bits(self.bits());
                let addr = descriptor.address();
                let page: RcPage<Self::Page> = RcPage::from_raw(vm::pa2ka(addr));
                let new = page.clone();
                core::mem::forget(page);
                core::mem::forget(new);
            }
        } else {
            unsafe {
                let descriptor = Self::TableDescriptor::from_bits(self.bits());
                let addr = descriptor.address();
                let table: RcPage<Self::Table> = RcPage::from_raw(vm::pa2ka(addr));
                let table = table.clone();
                let new = table.clone();
                core::mem::forget(table);
                core::mem::forget(new);
            }
        }
        unsafe { Self::new(self.bits()) }
    }
}

#[repr(transparent)]
pub struct Entry<P: HardwarePage, T: HardwarePageTable> {
    bits: u64,
    _page: PhantomData<P>,
    _table: PhantomData<T>,
}

pub type PageTable2MBEntry = Entry<Page4KB, !>;
pub type PageTable1GBEntry = Entry<Page2MB, PageTable2MB>;
pub type PageTable512GBEntry = Entry<Page1GB, PageTable1GB>;
pub type PageTable256TBEntry = Entry<!, PageTable512GB>;

pub type PageTable2MB = PageTable<PageTable2MBEntry>;
pub type PageTable1GB = PageTable<PageTable1GBEntry>;
pub type PageTable512GB = PageTable<PageTable512GBEntry>;
pub type PageTable256TB = PageTable<PageTable256TBEntry>;

type PageTable<T> = [T; 512];

unsafe impl HardwarePageTable for PageTable2MB {}
unsafe impl HardwarePageTable for PageTable1GB {}
unsafe impl HardwarePageTable for PageTable512GB {}
unsafe impl HardwarePageTable for PageTable256TB {}

impl PageTableEntry for PageTable2MBEntry {
    type Page = Page4KB;
    type Table = !;

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

impl PageTableEntry for PageTable1GBEntry {
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

impl PageTableEntry for PageTable512GBEntry {
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

impl PageTableEntry for PageTable256TBEntry {
    type Page = !;
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
