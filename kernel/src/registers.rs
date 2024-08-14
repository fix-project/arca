use core::arch::asm;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlReg0;

#[allow(unused)]
impl ControlReg0 {
    pub const PE: u64 = 1 << 0;
    pub const MP: u64 = 1 << 1;
    pub const EM: u64 = 1 << 2;
    pub const TS: u64 = 1 << 3;
    pub const ET: u64 = 1 << 4;
    pub const NE: u64 = 1 << 5;
    pub const WP: u64 = 1 << 16;
    pub const AM: u64 = 1 << 18;
    pub const NW: u64 = 1 << 29;
    pub const CD: u64 = 1 << 30;
    pub const PG: u64 = 1 << 31;
}

pub unsafe fn write_cr0(x: u64) {
    asm!("mov cr0, {x}", x=in(reg)x);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlReg4;

#[allow(unused)]
impl ControlReg4 {
    pub const VME: u64 = 1 << 0;
    pub const PVI: u64 = 1 << 1;
    pub const TSD: u64 = 1 << 2;
    pub const DE: u64 = 1 << 3;
    pub const PSE: u64 = 1 << 4;
    pub const PAE: u64 = 1 << 5;
    pub const MCE: u64 = 1 << 6;
    pub const PGE: u64 = 1 << 7;
    pub const PCE: u64 = 1 << 8;
    pub const OSFXSR: u64 = 1 << 9;
    pub const OSXMMEXCPT: u64 = 1 << 10;
    pub const UMIP: u64 = 1 << 11;
    pub const VMXE: u64 = 1 << 13;
    pub const SMXE: u64 = 1 << 14;
    pub const FSGSBASE: u64 = 1 << 16;
    pub const PCIDE: u64 = 1 << 17;
    pub const OSXSAVE: u64 = 1 << 18;
    pub const SMEP: u64 = 1 << 20;
    pub const SMAP: u64 = 1 << 21;
    pub const PKE: u64 = 1 << 22;
    pub const CET: u64 = 1 << 23;
    pub const PKS: u64 = 1 << 24;
}

pub unsafe fn write_cr4(x: u64) {
    asm!("mov cr4, {x}", x=in(reg)x);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtendedFeatureEnableReg;

#[allow(unused)]
impl ExtendedFeatureEnableReg {
    pub const SCE: u64 = 1 << 0;
    pub const LME: u64 = 1 << 8;
    pub const LMA: u64 = 1 << 10;
    pub const NXE: u64 = 1 << 11;
    pub const SVME: u64 = 1 << 12;
    pub const LMSLE: u64 = 1 << 13;
    pub const FFXSR: u64 = 1 << 14;
    pub const TCE: u64 = 1 << 15;
}

pub unsafe fn write_efer(x: u64) {
    crate::msr::wrmsr(0xC0000080, x);
}


