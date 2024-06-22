use core::cell::LazyCell;

use bitfield_struct::bitfield;

use crate::tss::{TaskStateSegment, TSS};

#[core_local]
pub(crate) static GDT: LazyCell<[GdtEntry; 8]> = LazyCell::new(|| {
    [
        GdtEntry::null(),                                                // 00: null
        GdtEntry::code64(Readability::Readable, PrivilegeLevel::System), // 08: kernel code
        GdtEntry::data(Writeability::Writeable, PrivilegeLevel::System), // 10: kernel data
        GdtEntry::null(),                                                // 18: user code (32-bit)
        GdtEntry::data(Writeability::Writeable, PrivilegeLevel::User),   // 20: user data
        GdtEntry::code64(Readability::Readable, PrivilegeLevel::User),   // 28: user code (64-bit)
        GdtEntry::tss0(unsafe { &**TSS }),                               // 30: TSS (low)
        GdtEntry::tss1(unsafe { &**TSS }),                               // 38: TSS (high)
    ]
});

#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct GdtDescriptor<'a> {
    size: u16,
    addr: &'a GdtEntry,
}

impl<'a> GdtDescriptor<'a> {
    pub const fn new(value: &'a [GdtEntry]) -> Self {
        GdtDescriptor {
            addr: &value[0],
            size: (core::mem::size_of_val(value) - 1) as u16,
        }
    }
}

impl<'a> From<&'a [GdtEntry]> for GdtDescriptor<'a> {
    fn from(value: &'a [GdtEntry]) -> Self {
        GdtDescriptor::new(value)
    }
}

#[bitfield(u64)]
pub struct GdtEntry {
    #[bits(16)]
    limit_low: u16,
    #[bits(24)]
    base_low: u32,
    #[bits(8)]
    access: Access,
    #[bits(4)]
    limit_high: u8,
    #[bits(4)]
    flags: Flags,
    #[bits(8)]
    base_high: u8,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PrivilegeLevel {
    System,
    User,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Readability {
    Readable,
    NotReadable,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Writeability {
    Writeable,
    NotWriteable,
}

impl GdtEntry {
    const fn new_with_segment(base: u32, limit: u32, access: Access, flags: Flags) -> GdtEntry {
        GdtEntry::new()
            .with_base_high((base >> 24) as u8)
            .with_base_low(base & 0xffffff)
            .with_limit_high((limit >> 16) as u8 & 0xf)
            .with_limit_low(limit as u16)
            .with_access(access)
            .with_flags(flags)
    }

    const fn new_with_settings(access: Access, flags: Flags) -> GdtEntry {
        GdtEntry::new_with_segment(0, -1i64 as u32, access, flags)
    }

    pub const fn null() -> GdtEntry {
        GdtEntry::from_bits(0)
    }

    pub fn code64(readable: Readability, privileged: PrivilegeLevel) -> GdtEntry {
        GdtEntry::new_with_settings(
            Access::code(
                readable == Readability::Readable,
                privileged == PrivilegeLevel::System,
            ),
            Flags::long_mode_code(),
        )
    }

    pub fn code32(readable: Readability, privileged: PrivilegeLevel) -> GdtEntry {
        GdtEntry::new_with_settings(
            Access::code(
                readable == Readability::Readable,
                privileged == PrivilegeLevel::System,
            ),
            Flags::protected_mode_code(),
        )
    }

    pub fn data(writeable: Writeability, privileged: PrivilegeLevel) -> GdtEntry {
        GdtEntry::new_with_settings(
            Access::data(
                writeable == Writeability::Writeable,
                privileged == PrivilegeLevel::System,
            ),
            Flags::data(),
        )
    }

    pub fn tss0(tss: *const TaskStateSegment) -> GdtEntry {
        GdtEntry::new_with_segment(
            tss as usize as u32,
            core::mem::size_of::<TaskStateSegment>() as u32,
            Access::tss(),
            Flags::tss(),
        )
    }

    pub fn tss1(tss: *const TaskStateSegment) -> GdtEntry {
        GdtEntry::from_bits((tss as usize >> 32) as u64)
    }
}

#[bitfield(u8)]
pub struct Access {
    a_accessed: bool,
    rw_readable_writeable: bool,
    dc_direction_conforming: bool,
    e_executable: bool,
    s_descriptor_type: bool,
    #[bits(2)]
    dpl_descriptor_privilege_level: u8,
    p_present: bool,
}

impl Access {
    const fn code(readable: bool, privileged: bool) -> Access {
        Access::new()
            .with_a_accessed(false)
            .with_rw_readable_writeable(readable)
            .with_dc_direction_conforming(false)
            .with_e_executable(true)
            .with_s_descriptor_type(true)
            .with_dpl_descriptor_privilege_level(if privileged { 0 } else { 3 })
            .with_p_present(true)
    }

    const fn data(writeable: bool, privileged: bool) -> Access {
        Access::new()
            .with_a_accessed(false)
            .with_rw_readable_writeable(writeable)
            .with_dc_direction_conforming(false)
            .with_e_executable(false)
            .with_s_descriptor_type(true)
            .with_dpl_descriptor_privilege_level(if privileged { 0 } else { 3 })
            .with_p_present(true)
    }

    const fn tss() -> Access {
        Access::from_bits(0x9).with_p_present(true)
    }
}

#[bitfield(u8)]
struct Flags {
    _reserved: bool,
    l_long_mode_code: bool,
    db_size: bool,
    g_granularity: bool,
    #[bits(4)]
    _padding: u8,
}

impl Flags {
    const fn long_mode_code() -> Flags {
        Flags::new()
            .with_l_long_mode_code(true)
            .with_db_size(false)
            .with_g_granularity(true)
    }

    const fn data() -> Flags {
        Flags::new()
            .with_l_long_mode_code(false)
            .with_db_size(true)
            .with_g_granularity(true)
    }

    const fn protected_mode_code() -> Flags {
        Flags::new()
            .with_l_long_mode_code(false)
            .with_db_size(true)
            .with_g_granularity(true)
    }

    const fn tss() -> Flags {
        Flags::new()
            .with_l_long_mode_code(false)
            .with_db_size(true)
            .with_g_granularity(false)
    }
}
