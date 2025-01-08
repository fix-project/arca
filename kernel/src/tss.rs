use core::{cell::LazyCell, ptr::addr_of};

use crate::interrupts::INTERRUPT_STACK;

#[core_local]
pub(crate) static mut TSS: LazyCell<TaskStateSegment> =
    LazyCell::new(|| TaskStateSegment::new(**INTERRUPT_STACK));

#[repr(C, packed)]
#[derive(Default)]
pub struct InputOutputBitmap {
    bits: [u8; 32],
    ending: u8,
}

impl InputOutputBitmap {
    pub fn enable(&mut self, port: u8) {
        self.bits[(port / 8) as usize] &= !(1 << (port % 8));
    }

    #[allow(unused)]
    pub fn disable(&mut self, port: u8) {
        self.bits[(port / 8) as usize] |= 1 << (port % 8);
    }
}

#[repr(C, packed)]
#[derive(Default)]
pub struct TaskStateSegment {
    _0: u32,
    rsp: [u64; 3],
    _1: u64,
    ist: [u64; 7],
    _2: u64,
    _3: u16,
    iopb: u16,
    io_bitmap: InputOutputBitmap,
}

const _: () = const {
    assert!(
        core::mem::size_of::<TaskStateSegment>() == 104 + core::mem::size_of::<InputOutputBitmap>()
    );
};

impl TaskStateSegment {
    pub fn new(stack: *mut [u8]) -> TaskStateSegment {
        let mut tss = TaskStateSegment {
            rsp: [
                unsafe { addr_of!((*stack)[0]).add(stack.len()) } as usize as u64,
                0,
                0,
            ],
            iopb: 104,
            io_bitmap: InputOutputBitmap {
                bits: [0xff; 32],
                ending: 0xff,
            },
            ..Default::default()
        };
        // Enable debug console.
        tss.io_bitmap.enable(0xe9);
        tss
    }
}
