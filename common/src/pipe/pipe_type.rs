#![allow(unused)]
extern crate alloc;
use alloc::{boxed::Box, vec};

use crate::impl_tag_enum;
use crate::message::traits::{Error, Tag, VariableMsg};

impl_tag_enum! {
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PipeType {
        Test = 0,
        File = 1,
        // More pipe types
    }
}

pub trait PipeProtocol {
    const TYPE: PipeType;
    type Header: VariableMsg;
}

pub fn encode_initialization<P: PipeProtocol>(header: &P::Header) -> Result<Box<[u8]>, Error> {
    let mut buffer = vec![0u8; PipeType::SIZE + header.encoded_len()];
    P::TYPE.write_tag(&mut buffer)?;
    header.encode(&mut buffer[PipeType::SIZE..])?;
    Ok(buffer.into_boxed_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::traits::VariableMsg;

    struct TestHeader(u8);
    impl VariableMsg for TestHeader {
        fn encoded_len(&self) -> usize {
            1
        }
        fn encode(&self, out: &mut [u8]) -> Result<(), Error> {
            out[0] = self.0;
            Ok(())
        }
        fn decode(input: &[u8]) -> Result<Self, Error> {
            Ok(TestHeader(*input.first().ok_or(Error::ParserError)?))
        }
    }

    struct TestPipe;
    impl PipeProtocol for TestPipe {
        const TYPE: PipeType = PipeType::Test;
        type Header = TestHeader;
    }

    #[test]
    fn initialization_round_trip() {
        let payload = encode_initialization::<TestPipe>(&TestHeader(0xAB)).unwrap();
        let (pipe_type, rest) = PipeType::extract_tag(&payload).unwrap();
        assert_eq!(pipe_type, PipeType::Test);
        assert_eq!(TestHeader::decode(rest).unwrap().0, 0xAB);
    }

    #[test]
    fn unknown_tag() {
        let payload = [0xFFu8, 0xFF, 0x00];
        assert_eq!(PipeType::extract_tag(&payload), Err(Error::ParserError));
    }

    #[test]
    fn empty_payload() {
        assert_eq!(PipeType::extract_tag(&[]), Err(Error::ParserError));
    }
}
