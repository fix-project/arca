use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};
use arca::Value;
use user::Runtime;

use crate::handles::{self, Query, Reference};
use crate::rt;

struct LocalObject {
    backing: Value<Runtime>,
    bytes: usize,
    promotion: usize,
    children: BTreeSet<usize>,
}

static mut OBJECTS: BTreeMap<usize, LocalObject> = BTreeMap::new();
static mut REGIONS: usize = 0;

pub fn region() -> usize {
    let capacity = rt::LOCAL_REGION_COUNT;
    if unsafe { REGIONS } == capacity {
        lift_all();
    }
    unsafe {
        let address = rt::LOCAL_BASE + REGIONS * rt::SLOT_SIZE;
        REGIONS += 1;
        address
    }
}

pub unsafe fn create(
    result: Reference,
    backing: Value<Runtime>,
    address: usize,
    bytes: usize,
    metadata: u16,
    promotion: usize,
    children: BTreeSet<usize>,
) {
    unsafe {
        handles::create_local(handles::address(result), address, bytes, metadata);
        assert!(
            (&mut *(&raw mut OBJECTS))
                .insert(
                    address,
                    LocalObject {
                        backing,
                        bytes,
                        promotion,
                        children,
                    },
                )
                .is_none()
        );
        crate::descriptors::local(handles::address(result));
    }
}

pub fn local_address(cell: usize) -> Option<usize> {
    unsafe {
        (handles::query_cell(cell, Query::Local) != 0)
            .then(|| handles::query_cell(cell, Query::LocalAddress))
    }
}

pub fn backing(address: usize) -> Value<Runtime> {
    unsafe { (&*(&raw const OBJECTS))[&address].backing.clone() }
}

pub fn release_unreachable() {
    let mut pending: Vec<_> = crate::descriptors::local_roots().collect();
    let mut live = BTreeSet::new();
    while let Some(address) = pending.pop() {
        if !live.insert(address) {
            continue;
        }
        let object = unsafe { &(&*(&raw const OBJECTS))[&address] };
        pending.extend(object.children.iter().copied());
    }
    let objects = unsafe { &mut *(&raw mut OBJECTS) };
    let dead: Vec<_> = objects
        .keys()
        .copied()
        .filter(|key| !live.contains(key))
        .collect();
    for address in dead {
        let object = objects.remove(&address).unwrap();
        unsafe {
            rt::map_region(address as *mut u8, rt::SLOT_SIZE, arcane::__MODE_none)
                .expect("local data unmap failed");
        }
        let charge = fixhandle::accounting::object(object.bytes);
        drop(object);
        crate::memory::release(charge);
    }
}

pub fn lift(roots: &[Reference]) {
    if roots
        .iter()
        .any(|root| unsafe { handles::query(*root, Query::Local) } != 0)
    {
        lift_all();
    }
}

pub fn lift_all() {
    let objects = unsafe { core::mem::take(&mut *(&raw mut OBJECTS)) };
    if objects.is_empty() {
        unsafe { REGIONS = 0 };
        crate::descriptors::take_local();
        return;
    }
    use crate::continuation::Argument::{Tuple, Value, Word};
    let mut promotion_bytes = 0;
    let records = Tuple(
        objects
            .into_iter()
            .map(|(address, object)| {
                promotion_bytes += object.promotion;
                Tuple(alloc::vec![
                    Word(address as u64),
                    Value(object.backing),
                    Word(object.bytes as u64)
                ])
            })
            .collect(),
    );
    let slots = crate::descriptors::take_local();
    crate::memory::charge(promotion_bytes);
    crate::continuation::operation("lift")
        .argument(records)
        .blob(&slots)
        .call_with_current_continuation();
    unsafe {
        for index in 0..REGIONS {
            rt::map_region(
                (rt::LOCAL_BASE + index * rt::SLOT_SIZE) as *mut u8,
                rt::SLOT_SIZE,
                arcane::__MODE_none,
            )
            .expect("local data unmap failed");
        }
        REGIONS = 0;
    }
    crate::memory::release(promotion_bytes);
}
