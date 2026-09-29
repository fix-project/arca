use core::ops::{Index, IndexMut};

use crate::paging::*;

#[allow(unreachable_code)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Impossible(!);

unsafe impl HardwarePageTable for Impossible {
    type Entry = Impossible;
    type Parent = Impossible;
    const SIZE: usize = 0;

    fn new() -> UniquePage<Self> {
        unreachable!();
    }
}

impl Index<usize> for Impossible {
    type Output = Impossible;

    fn index(&self, _: usize) -> &Self::Output {
        unreachable!();
    }
}

impl IndexMut<usize> for Impossible {
    fn index_mut(&mut self, _: usize) -> &mut Self::Output {
        unreachable!();
    }
}

impl HardwarePage for Impossible {
    type Parent = Impossible;
    const SIZE: usize = 0;
}

impl HardwarePageTableEntry for Impossible {
    type Page = Impossible;

    type Table = Impossible;

    type PageDescriptor = PageDescriptor;

    type TableDescriptor = TableDescriptor;

    fn bits(&self) -> u64 {
        unreachable!()
    }

    unsafe fn set_bits(&mut self, _: u64) {
        unreachable!()
    }

    unsafe fn new(_: u64) -> Self {
        unreachable!()
    }
}
