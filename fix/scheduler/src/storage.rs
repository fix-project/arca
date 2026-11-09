use crate::abi::{coordinate, coordinated, effect, request};
pub use crate::acquisition::{acquire, add_sources, import_source, sources};
use crate::shell::{Root, root};
use fix::coupon::Statement;
use fix::{Blob, Error, tree};

pub fn cache(value: &Root, completed: bool) -> Result<Root, Error> {
    coordinated(
        b"storage_cache",
        &[value.clone(), root(u64::from(completed))?],
    )
}
pub fn recommend(value: &Root) -> Result<Root, Error> {
    coordinated(b"storage_cache", &[value.clone(), root(2u64)?])
}
pub fn put(value: &Root) -> Result<Root, Error> {
    effect(b"storage_put", core::slice::from_ref(value))
}
pub fn get_todo(value: &Root) -> Result<Option<Root>, Error> {
    let (status, value) = request(tree![Blob::create(b"storage_get_todo"), value])?;
    match status {
        0 => Ok(Some(value)),
        1 => Ok(None),
        _ => panic!("todo metadata read failed"),
    }
}
pub fn set_todo(value: &Root, metadata: &Root) -> Result<(), Error> {
    effect(b"storage_set_todo", &[value.clone(), metadata.clone()])?;
    Ok(())
}
pub fn identify(value: &Root) -> Result<Option<Root>, Error> {
    match coordinate(b"storage_identify", core::slice::from_ref(value))? {
        (0, object) => Ok(Some(object)),
        (1, _) => Ok(None),
        _ => Err(Error::WrongType),
    }
}

pub fn canonicalize(value: &Root) -> Result<Root, Error> {
    effect(b"storage_canonicalize", core::slice::from_ref(value))
}

pub fn claims(statement: Statement, input: &Root) -> Result<Root, Error> {
    coordinated(b"storage_claims", &[root(statement as u64)?, input.clone()])
}

pub fn select(input: &Root) -> Result<Root, Error> {
    effect(b"storage_select", core::slice::from_ref(input))
}

pub fn eval(input: &Root) -> Result<Root, Error> {
    effect(b"storage_eval", core::slice::from_ref(input))
}
