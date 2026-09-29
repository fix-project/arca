#![allow(clippy::double_parens)]
use bitfield_struct::bitfield;

pub use crate::gdt::PrivilegeLevel;

#[repr(C, align(4096))]
#[derive(Copy, Clone, Debug)]
pub struct Idt(pub [IdtEntry; 256]);

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Default)]
pub struct IdtEntry {
    offset_low: u16,
    segment_selector: u16,
    attributes: Attributes,
    offset_mid: u16,
    offset_high: u32,
    _reserved: u32,
}

#[repr(u8)]
#[derive(Debug)]
pub enum GateType {
    Interrupt = 0xE,
    Trap = 0xF,
}

impl GateType {
    pub const fn from_bits(bits: u8) -> Self {
        match bits {
            0xE => GateType::Interrupt,
            0xF => GateType::Trap,
            _ => GateType::Interrupt,
        }
    }

    pub const fn into_bits(self) -> u8 {
        self as u8
    }
}

#[bitfield(u16)]
struct Attributes {
    #[bits(3)]
    ist: u8,
    #[bits(5)]
    _0: u8,
    #[bits(4)]
    gate_type: GateType,
    _1: bool,
    #[bits(2)]
    dpl: u8,
    present: bool,
}

impl IdtEntry {
    pub fn new(
        address: usize,
        segment: u16,
        ist_offset: Option<u8>,
        gate_type: GateType,
        privileged: PrivilegeLevel,
    ) -> IdtEntry {
        IdtEntry {
            offset_low: address as u16,
            segment_selector: segment,
            attributes: Attributes::new()
                .with_ist(ist_offset.map(|x| x + 1).unwrap_or(0))
                .with_gate_type(gate_type)
                .with_dpl(if privileged == PrivilegeLevel::User {
                    3
                } else {
                    0
                })
                .with_present(true),
            offset_mid: (address >> 16) as u16,
            offset_high: (address >> 32) as u32,
            _reserved: 0,
        }
    }
}

const _: () = const {
    assert!(core::mem::size_of::<IdtEntry>() == 16);
};

#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct IdtDescriptor<'a> {
    size: u16,
    addr: &'a Idt,
}

impl<'a> IdtDescriptor<'a> {
    pub const fn new(value: &'a Idt) -> Self {
        IdtDescriptor {
            addr: value,
            size: (core::mem::size_of_val(value) - 1) as u16,
        }
    }
}

impl<'a> From<&'a Idt> for IdtDescriptor<'a> {
    fn from(value: &'a Idt) -> Self {
        IdtDescriptor::new(value)
    }
}
