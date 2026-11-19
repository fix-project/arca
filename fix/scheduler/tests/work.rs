#![no_std]
use fixwasm::{Blob, Error, Handle, Object, Tree, Value};

#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<impl Value, Error> {
    let left = Handle::<Object<Blob>>::try_from(combination.get(2))?.read_u64()?;
    let right = Handle::<Object<Blob>>::try_from(combination.get(3))?.read_u64()?;
    let mut result = left.wrapping_add(right);
    for _ in 0..20_000_000 {
        result = core::hint::black_box(result)
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
    }
    Ok(Blob::create(result.to_le_bytes()))
}
