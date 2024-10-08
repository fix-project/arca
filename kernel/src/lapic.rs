#![allow(dead_code)]
use bitfield_struct::bitfield;
use core::{arch::x86_64::_mm_pause, cell::RefCell, marker::PhantomData};

use crate::{
    io::outb,
    msr::{rdmsr, wrmsr},
};

#[bitfield(u32)]
struct TimerConfig {
    vector: u8,
    #[bits(4)]
    _reserved0: u8,
    pending: bool,
    #[bits(3)]
    _reserved1: u8,
    mask: bool,
    #[bits(2)]
    mode: TimerMode,
    #[bits(13)]
    _reserved2: u16,
}

#[repr(u8)]
#[derive(Debug)]
enum TimerMode {
    OneShot = 0b00,
    Periodic = 0b01,
    TimeStampCounterDeadline = 0b10,
}

impl TimerMode {
    pub const fn from_bits(bits: u8) -> Self {
        match bits {
            0b00 => TimerMode::OneShot,
            0b01 => TimerMode::Periodic,
            0b10 => TimerMode::TimeStampCounterDeadline,
            _ => TimerMode::OneShot,
        }
    }

    pub const fn into_bits(self) -> u8 {
        self as u8
    }
}

pub struct LocalApic(PhantomData<()>);

impl LocalApic {
    pub fn read(&self, index: usize) -> u32 {
        assert!(index <= 0x3FF);
        (unsafe { rdmsr(0x800 + index as u32) }) as u32
    }

    pub fn write(&self, index: usize, value: u32) {
        assert!(index <= 0x3FF);
        unsafe { wrmsr(0x800 + index as u32, value as u64) }
    }

    pub fn write64(&self, index: usize, value: u64) {
        assert!(index <= 0x3FF);
        unsafe { wrmsr(0x800 + index as u32, value) }
    }

    pub fn id(&self) -> u32 {
        self.read(0x2)
    }

    pub fn version(&self) -> u32 {
        self.read(0x3)
    }

    fn set_spurious_interrupt_vector(&mut self, vector: u8) {
        self.write(0xF, (self.read(0xF) & !0xf) | vector as u32)
    }

    fn set_apic_enabled(&mut self, enabled: bool) {
        if enabled {
            self.write(0xF, self.read(0xF) | 0x100);
        } else {
            self.write(0xF, self.read(0xF) & !0x100);
        }
    }

    fn get_timer(&mut self) -> TimerConfig {
        TimerConfig::from_bits(self.read(0x32))
    }

    fn set_timer(&mut self, entry: TimerConfig) {
        self.write(0x32, entry.into_bits());
    }

    pub unsafe fn clear_interrupt(&mut self) {
        self.write(0xB, 0);
    }

    /*
    // send an INIT broadcast
    *icr = (5 << 8 | 1 << 14 | 3 << 18);
    // de-assert INIT
    while (*icr & (1 << 12)) {
    }
    *icr = (5 << 8 | 1 << 15 | 3 << 18);
    // send a SIPI with the trampoline page
    while (*icr & (1 << 12)) {
    }
    *icr =
        ((uint8_t)((uint32_t)trampoline / 0x1000) | 6 << 8 | 1 << 14 | 3 << 18);
    while (*icr & (1 << 12)) {
    }
      */

    pub unsafe fn boot_cpu(&mut self, id: u32, trampoline: usize) {
        assert!(trampoline.trailing_zeros() >= 12);
        assert!((trampoline >> 12) <= 255);
        let id_mask = (id as u64) << 32;
        self.write64(0x30, 5 << 8 | 1 << 14 | id_mask);
        while (self.read(0x30) & (1 << 12)) != 0 {
            _mm_pause();
        }
        self.write64(0x30, 5 << 8 | 1 << 15 | id_mask);
        while (self.read(0x30) & (1 << 12)) != 0 {
            _mm_pause();
        }
        self.write64(
            0x30,
            ((trampoline as u64) / 0x1000) | 6 << 8 | 1 << 14 | id_mask,
        );
    }
}

#[core_local]
pub static LAPIC: RefCell<LocalApic> = RefCell::new(LocalApic(PhantomData));

pub unsafe fn init(bootstrap: bool) {
    // switch to x2APIC
    let val = crate::msr::rdmsr(0x1B);
    crate::msr::wrmsr(0x1B, val | 0b11 << 10);

    if bootstrap {
        const PIC_1: u16 = 0x20;
        const PIC_2: u16 = 0xA0;
        const PIC_1_CMD: u16 = PIC_1;
        const PIC_1_DATA: u16 = PIC_1 + 1;
        const PIC_2_CMD: u16 = PIC_2;
        const PIC_2_DATA: u16 = PIC_2 + 1;

        // remap the PIC
        outb(PIC_1_CMD, 0x11); // begin init
        outb(PIC_2_CMD, 0x11); // begin init
        outb(PIC_1_DATA, 0x20); // irq offset
        outb(PIC_2_DATA, 0x28); // irq offset
        outb(PIC_1_DATA, 1 << 2); // secondary at IRQ 2
        outb(PIC_2_DATA, 2); // secondary ID=2
        outb(PIC_1_DATA, 0x01); // 8086
        outb(PIC_2_DATA, 0x01); // 8086

        // disable the PIC
        outb(PIC_1_DATA, 0xff); // no interrupts
        outb(PIC_2_DATA, 0xff); // no interrupts
    }

    let mut lapic = LAPIC.borrow_mut();
    lapic.set_spurious_interrupt_vector(0xff);
    lapic.set_apic_enabled(false);
    lapic.write(0x3E, 0x3);
    lapic.set_timer(
        TimerConfig::new()
            .with_mask(false)
            .with_vector(0x30)
            .with_mode(TimerMode::Periodic),
    );

    lapic.write(0x38, 0x1000000);
}
