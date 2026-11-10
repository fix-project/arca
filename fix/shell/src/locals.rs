use alloc::{collections::BTreeMap, vec::Vec};
use arca::Value;
use user::Runtime;

use crate::handles::{self, Query, Reference};
use crate::rt;

static mut OBJECTS: BTreeMap<usize, Value<Runtime>> = BTreeMap::new();
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
) {
    unsafe {
        assert!(
            (&mut *(&raw mut OBJECTS))
                .insert(address, backing)
                .is_none()
        );
        handles::create_local(handles::address(result), address, bytes, metadata);
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
        return;
    }
    use crate::continuation::Argument::{Tuple, Value, Word};
    let records = Tuple(
        objects
            .into_iter()
            .map(|(address, backing)| Tuple(alloc::vec![Word(address as u64), Value(backing)]))
            .collect(),
    );
    let (address, length) = crate::descriptors::range();
    crate::continuation::operation("lift")
        .argument(records)
        .word(address as u64)
        .word(length as u64)
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
}
