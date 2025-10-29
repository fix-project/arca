use core::sync::atomic::{AtomicPtr, AtomicU64, AtomicU8, Ordering};

extern crate alloc;
use alloc::boxed::Box;

struct SyncBox<T> {
    ptr: AtomicPtr<T>,
}

impl<T> SyncBox<T> {
    pub const fn new() -> Self {
        SyncBox {
            ptr: AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    pub fn take(&self) -> Box<T> {
        loop {
            if let Some(result) = self.try_take() {
                return result;
            }
            core::hint::spin_loop();
        }
    }

    pub fn put(&self, value: Box<T>) {
        let mut value = value;
        loop {
            let Err(result) = self.try_put(value) else {
                return;
            };
            value = result;
            core::hint::spin_loop();
        }
    }

    pub fn try_take(&self) -> Option<Box<T>> {
        unsafe {
            Some(Box::from_raw(
                self.ptr
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
                        if old.is_null() {
                            None
                        } else {
                            Some(core::ptr::null_mut())
                        }
                    })
                    .ok()?,
            ))
        }
    }

    pub fn try_put(&self, value: Box<T>) -> Result<(), Box<T>> {
        let ptr = Box::into_raw(value);
        unsafe {
            self.ptr
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
                    if old.is_null() {
                        Some(ptr)
                    } else {
                        None
                    }
                })
                .map_err(|p| Box::from_raw(p))?;
            Ok(())
        }
    }
}

#[allow(non_snake_case)]
pub mod Mode {
    pub const EMPTY: u8 = 0;
    pub const LEAF: u8 = 1;
    pub const INNER: u8 = 2;
    pub const WAIT: u8 = !0;
}

struct Retry;
struct RetryWith<V>(pub V);

pub struct Trie<const N: usize, V> {
    mode: AtomicU8,
    key: AtomicU64,
    value: SyncBox<V>,
    children: [SyncBox<Trie<N, V>>; N],
}

impl<const N: usize, V> Trie<N, V> {
    pub fn new() -> Self {
        Trie {
            mode: AtomicU8::new(0),
            key: AtomicU64::new(!0),
            value: SyncBox::new(),
            children: [const { SyncBox::new() }; N],
        }
    }

