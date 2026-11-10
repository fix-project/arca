extern crate alloc;

use alloc::collections::BTreeSet;

/// Allocation state for a dense table of 32-byte Fix handles.
/// Closed entries in the backing table must be cleared before reuse.
#[derive(Default)]
pub struct Descriptors {
    len: u32,
    free: BTreeSet<u32>,
}

impl Descriptors {
    pub const fn new() -> Self {
        Self {
            len: 0,
            free: BTreeSet::new(),
        }
    }

    pub fn allocate(&mut self, capacity: u32) -> Option<u32> {
        if let Some(index) = self.free.pop_first() {
            return Some(index);
        }
        if self.len >= capacity {
            return None;
        }
        let index = self.len;
        self.len += 1;
        Some(index)
    }

    pub fn contains(&self, index: u32) -> bool {
        index < self.len && !self.free.contains(&index)
    }

    pub fn close(&mut self, index: u32) -> bool {
        index < self.len && self.free.insert(index)
    }

    pub fn len(&self) -> u32 {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_reuses_lowest_closed_slot() {
        let mut table = Descriptors::new();
        for index in 0..8 {
            assert_eq!(table.allocate(8), Some(index));
        }
        assert_eq!(table.allocate(8), None);
        for index in [5, 1, 7, 3] {
            assert!(table.close(index));
        }
        assert!(!table.close(1));
        assert!(!table.close(8));
        for index in [1, 3, 5, 7] {
            assert!(!table.contains(index));
            assert_eq!(table.allocate(8), Some(index));
            assert!(table.contains(index));
        }
        assert_eq!(table.len(), 8);
        assert_eq!(table.allocate(8), None);
    }
}
