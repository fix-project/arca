#![allow(dead_code)]

use crate::page::UniquePage;

pub use common::BuddyAllocator;

pub type UniquePage4KB = UniquePage<[u8; 1 << 12]>;
pub type UniquePage2MB = UniquePage<[u8; 1 << 21]>;
pub type UniquePage1GB = UniquePage<[u8; 1 << 30]>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_alloc() {
        UniquePage2MB::new();
    }

    #[bench]
    pub fn bench_alloc_free_2mb(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            let _ = UniquePage2MB::new();
        });
    }
}