    fn try_insert(&self, key: u64, value: V) -> Result<Option<V>, RetryWith<V>> {
        let new_key = key;
        let new_value = value;
        let mode = self.mode.load(Ordering::SeqCst);
        match mode {
            Mode::EMPTY => {
                if self
                    .mode
                    .compare_exchange(Mode::EMPTY, Mode::LEAF, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
                {
                    return Err(RetryWith(new_value));
                }
                self.key.store(key, Ordering::SeqCst);
                self.value.put(Box::new(new_value));
                Ok(None)
            }
            Mode::LEAF => {
                let Some(value) = self.value.try_take() else {
                    return Err(RetryWith(new_value));
                };
                // We have ownership of this node (and its value).
                let key = self.key.load(Ordering::SeqCst);
                if key == new_key {
                    // Just replace the value.
                    self.value.put(Box::new(new_value));
                    Ok(Some(*value))
                } else {
                    // We've got to upgrade this to an inner node.
                    if self
                        .mode
                        .compare_exchange(
                            Mode::LEAF,
                            Mode::INNER,
                            Ordering::SeqCst,
                            Ordering::SeqCst,
                        )
                        .is_err()
                    {
                        unreachable!();
                    }
                    self.key.store(!0, Ordering::SeqCst);

                    let children: [Box<Self>; N] = core::array::from_fn(|_| Box::new(Trie::new()));
                    children[key as usize % N].insert(key / N as u64, *value);
                    children[new_key as usize % N].insert(new_key / N as u64, new_value);

                    // Writing to `zero` and `one` will unblock anyone trying to use this as an
                    // inner node.
                    for (i, child) in children.into_iter().enumerate() {
                        self.children[i].put(child);
                    }
                    Ok(None)
                }
            }
            Mode::INNER => {
                // We're in an inner node.  It's possible another thread is initializing this node,
                // or that someone is trying to downgrade it (or both!).
                let child = &self.children[new_key as usize % N];
                    let Some(old) = child.try_take() else {
                        return Err(RetryWith(new_value));
                    };
                    // We got ownership of this value!
                    let replaced = old.try_insert(new_key / N as u64, new_value);
                    // Returning ownership will unblock others.
                    child.put(old);
                    replaced
            }
            _ => unreachable!(),
        }
    }

    pub fn insert(&self, key: u64, value: V) -> Option<V> {
        let mut value = value;
        loop {
            match self.try_insert(key, value) {
                Ok(result) => return result,
                Err(RetryWith(v)) => value = v,
            }
            core::hint::spin_loop();
        }
    }

    fn try_remove(&self, key: u64) -> Result<Option<V>, Retry> {
        let target_key = key;
        let mode = self.mode.load(Ordering::SeqCst);
        match mode {
            Mode::EMPTY => return Ok(None),
            Mode::LEAF => {
                let Some(value) = self.value.try_take() else {
                    return Err(Retry);
                };
                let key = self.key.load(Ordering::SeqCst);
                if key != target_key {
                    // This doesn't match, replace the value.
                    self.value.put(value);
                    return Ok(None);
                }
                self.key.store(!0, Ordering::SeqCst);
                if self
                    .mode
                    .compare_exchange(Mode::LEAF, Mode::EMPTY, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
                {
                    unreachable!()
                }
                return Ok(Some(*value));
            }
            Mode::INNER => {
                let det = key as usize % N;
                let rest = key / N as u64;
                let cell = &self.children[det];
                let child = cell.take();
                let result = child.try_remove(rest);
                cell.put(child);
                // TODO: we need to compact this
                return result;
            }
            _ => unreachable!(),
        }
    }

    pub fn remove(&self, key: u64) -> Option<V> {
        loop {
            if let Ok(result) = self.try_remove(key) {
                return result;
            }
            core::hint::spin_loop();
        }
    }

    fn try_first_key(&self) -> Result<Option<u64>, Retry> {
        let mode = self.mode.load(Ordering::SeqCst);
        match mode {
            Mode::EMPTY => Ok(None),
            Mode::LEAF => Ok(Some(self.key.load(Ordering::SeqCst))),
            Mode::INNER => {
                for i in 0..N {
                    let cell = &self.children[i];
                    let Some(child) = cell.try_take() else {
                        return Err(Retry);
                    };
                    let result = loop {
                        if let Ok(result) = child.try_first_key() {
                            break result;
                        }
                        core::hint::spin_loop();
                    };
                    cell.put(child);
                    if let Some(key) = result {
                        return Ok(Some(key * N as u64 + i as u64));
                    }
                }
                Ok(None)
            }
            _ => unreachable!(),
        }
    }

    pub fn first_key(&self) -> Option<u64> {
        loop {
            if let Ok(result) = self.try_first_key() {
                return result;
            }
            core::hint::spin_loop();
        }
    }
}

impl<const N: usize, V> Default for Trie<N, V> {
    fn default() -> Self {
        Self::new()
    }
}

pub type BinaryTrie<V> = Trie<2, V>;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rand::Rng;

    use super::*;

    #[test]
    fn test_insert_remove() {
        let trie = BinaryTrie::new();
        trie.insert(1, "one");
        trie.insert(2, "two");
        trie.insert(3, "three");
        assert_eq!(trie.remove(1), Some("one"));
        trie.insert(4, "for");
        trie.insert(5, "five");
        trie.insert(4, "four");
        assert_eq!(trie.remove(5), Some("five"));
        assert_eq!(trie.remove(4), Some("four"));
        assert_eq!(trie.remove(4), None);
        assert_eq!(trie.remove(2), Some("two"));
        assert_eq!(trie.remove(3), Some("three"));
    }

    #[test]
    fn test_parallel() {
        use std::sync::Arc;
        let trie = Arc::new(BinaryTrie::new());
        std::thread::scope(|s| {
            for t in 0..32 {
                let trie = trie.clone();
                s.spawn(move || {
                    let mut rng = rand::rng();
                    for i in 0..256 {
                        let key = rng.random_range(0..256);
                        let key = 256 * key + t;
                        let value: u128 = rng.random();
                        let delay = rng.random_range(0..65536);
                        let delay = Duration::from_nanos(delay);

                        let result = trie.insert(key, value);
                        assert!(result.is_none());

                        std::thread::sleep(delay);
                        let result = trie.remove(key);

                        assert_eq!((Some(t), Some(i), result), (Some(t), Some(i), Some(value)));
                    }
                });
            }
        });
    }
}
