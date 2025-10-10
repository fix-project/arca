#![allow(clippy::double_parens)]
use bitfield_struct::bitfield;

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

    fn set_metadata(&mut self, metadata: u16) -> &mut Self;
    fn metadata(&self) -> u16;

    fn set_permissions(&mut self, perm: Permissions) -> &mut Self {
        match perm {
            Permissions::None => {
                self.set_user(false);
                self.set_writeable(true);
                self.set_execute_disable(false);
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

    fn set_metadata(&mut self, metadata: u16) -> &mut Self {
        self.set_available_2(metadata);
        self
    }

    fn metadata(&self) -> u16 {
        self.available_2()
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

    fn set_metadata(&mut self, metadata: u16) -> &mut Self {
        self.set_available_0((metadata & 0x7) as u8);
        self.set_available_2((metadata >> 3) as u8);
        self
    }

    fn metadata(&self) -> u16 {
        self.available_0() as u16 | ((self.available_2() as u16) << 3)
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

    fn set_metadata(&mut self, metadata: u16) -> &mut Self {
        self.set_available_0((metadata & 0x7) as u8);
        self.set_available_2((metadata >> 3) as u8);
        self
    }

    fn metadata(&self) -> u16 {
        self.available_0() as u16 | ((self.available_2() as u16) << 3)
    }
}
