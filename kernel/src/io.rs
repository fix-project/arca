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

pub(crate) unsafe fn wait() {
    outb(0x80, 0);
}
