use alloc::{collections::BTreeMap, vec::Vec};
use arca::Value;
use user::Runtime;

use crate::handles::{self, Query, Reference};
use crate::rt;

struct LocalObject {
    backing: Value<Runtime>,
    bytes: usize,
    promotion: usize,
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
) {
    unsafe {
        assert!(
            (&mut *(&raw mut OBJECTS))
                .insert(
                    address,
                    LocalObject {
                        backing,
                        bytes,
                        promotion,
                    },
                )
                .is_none()
        );
        handles::create_local(handles::address(result), address, bytes, metadata);
        crate::descriptors::local(handles::address(result));
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
