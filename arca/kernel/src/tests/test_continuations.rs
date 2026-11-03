use crate::prelude::*;
use bytes::Buf;

#[derive(Clone)]
struct Chunks<'a> {
    bytes: &'a [u8],
    width: usize,
}

impl Buf for Chunks<'_> {
    fn remaining(&self) -> usize {
        self.bytes.len()
    }

    fn chunk(&self) -> &[u8] {
        &self.bytes[..self.bytes.len().min(self.width)]
    }

    fn advance(&mut self, count: usize) {
        self.bytes.advance(count);
    }
}

#[test]
fn elf_loading_accepts_fragmented_headers_and_segments() {
    let elf = crate::testing::fixture("ARCA_ZEROED_REGIONS");
    for split in [1, 15, 17, 63, 65, 119, 4095, 4097, elf.len() - 1] {
        let source = Chunks {
            bytes: elf,
            width: split,
        };
        let program: Function = common::elfloader::load_elf(source).unwrap();
        let result: Word = program.apply(Word::new(42)).force().try_into().unwrap();
        assert_eq!(result.read(), 2049);
    }
    for length in [0, 1, 15, 16, 51, 63, 64, 119] {
        assert!(common::elfloader::load_elf::<Runtime>(&elf[..length]).is_err());
    }
    let mut invalid = elf.to_vec();
    invalid[64 + 32..64 + 40].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(common::elfloader::load_elf::<Runtime>(invalid.as_slice()).is_err());
}

#[test]
fn elf_loading_reads_extended_program_header_counts() {
    let mut elf = crate::testing::fixture("ARCA_ZEROED_REGIONS").to_vec();
    let count = u16::from_le_bytes(elf[56..58].try_into().unwrap());
    let sections = u64::from_le_bytes(elf[40..48].try_into().unwrap()) as usize;
    assert_ne!(sections, 0);
    elf[56..58].copy_from_slice(&0xffffu16.to_le_bytes());
    elf[sections + 44..sections + 48].copy_from_slice(&(count as u32).to_le_bytes());
    let source = Chunks {
        bytes: &elf,
        width: 17,
    };
    let program: Function = common::elfloader::load_elf(source).unwrap();
    let result: Word = program.apply(Word::new(42)).force().try_into().unwrap();
    assert_eq!(result.read(), 2049);
}

#[test]
fn elf_zeroed_regions_survive_independent_invocations() {
    let program: Function =
        common::elfloader::load_elf(crate::testing::fixture("ARCA_ZEROED_REGIONS")).unwrap();
    for value in [7, 19, 255] {
        let result: Word = program
            .clone()
            .apply(Word::new(value))
            .force()
            .try_into()
            .unwrap();
        assert_eq!(result.read(), 2049);
    }
}

#[test]
fn capture_and_call_preserve_independent_continuations() {
    for seed in [7, 19] {
        let program =
            common::elfloader::load_elf(crate::testing::fixture("ARCA_CONTINUATIONS")).unwrap();
        let mut result: Tuple = program.apply(Word::new(seed)).force().try_into().unwrap();
        let continuation: Function = result.take(0).try_into().unwrap();
        let bytes: Blob = result.take(1).try_into().unwrap();
        assert_eq!(bytes[0], seed as u8 + 1);
        assert_eq!(&bytes[1..], &[seed as u8; 63]);
        let definition = continuation.read_cloned();
        let restored = Function::new(definition).unwrap();
        for (continuation, reply) in [(continuation, 41), (restored, 73)] {
            let effect: Function = continuation
                .apply(Word::new(reply))
                .force()
                .try_into()
                .unwrap();
            let mut definition: Tuple = effect.read().try_into().unwrap();
            let symbol: Blob = definition.take(1).try_into().unwrap();
            assert_eq!(&*symbol, b"captured");
            let mut args: Tuple = definition.take(2).try_into().unwrap();
            let actual_seed: Word = args.take(0).try_into().unwrap();
            let actual_reply: Word = args.take(1).try_into().unwrap();
            assert_eq!(actual_seed.read(), seed);
            assert_eq!(actual_reply.read(), reply);
            let continuation: Function = args.take(2).try_into().unwrap();
            let result: Word = continuation
                .apply(Word::new(99))
                .force()
                .try_into()
                .unwrap();
            assert_eq!(result.read(), 99);
        }
    }
}
