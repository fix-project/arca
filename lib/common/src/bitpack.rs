pub use macros::BitPack;

pub trait BitPack {
    const TAGBITS: u32;
    fn pack(&self) -> [u8; 32];
    fn unpack(content: [u8; 32]) -> Self;
}
