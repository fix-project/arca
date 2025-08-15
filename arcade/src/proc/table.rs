use core::sync::atomic::{AtomicU64, Ordering};

use super::*;
use alloc::{
    collections::btree_map::BTreeMap,
    sync::{Arc, Weak},
};
use kernel::{initcell::LazyLock, prelude::RwLock};

pub static PROCS: LazyLock<ProcTable> = LazyLock::new(Default::default);

#[derive(Default)]
pub struct ProcTable {
    table: Arc<RwLock<BTreeMap<u64, Weak<ProcState>>>>,
    current: AtomicU64,
}

impl ProcTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate(&self, p: &Arc<ProcState>) -> u64 {
        loop {
            let pid = self.current.fetch_add(1, Ordering::Relaxed);
            let mut table = self.table.write();
            if let Some(a) = table.get(&pid)
                && a.strong_count() != 0
            {
                continue;
            }
            table.insert(pid, Arc::downgrade(p));
            return pid;
        }
    }
}
