#![no_std]
#![allow(dead_code)]
#![allow(unused_features)]
#![feature(portable_simd)]

use derive_more::{From, Into, TryUnwrap, Unwrap};
pub use common::bitpack::BitPack;
use bitint::U5;

const fn bitmask256<const I: u32, const WIDTH: u32>() -> [u8; 32] {
    assert!(I + WIDTH <= 256);
    let mut out = [0u8; 32];
    let mut i = I;
    loop {
        if i >= I + WIDTH {
            break;
        }

        let byte = i / 8;
        let off = i % 8;
        out[byte as usize] |= 1u8 << off;

        i += 1;
    }
    out
}

#[derive(BitPack, Debug, Copy, Clone, Eq, PartialEq, TryUnwrap, Unwrap, From)]
#[try_unwrap(ref)]
pub enum Handle {
    Ref(Ref),
    Object(Object),
    Thunk(Thunk),
    Encode(Encode),
}

impl Handle {
    pub fn len(&self) -> usize {
        match self {
            Handle::Ref(x) => x.len(),
            Handle::Object(x) => x.len(),
            Handle::Thunk(x) => x.len(),
            Handle::Encode(x) => x.len(),
        }
    }
}

#[derive(BitPack, Debug, Copy, Clone, Eq, PartialEq, TryUnwrap, Unwrap, From)]
#[try_unwrap(ref)]
pub enum Ref {
    Blob(Blob),
    Tree(Tree),
}

impl Ref {
    pub fn len(&self) -> usize {
        match self {
            Ref::Blob(x) => x.len(),
            Ref::Tree(x) => x.len(),
        }
    }
}

#[derive(BitPack, Debug, Copy, Clone, Eq, PartialEq, TryUnwrap, Unwrap, From)]
#[try_unwrap(ref)]
pub enum Object {
    Blob(Blob),
    Tree(Tree),
}

impl Object {
    pub fn len(&self) -> usize {
        match self {
            Object::Blob(x) => x.len(),
            Object::Tree(x) => x.len(),
        }
    }
}

#[derive(BitPack, Debug, Copy, Clone, Eq, PartialEq, Unwrap)]
pub enum Thunk {
    Identification(Ref),
    Application(Tree),
    Selection(Tree),
}

impl Thunk {
    pub fn len(&self) -> usize {
        match self {
            Thunk::Identification(x) => x.len(),
            Thunk::Application(x) => x.len(),
            Thunk::Selection(x) => x.len(),
        }
    }
}

#[derive(BitPack, Debug, Copy, Clone, Eq, PartialEq, TryUnwrap, Unwrap)]
#[try_unwrap(ref)]
pub enum Encode {
    Strict(Thunk),
    Shallow(Thunk),
}

impl Encode {
    pub fn len(&self) -> usize {
        match self {
            Encode::Strict(x) => x.len(),
            Encode::Shallow(x) => x.len(),
        }
    }
}

#[derive(BitPack, Debug, Copy, Clone, Eq, PartialEq, TryUnwrap, Unwrap)]
#[try_unwrap(ref)]
pub enum Tree {
    Tree(TreeName),
    Tag(TreeName),
}

impl Tree {
    pub fn len(&self) -> usize {
        match self {
            Tree::Tree(x) => x.len(),
            Tree::Tag(x) => x.len(),
        }
    }
}

#[derive(BitPack, Debug, Copy, Clone, Eq, PartialEq, TryUnwrap, Unwrap)]
#[try_unwrap(ref)]
pub enum Blob {
    Blob(BlobName),
    Literal(LiteralName),
}

impl Blob {
    pub fn len(&self) -> usize {
        match self {
            Blob::Blob(x) => x.len(),
            Blob::Literal(x) => x.len(),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, From, Into)]
pub struct BlobName(RawName);

#[derive(Debug, Copy, Clone, Eq, PartialEq, From, Into)]
pub struct LiteralName {
    bytes: [u8; 30],
    len: U5,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, From, Into)]
pub struct TreeName(RawName);

impl BlobName {
    pub unsafe fn new(name: RawName) -> Self {
        Self(name)
    }

    pub fn name(&self) -> RawName {
        self.0
    }

    pub fn len(&self) -> usize {
        self.0.size.to_primitive() as usize
    }
}

