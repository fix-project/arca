#![allow(dead_code)]
use core::arch::asm;

pub(crate) unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value);
}

pub(crate) unsafe fn inb(port: u16) -> u8 {
    let mut value: u8;
    asm!("in al, dx", in("dx") port, out("al") value);
    value
}

pub(crate) unsafe fn outw(port: u16, value: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") value);
}

pub(crate) unsafe fn outl(port: u16, value: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") value);
}

pub(crate) unsafe fn hypercall0(number: u64) -> u64 {
    let mut result: u64 = 0;
    asm!("out dx, al", lateout("rax")result, in("dx") 0, in("eax")number);
    result
}

pub(crate) unsafe fn hypercall1(number: u64, x0: u64) -> u64 {
    let mut result: u64 = 0;
    asm!("out dx, al", lateout("rax") result, in("dx") 0, in("rax")number, in("rdi")x0);
    result
}

pub(crate) unsafe fn hypercall2(number: u64, x0: u64, x1: u64) -> u64 {
    let mut result: u64 = 0;
    asm!("out dx, al", lateout("rax") result, in("dx") 0, in("rax")number, in("rdi")x0, in("rsi")x1);
    result
}

pub(crate) unsafe fn hypercall3(number: u64, x0: u64, x1: u64, x2: u64) -> u64 {
    let mut result: u64 = 0;
    asm!("out dx, al", lateout("rax") result, in("dx") 0, in("rax")number, in("rdi")x0, in("rsi")x1, in("rcx")x2);
    result
}

pub(crate) unsafe fn hypercall4(number: u64, x0: u64, x1: u64, x2: u64, x3: u64) -> u64 {
    let mut result: u64 = 0;
    asm!("out dx, al", lateout("rax") result, in("dx") 0, in("rax")number, in("rdi")x0, in("rsi")x1, in("rcx")x2, in("r10")x3);
    result
}

pub(crate) unsafe fn wait() {
    outb(0x80, 0);
}
