use core::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use bitfield_struct::bitfield;

use crate::{
    buddy::{Block, Page1GB, Page4KB},
    vm,
};

extern "C" {
    fn set_pt(page_map: *const ());
}

pub(crate) unsafe fn set_page_map(page_map: PageMapLevel4) {
    set_pt(vm::ka2pa(page_map.into_raw()));
}

#[bitfield(u64)]
pub struct TableDescriptor {
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
pub struct LeafDescriptor {
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
    #[bits(17)]
    _sbz: u32,
    #[bits(22)]
    address: usize,
    #[bits(7)]
    available_2: u8,
    #[bits(4)]
    protection_key: u8,
    execute_disable: bool,
}

#[bitfield(u64)]
pub struct CommonDescriptor {
    present: bool,
    writeable: bool,
    user: bool,
    write_through: bool,
    cache_disable: bool,
    accessed: bool,
    _skip0: bool,
    leaf: bool,
    #[bits(55)]
    _skip1: u64,
    execute_disable: bool,
}

#[bitfield(u64)]
pub struct LeafPageTableEntry {
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
    address: u64,
    #[bits(7)]
    available_2: u8,
    #[bits(4)]
    protection_key: u8,
    execute_disable: bool,
}

pub enum Permissions {
    None,
    ReadOnly,
    ReadWrite,
    Executable,
    All,
}

#[repr(C)]
#[derive(Debug)]
pub struct PageTable<T> {
    block: Page4KB,
    ptr: *mut [T; 512],
}

impl<T> Default for PageTable<T> {
    fn default() -> Self {
        let mut block = Page4KB::new().expect("could not allocate page table");
        block.fill(0);
        let ptr = block.kernel();
        let ptr = ptr as *mut [T; 512];
        Self { block, ptr }
    }
}

impl<T> Deref for PageTable<T> {
    type Target = [T; 512];

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T> DerefMut for PageTable<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

pub(crate) type PageMapLevel4 = PageTable<Entry<!, PageDirectoryPointerTable>>;
pub(crate) type PageDirectoryPointerTable = PageTable<Entry<Page1GB, !>>;

pub(crate) trait Leaf {
    const ORDER: usize;

    fn into_raw(self) -> *mut ();
    unsafe fn from_raw(addr: *mut ()) -> Self;
}

impl<const N: usize> Leaf for Block<N> {
    const ORDER: usize = N;

    fn into_raw(self) -> *mut () {
        self.into_raw() as *mut ()
    }

    unsafe fn from_raw(addr: *mut ()) -> Self {
        Self::from_raw(addr as *mut u8)
    }
}

pub(crate) trait Table {
    fn into_raw(self) -> *mut ();
    unsafe fn from_raw(addr: *mut ()) -> Self;
}

impl<T> Table for PageTable<T> {
    fn into_raw(self) -> *mut () {
        let p = self.ptr as *mut ();
        core::mem::forget(self);
        p
    }

    unsafe fn from_raw(addr: *mut ()) -> Self {
        let block = Page4KB::from_raw(addr as *mut u8);
        Self {
            ptr: addr as *mut [T; 512],
            block,
        }
    }
}

impl Leaf for ! {
    const ORDER: usize = 0;

    fn into_raw(self) -> *mut () {
        unreachable!();
    }

    unsafe fn from_raw(_: *mut ()) -> Self {
        unreachable!();
    }
}

impl Table for ! {
    fn into_raw(self) -> *mut () {
        unreachable!();
    }

    unsafe fn from_raw(_: *mut ()) -> Self {
        unreachable!();
    }
}

pub(crate) union Entry<L: Leaf, T: Table> {
    raw: u64,
    common: CommonDescriptor,
    leaf: LeafDescriptor,
    table: TableDescriptor,
    phantom_leaf: PhantomData<L>,
    phantom_table: PhantomData<T>,
}

pub(crate) enum ResolvedEntry<L, T> {
    Null,
    Leaf(L),
    Table(T),
}

impl<L: Leaf, T: Table> Entry<L, T> {
    pub fn is_null(&self) -> bool {
        unsafe { !self.common.present() }
    }

    pub fn is_leaf(&self) -> bool {
        !self.is_null() && unsafe { self.common.leaf() }
    }

    pub fn is_table(&self) -> bool {
        !self.is_null() && !self.is_leaf()
    }

    pub fn map(&mut self, leaf: L, permissions: Permissions) -> ResolvedEntry<L, T> {
        let old = self.unmap();
        let leaf = leaf.into_raw();
        self.leaf = LeafDescriptor::new()
            .with_present(true)
            .with_leaf(true)
            .with_address(vm::ka2pa(leaf) as usize >> L::ORDER);
        self.protect(permissions);
        old
    }

    pub fn chain(&mut self, table: T, permissions: Permissions) -> ResolvedEntry<L, T> {
        let old = self.unmap();
        self.table = TableDescriptor::new()
            .with_present(true)
            .with_leaf(false)
            .with_address(vm::ka2pa(table.into_raw()) as usize >> 12);
        self.protect(permissions);
        old
    }

    pub fn protect(&mut self, permissions: Permissions) {
        let x = unsafe { self.common };
        self.common = match permissions {
            Permissions::None => x.with_user(false),
            Permissions::ReadOnly => x
                .with_user(true)
                .with_writeable(false)
                .with_execute_disable(true),
            Permissions::ReadWrite => x
                .with_user(true)
                .with_writeable(true)
                .with_execute_disable(true),
            Permissions::Executable => x
                .with_user(true)
                .with_writeable(false)
                .with_execute_disable(false),
            Permissions::All => x
                .with_user(true)
                .with_writeable(true)
                .with_execute_disable(false),
        };
    }

    pub fn unmap(&mut self) -> ResolvedEntry<L, T> {
        let result = if self.is_null() {
            ResolvedEntry::Null
        } else if self.is_leaf() {
            unsafe {
                let descriptor = self.leaf;
                let addr = descriptor.address() << L::ORDER;
                let leaf = L::from_raw(vm::pa2ka_mut(addr as *mut ()));
                ResolvedEntry::Leaf(leaf)
            }
        } else {
            unsafe {
                let descriptor = self.table;
                let addr = descriptor.address() << 12;
                let table = T::from_raw(vm::pa2ka_mut(addr as *mut ()));
                ResolvedEntry::Table(table)
            }
        };
        self.common = CommonDescriptor::new().with_present(false);
        result
    }
}

impl<L: Leaf, T: Table> core::fmt::Debug for Entry<L, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_null() {
            f.debug_tuple("Entry::Null").finish()
        } else if self.is_leaf() {
            f.debug_tuple("Entry::Leaf")
                .field(unsafe { &self.leaf })
                .finish()
        } else if self.is_table() {
            f.debug_tuple("Entry::Table")
                .field(unsafe { &self.table })
                .finish()
        } else {
            f.debug_tuple("Entry::Invalid")
                .field(unsafe { &self.raw })
                .finish()
        }
    }
}

impl<L: Leaf, T: Table> Drop for Entry<L, T> {
    fn drop(&mut self) {
        self.unmap();
    }
}