impl LiteralName {
    pub unsafe fn new(contents: &[u8]) -> Self {
        assert!(contents.len() <= 30);
        let len = U5::new(contents.len() as u8).unwrap();
        let mut bytes = [0; 30];
        bytes[..contents.len()].copy_from_slice(contents);
        Self {
            bytes,
            len,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        let len = self.len.to_primitive() as usize;
        &self.bytes[..len]
    }

    pub fn len(&self) -> usize {
        self.len.to_primitive() as usize
    }
}

impl TreeName {
    pub unsafe fn new(name: RawName) -> Self {
        Self(name)
    }

    pub fn name(&self) -> RawName {
        self.0
    }

    pub fn len(&self) -> usize {
        self.0.size.to_primitive() as usize
    }
}

impl common::bitpack::BitPack for BlobName {
    const TAGBITS: u32 = 240;

    fn pack(&self) -> [u8; 32] {
        self.0.into()
    }

    fn unpack(content: [u8; 32]) -> Self {
        unsafe {
            Self::new(RawName::forge(content))
        }
    }
}

impl common::bitpack::BitPack for LiteralName {
    const TAGBITS: u32 = 245;

    fn pack(&self) -> [u8; 32] {
        let mut bytes = [0; 32];
        bytes[0..30].copy_from_slice(&self.bytes);
        bytes[30] = self.len.to_primitive();
        bytes
    }

    fn unpack(content: [u8; 32]) -> Self {
        let mut bytes = [0; 30];
        bytes.copy_from_slice(&content[0..30]);
        let len = content[30] & 0b11111;
        let len = U5::new(len).unwrap();
        Self {
            bytes,
            len
        }
    }
}

impl common::bitpack::BitPack for TreeName {
    const TAGBITS: u32 = 240;

    fn pack(&self) -> [u8; 32] {
        self.0.into()
    }

    fn unpack(content: [u8; 32]) -> Self {
        unsafe {
            Self::new(RawName::forge(content))
        }
    }
}

impl From<Blob> for Handle {
    fn from(value: Blob) -> Handle {
        Handle::Object(Object::Blob(value))
    }
}

impl From<Tree> for Handle {
    fn from(value: Tree) -> Handle {
        Handle::Object(Object::Tree(value))
    }
}

impl From<BlobName> for Blob {
    fn from(value: BlobName) -> Blob {
        Blob::Blob(value)
    }
}

impl From<TreeName> for Tree {
    fn from(value: TreeName) -> Tree {
        Tree::Tree(value)
    }
}

impl From<Tree> for TreeName {
    fn from(value: Tree) -> TreeName {
        match value {
            Tree::Tree(x) => x,
            Tree::Tag(x) => x,
        }
    }
}

use core::simd::u8x32;
use core::convert::{From, Into};

use bitint::U48;

/// A raw Fix name. This struct conveys no information about the validity of the contained name.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C, packed)]
pub struct RawName {
    pub name: [u8; 24],
    pub size: U48,
    pub meta: u16,
}

impl RawName {
    pub fn forge(bytes: [u8; 32]) -> Self {
        let mut name = [0; 24];
        name.copy_from_slice(&bytes[..24]);
        let mut size = [0; 8];
        size[..6].copy_from_slice(&bytes[24..30]);
        let size = u64::from_le_bytes(size);
        let size = U48::new(size).unwrap();
        let mut meta = [0; 2];
        meta.copy_from_slice(&bytes[30..32]);
        let meta = u16::from_le_bytes(meta);
        Self {
            name,
            size,
            meta
        }
    }

    pub fn as_bytes(&self) -> [u8; 32] {
        let mut bytes = [0; 32];
        bytes[0..24].copy_from_slice(&self.name);
        let size: [u8; 8] = self.size.to_primitive().to_le_bytes();
        bytes[24..30].copy_from_slice(&size[..6]);
        bytes[30..32].copy_from_slice(&self.meta.to_le_bytes());
        bytes
    }
}

impl From<RawName> for [u8; 32] {
    fn from(value: RawName) -> [u8; 32] {
        value.as_bytes()
    }
}

impl From<RawName> for u8x32 {
    fn from(value: RawName) -> u8x32 {
        value.as_bytes().into()
    }
}

impl core::fmt::Display for Handle {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        // write!(f, "{self:?}")
        let bytes = Handle::pack(self);
        for byte in bytes {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
