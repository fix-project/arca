#[allow(deprecated)]
use core::hash::{Hash, Hasher, SipHasher};

use alloc::boxed::Box;
use user::{io::Monitor, prelude::*};

pub struct KVStore {
    cells: Box<[Monitor]>,
}

impl KVStore {
    pub fn new(n: usize) -> Self {
        let mut cells = Box::new_uninit_slice(n);
        for cell in cells.iter_mut() {
            cell.write(Monitor::new());
        }
        let cells = unsafe { cells.assume_init() };
        Self { cells }
    }

    fn index(&self, key: &[u8]) -> usize {
        #[allow(deprecated)]
        let mut hasher = SipHasher::new();
        key.hash(&mut hasher);
        hasher.finish() as usize % self.cells.len()
    }

    pub fn insert(&self, key: &[u8], value: &[u8], meta: u16) {
        let i = self.index(&key);
        let m = &self.cells[i];
        m.enter(|cell| {
            let mut replacement = Tuple::new(3);
            let key = Blob::from(key);
            let value = Blob::from(value);
            replacement.set(0, key);
            replacement.set(1, value);
            replacement.set(2, meta as u64);
            cell.set(replacement)
        });
    }

    pub fn lookup(&self, target: &[u8]) -> Option<(Blob, u16)> {
        let i = self.index(&target);
        let m = &self.cells[i];
        let result = m.enter(|cell| {
            let _: Result<(), Value> = try {
                if let Value::Tuple(tuple) = cell.get() {
                    let key: Blob = tuple.get(0).try_into()?;
                    if key.with_ref(|key| key == target) {
                        return tuple.into();
                    }
                };
            };
            return Null::new().into();
        });
        match result {
            Value::Tuple(tuple) => {
                let value: Blob = tuple.get(1).try_into().ok()?;
                let meta: Word = tuple.get(2).try_into().ok()?;
                return Some((value, meta.read() as u16));
            }
            _ => return None,
        }
    }
}
