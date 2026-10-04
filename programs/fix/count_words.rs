#![no_std]
extern crate alloc;

use fix::{Blob, Error, Handle, Object, Tree, Value};

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[fix::apply]
pub fn apply(
    combination: &Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Object<Blob>>, Error> {
    if combination.len()? != 2 {
        return Ok(Blob::create(u64::MAX.to_le_bytes()));
    }
    let tuple = Handle::<Object<Tree>, _>::try_from(combination.get(1))?;
    if tuple.len()? != 2 {
        return Ok(Blob::create(u64::MAX.to_le_bytes()));
    }
    let needle = Handle::<Object<Blob>, _>::try_from(tuple.get(0))?.read()?;
    let haystack = Handle::<Object<Blob>, _>::try_from(tuple.get(1))?.read()?;
    let count = if needle.is_empty() {
        0
    } else {
        haystack
            .windows(needle.len())
            .filter(|&window| window == needle)
            .count()
    };
    Ok(Blob::create((count as u64).to_le_bytes()))
}
