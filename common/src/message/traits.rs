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
