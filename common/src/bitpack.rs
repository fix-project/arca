use core::simd::u8x32;

pub use macros::BitPack;

pub trait BitPack {
    const TAGBITS: u32;
    fn pack(&self) -> u8x32;
    fn unpack(content: u8x32) -> Self;
}
