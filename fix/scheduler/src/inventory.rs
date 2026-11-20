use crate::{
    shell::{Value, fields},
    storage,
};
use alloc::{collections::BTreeSet, vec::Vec};
use fixwasm::{DataType, Error, Value as AsValue, ValueType};

pub struct Inventory {
    pub required: Vec<([u8; 32], u64)>,
    pub local: BTreeSet<[u8; 32]>,
}

pub fn inventory(input: &Value) -> Result<Inventory, Error> {
    let mut pending = alloc::vec![(input.clone(), true)];
    let mut seen = BTreeSet::new();
    let mut result = Inventory {
        required: Vec::new(),
        local: BTreeSet::new(),
    };
    while let Some((value, root)) = pending.pop() {
        if !root && value.value_type() != ValueType::Object {
            continue;
        }
        let raw = value.raw_handle_bytes()?;
        if raw[31] & 0xe0 == 0 {
            continue;
        }
        let available = storage::identify(&value)?;
        let value = match &available {
            Some(object) => object.clone(),
            None if matches!(value.value_type(), ValueType::Object | ValueType::Ref) => value,
            None => continue,
        };
        let canonical = storage::canonical_value(&value)?;
        let mut name = canonical.raw_handle_bytes()?;
        name[31] &= 0xe0;
        if name[31] == 0 || !seen.insert(name) {
            continue;
        }
        let size = byte_size(&value)?;
        result.required.push((name, size));
        if available.is_some() {
            result.local.insert(name);
            if value.data_type()? == DataType::Tree {
                pending.extend(fields(&value)?.into_iter().map(|child| (child, false)));
            }
        }
    }
    Ok(result)
}

pub fn byte_size(value: &Value) -> Result<u64, Error> {
    let bytes = value.raw_handle_bytes()?;
    fixhandle::Handle::parse_ref(&bytes)
        .map(|handle| handle.byte_len())
        .map_err(|_| Error::WrongType)
}
