use core::marker::PhantomData;

use bitfield_struct::bitfield;

use crate::{
    page::UniquePage,
    refcnt::{Page1GB, Page2MB, Page4KB, SharedPage},
    vm,
};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Permissions {
    None,
    ReadOnly,
    ReadWrite,
    Executable,
    All,
}

pub trait Descriptor: Default + core::fmt::Debug {
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

    fn set_global(&mut self, global: bool) -> &mut Self;
    fn global(&self) -> bool;

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

    fn set_global(&mut self, _: bool) -> &mut Self {
        unreachable!();
    }

    fn global(&self) -> bool {
        unreachable!();
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

    fn set_global(&mut self, global: bool) -> &mut Self {
        self.set_global(global);
        self
    }

    fn global(&self) -> bool {
        self.global()
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

    fn set_global(&mut self, global: bool) -> &mut Self {
        self.set_global(global);
        self
    }

    fn global(&self) -> bool {
        self.global()
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
pub unsafe trait PageTable: Sized + Clone {
    fn new() -> UniquePage<Self> {
        unsafe { UniquePage::zeroed().assume_init() }
    }
}

unsafe impl PageTable for ! {}

pub enum UnmappedPage<P: HardwarePage, T: PageTable> {
    None,
    Unique(UniquePage<P>),
    Shared(SharedPage<P>),
    Global(usize),
    Table(SharedPage<T>),
}

pub trait PageTableEntry: Sized + Clone {
    type Page: HardwarePage + core::fmt::Debug;
    type Table: PageTable + core::fmt::Debug;
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

    fn nested(&self) -> bool {
        self.present() && ((self.bits() >> 7) & 1 == 0)
    }

    fn map_unique(
        &mut self,
        page: UniquePage<Self::Page>,
    ) -> UnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe {
            self.set_bits(
                Self::PageDescriptor::new(vm::ka2pa(page.into_raw()), Permissions::All).into_bits(),
            );
        }
        original
    }

    fn map_shared(
        &mut self,
        page: SharedPage<Self::Page>,
    ) -> UnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe {
            let desc =
                Self::PageDescriptor::new(vm::ka2pa(page.into_raw()), Permissions::Executable);
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
    ) -> UnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe {
            let mut desc = Self::PageDescriptor::new(addr, prot);
            desc.set_global(true);
            self.set_bits(desc.into_bits());
        }
        original
    }

    fn chain(&mut self, table: SharedPage<Self::Table>) -> UnmappedPage<Self::Page, Self::Table> {
        let original = self.unmap();
        unsafe {
            self.set_bits(
                Self::TableDescriptor::new(vm::ka2pa(table.into_raw()), Permissions::All)
                    .into_bits(),
            );
        }
        original
    }

    fn unmap(&mut self) -> UnmappedPage<Self::Page, Self::Table> {
        if !self.present() {
            UnmappedPage::None
        } else if self.leaf() {
            unsafe {
                let descriptor = Self::PageDescriptor::from_bits(self.bits());
                self.set_bits(0);
                let addr = descriptor.address();
                if descriptor.global() {
                    UnmappedPage::Global(addr)
                } else if descriptor.writeable() {
                    let page = UniquePage::from_raw(vm::pa2ka(addr));
                    UnmappedPage::Unique(page)
                } else {
                    let page = SharedPage::from_raw(vm::pa2ka(addr));
                    UnmappedPage::Shared(page)
                }
            }
        } else {
            unsafe {
                let descriptor = Self::TableDescriptor::from_bits(self.bits());
                self.set_bits(0);
                let addr = descriptor.address();
                let table = SharedPage::from_raw(vm::pa2ka(addr));
                UnmappedPage::Table(table)
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
                if !descriptor.global() {
                    let addr = descriptor.address();
                    let page: SharedPage<Self::Page> = SharedPage::from_raw(vm::pa2ka(addr));
                    let new = page.clone();
                    core::mem::forget(page);
                    core::mem::forget(new);
                }
            }
        } else {
            unsafe {
                let descriptor = Self::TableDescriptor::from_bits(self.bits());
                let addr = descriptor.address();
                let table: SharedPage<Self::Table> = SharedPage::from_raw(vm::pa2ka(addr));
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
pub struct Entry<P: HardwarePage, T: PageTable> {
    bits: u64,
    _page: PhantomData<P>,
    _table: PhantomData<T>,
}

impl<P: HardwarePage, T: PageTable> core::fmt::Debug for Entry<P, T>
where
    Entry<P, T>: PageTableEntry,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let prot = self.get_protection();
        if !self.present() {
            f.debug_struct("None").finish()
        } else if self.leaf() {
            unsafe {
                let descriptor = <Self as PageTableEntry>::PageDescriptor::from_bits(self.bits());
                let addr = descriptor.address();
                if descriptor.global() {
                    f.debug_tuple("Global").field(&addr).field(&prot).finish()
                } else if descriptor.writeable() {
                    f.debug_tuple("Unique").field(&addr).field(&prot).finish()
                } else {
                    f.debug_tuple("Shared").field(&addr).field(&prot).finish()
                }
            }
        } else {
            unsafe {
                let descriptor = <Self as PageTableEntry>::TableDescriptor::from_bits(self.bits());
                let addr = descriptor.address();
                f.debug_tuple("Table").field(&addr).field(&prot).finish()
            }
        }
    }
}

pub type PageTable2MBEntry = Entry<Page4KB, !>;
pub type PageTable1GBEntry = Entry<Page2MB, PageTable2MB>;
pub type PageTable512GBEntry = Entry<Page1GB, PageTable1GB>;
pub type PageTable256TBEntry = Entry<!, PageTable512GB>;

pub type PageTable2MB = [PageTable2MBEntry; 512];
pub type PageTable1GB = [PageTable1GBEntry; 512];
pub type PageTable512GB = [PageTable512GBEntry; 512];
pub type PageTable256TB = [PageTable256TBEntry; 512];

unsafe impl PageTable for PageTable2MB {}
unsafe impl PageTable for PageTable1GB {}
unsafe impl PageTable for PageTable512GB {}
unsafe impl PageTable for PageTable256TB {}

impl PageTableEntry for PageTable2MBEntry {
    type Page = Page4KB;
    type Table = !;

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
