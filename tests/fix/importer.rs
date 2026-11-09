#![no_std]
extern crate alloc;

use fix::{Blob, Error, Handle, Object, Tree, Value};

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[fix::apply]
fn apply(combination: &Handle<Object<Tree>, fix::Focus>) -> Result<impl Value, Error> {
    let uri = Handle::<Object<Blob>, _>::try_from(combination.get(2))?;
    let mut bytes = alloc::vec![0; usize::try_from(uri.len()?)?];
    uri.read_into(0, &mut bytes)?;
    let uri = core::str::from_utf8(&bytes).map_err(|_| Error::WrongType)?;
    fix::importers::directory(uri)?.ok_or(Error::OutOfBounds)
}
