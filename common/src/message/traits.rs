extern crate alloc;
use alloc::boxed::Box;
use alloc::vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Failed to parse message
    ParserError,
    /// Failed to serialize message
    SerializerError,
}

pub trait FixedMsg: Sized {
    const SIZE: usize;

    fn encode(&self, out: &mut [u8]) -> Result<(), Error>;
    fn decode(input: &[u8]) -> Result<Self, Error>;
}

pub trait IntoBytes {
    fn into_boxed_slice(self) -> Box<[u8]>;
}

impl<T: FixedMsg> IntoBytes for T {
    fn into_boxed_slice(self: T) -> Box<[u8]> {
        let mut buf = vec![0; T::SIZE];
        self.encode(&mut buf).unwrap();
        buf.into_boxed_slice()
    }
}
