#![allow(unused)]
use bitfield_struct::bitfield;
use core::{cell::RefCell, marker::PhantomData};

use crate::msr::{rdmsr, wrmsr};

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

#[repr(u8)]
#[derive(Debug)]
enum TimerDivider {
    Two = 0,
    Four = 1,
    Eight = 2,
    Sixteen = 3,
    ThirtyTwo = 4,
    SixtyFour = 5,
    OneHundredTwentyEight = 6,
    One = 7,
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

    fn set_divide_configuration(&mut self, value: TimerDivider) {
        self.write(0x3E, value as u32);
    }

    fn set_initial_count(&mut self, value: u32) {
        self.write(0x38, value);
    }

    fn get_current_count(&self) -> u32 {
        self.read(0x39)
    }

    fn get_errors(&self) -> u32 {
        self.write(0x28, 0);
        self.read(0x28)
    }

    pub(crate) unsafe fn clear_interrupt(&mut self) {
        self.write(0xB, 0);
    }
}

#[core_local]
pub static LAPIC: RefCell<LocalApic> = RefCell::new(LocalApic(PhantomData));

pub unsafe fn init() {
    // switch to x2APIC and enable LAPIC
    // https://courses.cs.washington.edu/courses/cse451/21sp/readings/x2apic.pdf
    // - section 2.2 (p2-2)
    let val = crate::msr::rdmsr(0x1B);
    crate::msr::wrmsr(0x1B, val | (0b11 << 10));

    let mut lapic = LAPIC.borrow_mut();

    lapic.set_spurious_interrupt_vector(0xff);
    lapic.set_apic_enabled(true);
    lapic.set_divide_configuration(TimerDivider::One);
    lapic.set_timer(
        TimerConfig::new()
            .with_mask(false)
            .with_vector(0x20)
            .with_mode(TimerMode::Periodic),
    );

    lapic.set_initial_count(0x10000); // 1ms

    lapic.set_initial_count(0x40); // 1us
}
