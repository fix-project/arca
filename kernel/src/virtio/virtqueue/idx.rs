#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[repr(transparent)]
pub struct DescriptorIndex(u16);

impl DescriptorIndex {
    pub unsafe fn from(&self, idx: u16) -> Self {
        Self(idx)
    }

    pub fn get(&self) -> u16 {
        self.0
    }
}

impl From<u16> for DescriptorIndex {
    fn from(value: u16) -> Self {
        DescriptorIndex(value)
    }
}

impl From<u32> for DescriptorIndex {
    fn from(value: u32) -> Self {
        DescriptorIndex(value as u16)
    }
}

impl From<usize> for DescriptorIndex {
    fn from(value: usize) -> Self {
        DescriptorIndex(value as u16)
    }
}

impl From<DescriptorIndex> for u16 {
    fn from(value: DescriptorIndex) -> Self {
        value.0
    }
}

impl From<DescriptorIndex> for u32 {
    fn from(value: DescriptorIndex) -> Self {
        value.0 as u32
    }
}

impl From<DescriptorIndex> for usize {
    fn from(value: DescriptorIndex) -> Self {
        value.0 as usize
    }
}

impl AsRef<u16> for DescriptorIndex {
    fn as_ref(&self) -> &u16 {
        &self.0
    }
}
