#![cfg_attr(not(feature = "std"), no_std)]

use core::fmt;
use fixwasm::{
    Error,
    backend::{Blob, Tree, Value},
};

struct LongMessage;
impl fmt::Display for LongMessage {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        for _ in 0..70000 {
            output.write_str("x")?;
        }
        Ok(())
    }
}

#[fixwasm::apply]
fn apply(combination: &Tree) -> Result<Value, Error> {
    let mode = Blob::try_from(combination.get(2)?.expect("mode"))?;
    let mut bytes = [0; 8];
    assert_eq!(mode.read(0, &mut bytes)?, bytes.len());
    match u64::from_le_bytes(bytes) {
        0 => panic!("simple panic"),
        1 => panic!("failed λ with value {}", 42),
        2 => panic!("{LongMessage}"),
        3 => Err(Error::WrongType),
        #[cfg(feature = "std")]
        4 => std::panic::panic_any(std::string::String::from("owned panic message")),
        _ => fixwasm::exit(Blob::from("early return")),
    }
}
