#![allow(clippy::double_parens)]
use core::simd::{u8x32, u64x4};
use derive_more::{From, TryInto, Unwrap};
use macros::BitPack;

pub trait BitPack {
    const TAGBITS: u32;
    fn pack(&self) -> u8x32;
    fn unpack(content: u8x32) -> Self;
}

const fn ceil_log2(n: u32) -> u32 {
    if n <= 1 {
        0
    } else {
        32 - (n - 1).leading_zeros()
    }
}

const fn bitmask256<const I: u32, const WIDTH: u32>() -> u8x32 {
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
    u8x32::from_array(out)
}

#[derive(Debug)]
struct RawHandle {
    content: u8x32,
}

impl RawHandle {
    fn new(content: u8x32) -> Self {
        Self { content }
    }
}

#[derive(Debug)]
struct MachineHandle {
    inner: RawHandle,
}

impl MachineHandle {
    fn new(payload: u64, size: u64) -> Self {
        assert!(size & 0xffff000000000000 == 0);
        let field = unsafe {
            core::mem::transmute::<core::simd::Simd<u64, 4>, core::simd::Simd<u8, 32>>(
                u64x4::from_array([payload, 0, 0, size]),
            )
        };
        let inner = RawHandle::new(field);
        Self { inner }
    }

    fn get_payload(&self) -> u64 {
        let field: &u64x4 = unsafe { core::mem::transmute(&self.inner.content) };
        field[0]
    }

    fn get_size(&self) -> u64 {
        let field: &u64x4 = unsafe { core::mem::transmute(&self.inner.content) };
        field[3] & 0xffffffffffff
    }
}

impl BitPack for MachineHandle {
    const TAGBITS: u32 = 240;

    fn unpack(content: u8x32) -> Self {
        let inner = RawHandle::new(content);
        Self { inner }
    }

    fn pack(&self) -> u8x32 {
        self.inner.content
    }
}

#[derive(Debug)]
pub struct VirtualHandle {
    inner: MachineHandle,
}

impl BitPack for VirtualHandle {
    const TAGBITS: u32 = MachineHandle::TAGBITS;
    fn unpack(content: u8x32) -> Self {
        let inner = MachineHandle::unpack(content);
        Self { inner }
    }

    fn pack(&self) -> u8x32 {
        self.inner.pack()
    }
}

impl VirtualHandle {
    pub fn new(addr: usize, size: usize) -> Self {
        let inner = MachineHandle::new(addr as u64, size as u64);
        Self { inner }
    }

    pub fn addr(&self) -> usize {
        self.inner.get_payload().try_into().unwrap()
    }

    pub fn len(&self) -> usize {
        self.inner.get_size().try_into().unwrap()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug)]
pub struct PhysicalHandle {
    inner: MachineHandle,
}

impl BitPack for PhysicalHandle {
    const TAGBITS: u32 = MachineHandle::TAGBITS;
    fn unpack(content: u8x32) -> Self {
        let inner = MachineHandle::unpack(content);
        Self { inner }
    }

    fn pack(&self) -> u8x32 {
        self.inner.pack()
    }
}

impl PhysicalHandle {
    pub fn new(local_id: usize, size: usize) -> Self {
        let inner = MachineHandle::new(local_id as u64, size as u64);
        Self { inner }
    }

    pub fn local_id(&self) -> usize {
        self.inner.get_payload().try_into().unwrap()
    }

    pub fn len(&self) -> usize {
        self.inner.get_size().try_into().unwrap()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(BitPack, Debug)]
pub enum Handle {
    VirtualHandle(VirtualHandle),
    PhysicalHandle(PhysicalHandle),
}

#[derive(BitPack, Debug, TryInto, Unwrap, From)]
pub enum BlobName {
    Blob(Handle),
}

#[derive(BitPack, Debug, Unwrap)]
pub enum TreeName {
    NotTag(Handle),
    Tag(Handle),
}

impl From<TreeName> for Handle {
    fn from(val: TreeName) -> Self {
        match val {
            TreeName::Tag(h) | TreeName::NotTag(h) => h,
        }
    }
}

#[derive(BitPack, Debug, TryInto, Unwrap, From)]
pub enum Ref {
    BlobName(BlobName),
    TreeName(TreeName),
}

#[derive(BitPack, Debug, TryInto, Unwrap, From)]
pub enum Object {
    BlobName(BlobName),
    TreeName(TreeName),
}

#[derive(BitPack, Debug, Unwrap)]
pub enum Thunk {
    Identification(Ref),
    Application(TreeName),
    Selection(TreeName),
}

#[derive(BitPack, Debug, TryInto, Unwrap)]
pub enum Encode {
    Strict(Thunk),
    Shallow(Thunk),
}

#[derive(Debug, BitPack, TryInto, Unwrap, From)]
pub enum FixHandle {
    Ref(Ref),
    Object(Object),
    Thunk(Thunk),
    Encode(Encode),
}

#[derive(BitPack, Debug, TryInto, Unwrap, From)]
pub enum Value {
    Ref(Ref),
    Object(Object),
    Thunk(Thunk),
}
