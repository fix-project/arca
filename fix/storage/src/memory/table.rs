use alloc::boxed::Box;
use core::{
    cell::UnsafeCell,
    hash::{BuildHasher, Hash},
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
};
use hashbrown::{DefaultHashBuilder, HashMap};

use crate::platform::Mutex;

const SHARDS: usize = 256;
const FIRST_SEGMENT: usize = 1024;
const SEGMENTS: usize = usize::BITS as usize - FIRST_SEGMENT.trailing_zeros() as usize;

#[repr(align(128))]
struct Padded<T>(T);

struct Slot<T> {
    ready: AtomicBool,
    value: UnsafeCell<MaybeUninit<T>>,
}

/// Write-once slots addressed by index.
pub(super) struct Table<T> {
    segments: [AtomicPtr<Slot<T>>; SEGMENTS],
    /// Held while creating a segment, so each segment is allocated once.
    growing: Mutex<()>,
}

unsafe impl<T: Send> Send for Table<T> {}
unsafe impl<T: Send + Sync> Sync for Table<T> {}

fn locate(index: usize) -> (usize, usize) {
    let position = index / FIRST_SEGMENT + 1;
    let segment = (usize::BITS - 1 - position.leading_zeros()) as usize;
    (segment, index - FIRST_SEGMENT * ((1 << segment) - 1))
}

fn segment_len(segment: usize) -> usize {
    FIRST_SEGMENT << segment
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Self {
            segments: [const { AtomicPtr::new(core::ptr::null_mut()) }; SEGMENTS],
            growing: Mutex::new(()),
        }
    }
}

impl<T> Table<T> {
    fn slot(&self, index: usize) -> Option<&Slot<T>> {
        let (segment, offset) = locate(index);
        let slots = self.segments[segment].load(Ordering::Acquire);
        (!slots.is_null()).then(|| unsafe { &*slots.add(offset) })
    }

    fn slot_or_create(&self, index: usize) -> &Slot<T> {
        let (segment, offset) = locate(index);
        let mut slots = self.segments[segment].load(Ordering::Acquire);
        if slots.is_null() {
            let _growing = self.growing.lock();
            slots = self.segments[segment].load(Ordering::Acquire);
            if slots.is_null() {
                slots = Box::into_raw(
                    (0..segment_len(segment))
                        .map(|_| Slot {
                            ready: AtomicBool::new(false),
                            value: UnsafeCell::new(MaybeUninit::<T>::uninit()),
                        })
                        .collect::<Box<[Slot<T>]>>(),
                ) as *mut Slot<T>;
                self.segments[segment].store(slots, Ordering::Release);
            }
        }
        unsafe { &*slots.add(offset) }
    }

    /// Each index must be inserted at most once.
    pub(super) fn insert(&self, index: usize, value: T) {
        let slot = self.slot_or_create(index);
        assert!(!slot.ready.load(Ordering::Relaxed), "slot already filled");
        unsafe { (*slot.value.get()).write(value) };
        slot.ready.store(true, Ordering::Release);
    }

    pub(super) fn get<R>(&self, index: usize, read: impl FnOnce(&T) -> R) -> Option<R> {
        let slot = self.slot(index)?;
        slot.ready
            .load(Ordering::Acquire)
            .then(|| read(unsafe { (*slot.value.get()).assume_init_ref() }))
    }

    /// Visits every filled slot below `end`.
    pub(super) fn for_each(&self, end: usize, mut visit: impl FnMut(&T)) {
        for index in 0..end {
            self.get(index, &mut visit);
        }
    }
}

impl<T> Drop for Table<T> {
    fn drop(&mut self) {
        for (segment, slots) in self.segments.iter_mut().enumerate() {
            let slots = *slots.get_mut();
            if slots.is_null() {
                continue;
            }
            let mut slots = unsafe {
                Box::from_raw(core::ptr::slice_from_raw_parts_mut(
                    slots,
                    segment_len(segment),
                ))
            };
            for slot in slots.iter_mut() {
                if *slot.ready.get_mut() {
                    unsafe { slot.value.get_mut().assume_init_drop() };
                }
            }
        }
    }
}

impl<T> core::fmt::Debug for Table<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Table")
    }
}

type Shard<K, V> = Padded<Mutex<HashMap<K, V>>>;

/// A hash map split into independently locked shards.
pub struct ShardedMap<K, V> {
    hasher: DefaultHashBuilder,
    shards: Box<[Shard<K, V>]>,
}

impl<K: Hash + Eq, V> Default for ShardedMap<K, V> {
    fn default() -> Self {
        Self {
            hasher: DefaultHashBuilder::default(),
            shards: (0..SHARDS)
                .map(|_| Padded(Mutex::new(HashMap::new())))
                .collect(),
        }
    }
}

impl<K: Hash + Eq, V> ShardedMap<K, V> {
    pub fn with<Q: Hash + ?Sized, R>(&self, key: &Q, f: impl FnOnce(&mut HashMap<K, V>) -> R) -> R
    where
        K: core::borrow::Borrow<Q>,
    {
        let shard = self.hasher.hash_one(key) as usize % SHARDS;
        f(&mut self.shards[shard].0.lock())
    }
}

impl<K, V> core::fmt::Debug for ShardedMap<K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ShardedMap")
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use core::sync::atomic::AtomicUsize;
    use std::{sync::Arc, thread, vec::Vec};

    #[test]
    fn concurrent_inserts_are_visible_to_every_reader() {
        let table = Arc::new(Table::<Vec<usize>>::default());
        let next = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let (table, next) = (table.clone(), next.clone());
                thread::spawn(move || {
                    for _ in 0..5000 {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        table.insert(index, alloc::vec![index; 4]);
                        let read = table.get(index, |value| value.clone());
                        assert_eq!(read, Some(alloc::vec![index; 4]));
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let end = next.load(Ordering::Relaxed);
        let mut seen = 0;
        table.for_each(end, |value| {
            assert_eq!(value[0], seen);
            seen += 1;
        });
        assert_eq!(seen, end);
    }
}
