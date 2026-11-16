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
    let mut pending = alloc::vec![input.clone()];
    let mut seen = BTreeSet::new();
    let mut result = Inventory {
        required: Vec::new(),
        local: BTreeSet::new(),
    };
    while let Some(value) = pending.pop() {
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
                pending.extend(fields(&value)?);
            }
        }
    }
    Ok(result)
}

pub fn byte_size(value: &Value) -> Result<u64, Error> {
    Ok(value
        .len()?
        .saturating_mul(if value.data_type()? == DataType::Tree {
            32
        } else {
            1
        }))
}
