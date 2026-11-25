use crate::abi::{coordinate, coordinated, effect, request};
pub use crate::acquisition::{acquire_from, add_sources, import_source, sources};
pub use crate::inventory::{Inventory, byte_size, inventory};
use crate::shell::{Value, capture, child};
use alloc::{collections::BTreeMap, string::String};
use core::cell::RefCell;
use fixwasm::{Blob, Error, Value as AsValue, tree};

struct Identified(RefCell<BTreeMap<[u8; 32], Value>>);
// A scheduler Wasm instance is accessed by one kernel thread. Successful
// identification is immutable; misses must be retried after data arrives.
unsafe impl Sync for Identified {}
static IDENTIFIED: Identified = Identified(RefCell::new(BTreeMap::new()));

pub(crate) fn clear_identified() {
    IDENTIFIED.0.borrow_mut().clear();
}

pub fn cache(value: &Value, completed: bool) -> Result<Value, Error> {
    coordinated(
        b"storage_cache",
        &[value.clone(), capture(u64::from(completed))?],
    )
}
pub fn recommend(value: &Value) -> Result<Value, Error> {
    coordinated(b"storage_cache", &[value.clone(), capture(2u64)?])
}
pub fn put(value: &Value) -> Result<Value, Error> {
    effect(b"storage_put", core::slice::from_ref(value))
}
pub fn get_todo(value: &Value) -> Result<Option<Value>, Error> {
    let (status, value) = request(tree![Blob::create(b"storage_get_todo"), value])?;
    match status {
        0 => Ok(Some(value)),
        1 => Ok(None),
        _ => panic!("todo metadata read failed"),
    }
}
pub fn set_todo(value: &Value, metadata: &Value) -> Result<(), Error> {
    effect(b"storage_set_todo", &[value.clone(), metadata.clone()])?;
    Ok(())
}
/// Recover a Thunk from a Thunk or Encode without loading its operand.
pub fn thunk(value: &Value) -> Result<Value, Error> {
    coordinated(b"storage_thunk", core::slice::from_ref(value))
}

/// Return the underlying data Ref without loading its backing.
pub fn reference(value: &Value) -> Result<Value, Error> {
    coordinated(b"storage_reference", core::slice::from_ref(value))
}

/// Construct an opaque canonical Ref without accessing backing data.
pub fn reference_named(name: [u8; 32]) -> Result<Value, Error> {
    coordinated(b"storage_reference_named", &[capture(Blob::create(name))?])
}

/// Describe a value using labels, Fix notation, and unambiguous canonical prefixes.
pub fn friendly_name(value: &Value) -> Result<String, Error> {
    let name = coordinated(b"storage_friendly_name", core::slice::from_ref(value))?;
    let mut bytes = alloc::vec![0; usize::try_from(name.len()?)?];
    name.read_into(0, &mut bytes)?;
    String::from_utf8(bytes).map_err(|_| Error::WrongType)
}

pub fn identify(value: &Value) -> Result<Option<Value>, Error> {
    let key = value.raw_handle_bytes()?;
    if let Some(object) = IDENTIFIED.0.borrow().get(&key) {
        return Ok(Some(object.clone()));
    }
    match coordinate(b"storage_identify", core::slice::from_ref(value))? {
        (0, object) => {
            IDENTIFIED.0.borrow_mut().insert(key, object.clone());
            Ok(Some(object))
        }
        (1, _) => Ok(None),
        _ => Err(Error::WrongType),
    }
}

pub fn canonicalize(value: &Value) -> Result<Value, Error> {
    effect(b"storage_canonicalize", core::slice::from_ref(value))
}

pub fn canonical_value(value: &Value) -> Result<Value, Error> {
    let coupon = canonicalize(value)?;
    let assertion = identify(&child(&coupon, 1)?)?.ok_or(Error::OutOfBounds)?;
    child(&assertion, 2)
}

pub(crate) fn claims(operation: u64, input: &Value) -> Result<Value, Error> {
    coordinated(b"storage_claims", &[capture(operation)?, input.clone()])
}

/// Select a value without creating a coupon.
pub fn selection(input: &Value) -> Result<Value, Error> {
    effect(b"storage_selection", core::slice::from_ref(input))
}

/// Request a Select coupon for an evaluated combination.
pub fn select(input: &Value) -> Result<Value, Error> {
    effect(b"storage_select", core::slice::from_ref(input))
}

/// Request an identity Eval coupon for a value.
pub fn eval(input: &Value) -> Result<Value, Error> {
    coordinated(b"storage_eval", core::slice::from_ref(input))
}
