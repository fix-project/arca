#![no_std]
use fixwasm::{
    Error,
    backend::{Blob, Tree},
};

#[fixwasm::main(combination)]
pub fn apply(combination: &Tree, value: Blob) -> Result<Blob, Error> {
    combination.as_value().advise("test::marker", b"seen");
    Ok(value)
}
