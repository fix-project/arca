#![allow(unused)]

use core::arch::asm;

pub use common::controlreg::*;

pub unsafe fn write_cr0(x: u64) {
    asm!("mov cr0, {x}", x=in(reg)x);
}

pub unsafe fn write_cr4(x: u64) {
    asm!("mov cr4, {x}", x=in(reg)x);
}

pub unsafe fn write_efer(x: u64) {
    crate::msr::wrmsr(0xC0000080, x);
}

pub fn read_efer() -> u64 {
    unsafe { crate::msr::rdmsr(0xC0000080) }
}
