#![allow(clippy::double_parens)]
pub use common::bitpack::BitPack;
use core::fmt;
use derive_more::{From, TryInto, TryUnwrap, Unwrap};

#[derive(Copy, Clone, Debug)]
pub enum Error {
    Unwrap,
}

const fn ceil_log2(n: u32) -> u32 {
    if n <= 1 {
        0
    } else {
        32 - (n - 1).leading_zeros()
    }
}

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

#[derive(Debug, Clone, Copy)]
struct RawHandle {
    content: [u8; 32],
}

impl RawHandle {
    fn new(content: [u8; 32]) -> Self {
        Self { content }
    }
}

#[derive(Debug, Clone, Copy)]
struct MachineHandle {
    inner: RawHandle,
}

impl MachineHandle {
    fn new(payload: u64, size: u64) -> Self {
        assert!(size & 0xffff000000000000 == 0);
        let field = unsafe { core::mem::transmute::<[u64; 4], [u8; 32]>([payload, 0, 0, size]) };
        let inner = RawHandle::new(field);
        Self { inner }
    }

    fn get_payload(&self) -> u64 {
        let field: &[u64; 4] = unsafe { core::mem::transmute(&self.inner.content) };
        field[0]
    }

    fn get_size(&self) -> u64 {
        let field: &[u64; 4] = unsafe { core::mem::transmute(&self.inner.content) };
        field[3] & 0xffffffffffff
    }
}

impl BitPack for MachineHandle {
    const TAGBITS: u32 = 240;

    fn unpack(mut content: [u8; 32]) -> Self {
        content[30] = 0;
        content[31] = 0;
        let inner = RawHandle::new(content);
        Self { inner }
    }

    fn pack(&self) -> [u8; 32] {
        self.inner.content
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VirtualHandle {
    inner: MachineHandle,
}

impl BitPack for VirtualHandle {
    const TAGBITS: u32 = MachineHandle::TAGBITS;
    fn unpack(content: [u8; 32]) -> Self {
        let inner = MachineHandle::unpack(content);
        Self { inner }
    }

    fn pack(&self) -> [u8; 32] {
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

#[derive(Clone, Copy)]
pub struct PhysicalHandle {
    inner: MachineHandle,
}

impl BitPack for PhysicalHandle {
    const TAGBITS: u32 = MachineHandle::TAGBITS;
    fn unpack(content: [u8; 32]) -> Self {
        let inner = MachineHandle::unpack(content);
        Self { inner }
    }

    fn pack(&self) -> [u8; 32] {
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

impl fmt::Debug for PhysicalHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "local_id: {}", self.local_id())
    }
}

#[derive(Clone, Copy)]
pub struct CanonicalHandle {
    inner: RawHandle,
}

impl CanonicalHandle {
    pub fn new(hash: [u8; 32], size: u64) -> Self {
        assert!(size & 0xffff000000000000 == 0);
        let hash_64: &[u64; 4] = unsafe { core::mem::transmute(&hash) };
        let field = unsafe {
            core::mem::transmute::<[u64; 4], [u8; 32]>([hash_64[0], hash_64[1], hash_64[2], size])
        };
        let inner = RawHandle::new(field);
        Self { inner }
    }

    pub fn len(&self) -> usize {
        let field: &[u64; 4] = unsafe { core::mem::transmute(&self.inner.content) };
        (field[3] & 0xffffffffffff).try_into().unwrap()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl fmt::Debug for CanonicalHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x")?;

        for i in self.inner.content.iter() {
            write!(f, "{:02x}", i)?;
        }

        Ok(())
    }
}

impl BitPack for CanonicalHandle {
    const TAGBITS: u32 = 240;

    fn unpack(mut content: [u8; 32]) -> Self {
        content[30] = 0;
        content[31] = 0;
        let inner = RawHandle::new(content);
        Self { inner }
    }

    fn pack(&self) -> [u8; 32] {
        self.inner.content
    }
}

#[derive(Clone, Copy)]
pub struct LiteralHandle {
    inner: RawHandle,
}

impl LiteralHandle {
    pub fn new(content: &[u8]) -> Self {
        assert!(content.len() <= 30);

        let mut field = [0u8; 32];
        field[..content.len()].copy_from_slice(content);
        field[30] = content.len().try_into().expect("size larger than 30");
        let inner = RawHandle::new(field);
        Self { inner }
    }

    pub fn len(&self) -> usize {
        self.inner.content[30] as usize
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn content(&self) -> &[u8] {
        let len = self.len();
        &self.inner.content[..len]
    }
}

impl fmt::Debug for LiteralHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let content = self.content();

        if content.len() == 8 {
            let data = u64::from_le_bytes(content.try_into().unwrap());
            write!(f, "0d{}", data)?;
        } else if content.len() == 4 {
            let data = u32::from_le_bytes(content.try_into().unwrap());
            write!(f, "0d{}", data)?;
        } else {
            write!(f, "[")?;
            for i in content.iter() {
                write!(f, "0d{}, ", i)?;
            }
            write!(f, "]")?;
        }

        Ok(())
    }
}

impl BitPack for LiteralHandle {
    const TAGBITS: u32 = 245;

    fn unpack(mut content: [u8; 32]) -> Self {
        content[30] &= 0b00011111;
        content[31] = 0;
        let inner = RawHandle::new(content);
        Self { inner }
    }

    fn pack(&self) -> [u8; 32] {
        self.inner.content
    }
}

#[derive(BitPack, Debug, Clone, Copy, From, TryUnwrap)]
pub enum Handle {
    VirtualHandle(VirtualHandle),
    PhysicalHandle(PhysicalHandle),
    CanonicalHandle(CanonicalHandle),
}

#[derive(BitPack, Debug, TryUnwrap, Unwrap, From, Clone, Copy)]
#[unwrap(ref)]
#[try_unwrap(ref)]
pub enum BlobName {
    Blob(Handle),
    Literal(LiteralHandle),
}

#[derive(BitPack, Debug, Unwrap, Clone, Copy)]
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

#[derive(BitPack, Debug, TryUnwrap, Unwrap, From, Clone, Copy)]
#[try_unwrap(ref)]
pub enum Ref {
    BlobRef(BlobName),
    TreeRef(TreeName),
}

#[derive(BitPack, Debug, TryUnwrap, Unwrap, From, Clone, Copy)]
#[try_unwrap(ref)]
pub enum Object {
    BlobObj(BlobName),
    TreeObj(TreeName),
}

#[derive(BitPack, Debug, Unwrap, Clone, Copy)]
pub enum Thunk {
    Identification(Ref),
    Application(TreeName),
    Selection(TreeName),
}

#[derive(BitPack, Debug, TryUnwrap, Unwrap, Clone, Copy)]
#[try_unwrap(ref)]
pub enum Encode {
    Strict(Thunk),
    Shallow(Thunk),
}

#[derive(Debug, BitPack, TryUnwrap, Unwrap, From, Clone, Copy)]
#[try_unwrap(ref)]
pub enum FixHandle {
    Ref(Ref),
    Object(Object),
    Thunk(Thunk),
    Encode(Encode),
}

#[derive(BitPack, Debug, TryInto, Unwrap, From, Clone, Copy)]
pub enum Value {
    Ref(Ref),
    Object(Object),
    Thunk(Thunk),
}

pub fn create_application_thunk(handle: &FixHandle) -> Result<FixHandle, Error> {
    let result = handle
        .try_unwrap_object_ref()
        .map_err(|_| Error::Unwrap)
        .and_then(|h| h.try_unwrap_tree_obj_ref().map_err(|_| Error::Unwrap))
        .or_else(|_| {
            handle
                .try_unwrap_ref_ref()
                .map_err(|_| Error::Unwrap)
                .and_then(|h| h.try_unwrap_tree_ref_ref().map_err(|_| Error::Unwrap))
        })?;

    Ok(FixHandle::Thunk(Thunk::Application(*result)))
}

pub fn create_strict_encode(handle: &FixHandle) -> Result<FixHandle, Error> {
    let result = handle.try_unwrap_thunk().map_err(|_| Error::Unwrap)?;

    Ok(FixHandle::Encode(Encode::Strict(result)))
}

#[cfg(test)]
mod tests {

    use crate::rawhandle::*;
    use core::simd::*;

    #[test]
    fn test_tag_bits() {
        assert_eq!(Handle::TAGBITS, 242);
        assert_eq!(BlobName::TAGBITS, 246);
        assert_eq!(TreeName::TAGBITS, 243);
        assert_eq!(Object::TAGBITS, 247);
        assert_eq!(Ref::TAGBITS, 247);
        assert_eq!(Thunk::TAGBITS, 249);
        assert_eq!(Encode::TAGBITS, 250);
        assert_eq!(FixHandle::TAGBITS, 252);
    }

    #[test]
    fn test_tag_masks() {
        assert_eq!(Handle::TAGMASK.as_array::<32>().unwrap()[30], 0b00000011);
        assert_eq!(Handle::TAGMASK.as_array::<32>().unwrap()[31], 0b00000000);

        let field: u16x16 = unsafe { core::mem::transmute(Handle::TAGMASK) };
        assert_eq!(field[15], 0b0000000000000011);

        assert_eq!(TreeName::TAGMASK.as_array::<32>().unwrap()[30], 0b00000100);
        assert_eq!(TreeName::TAGMASK.as_array::<32>().unwrap()[31], 0b00000000);

        assert_eq!(BlobName::TAGMASK.as_array::<32>().unwrap()[30], 0b00100000);
        assert_eq!(BlobName::TAGMASK.as_array::<32>().unwrap()[31], 0b00000000);

        assert_eq!(Thunk::TAGMASK.as_array::<32>().unwrap()[30], 0b10000000);
        assert_eq!(Thunk::TAGMASK.as_array::<32>().unwrap()[31], 0b00000001);

        assert_eq!(Encode::TAGMASK.as_array::<32>().unwrap()[30], 0b00000000);
        assert_eq!(Encode::TAGMASK.as_array::<32>().unwrap()[31], 0b00000010);

        assert_eq!(FixHandle::TAGMASK.as_array::<32>().unwrap()[30], 0b00000000);
        assert_eq!(FixHandle::TAGMASK.as_array::<32>().unwrap()[31], 0b00001100);
    }

    #[test]
    fn test_literal() {
        let content: usize = 3;
        let content = content.to_le_bytes();
        let literal = LiteralHandle::new(&content);

        assert_eq!(literal.len(), 8);
        let content = literal.content();
        let x = usize::from_le_bytes(content.try_into().unwrap());
        assert_eq!(x, 3);

        let handle = FixHandle::Ref(Ref::BlobRef(BlobName::Literal(literal)));
        let literal = FixHandle::unpack(handle.pack())
            .unwrap_ref()
            .unwrap_blob_ref()
            .unwrap_literal();

        assert_eq!(literal.len(), 8);
        let content = literal.content();
        let x = usize::from_le_bytes(content.try_into().unwrap());
        assert_eq!(x, 3);
    }

    #[test]
    fn test_pack() {
        let h: Handle = PhysicalHandle::new(42, 10086).into();
        let res = h.pack();
        let field: &u16x16 = unsafe { core::mem::transmute(&res) };
        assert_eq!(field[15], 0b0000000000000001);

        let h: TreeName = TreeName::Tag(PhysicalHandle::new(42, 10086).into());
        let res = h.pack();
        let field: &u16x16 = unsafe { core::mem::transmute(&res) };
        assert_eq!(field[15], 0b0000000000000101);

        let h: Encode = Encode::Shallow(Thunk::Selection(TreeName::NotTag(
            PhysicalHandle::new(42, 10086).into(),
        )));
        let res = h.pack();
        let field: &u16x16 = unsafe { core::mem::transmute(&res) };
        assert_eq!(field[15], 0b0000001100000001);

        let h: FixHandle = FixHandle::Encode(Encode::Shallow(Thunk::Selection(TreeName::NotTag(
            PhysicalHandle::new(42, 10086).into(),
        ))));
        let res = h.pack();
        let field: &u16x16 = unsafe { core::mem::transmute(&res) };
        assert_eq!(field[15], 0b0000111100000001);
    }

    #[test]
    fn test_round_trip() {
        let h: Handle = PhysicalHandle::new(42, 10086).into();
        let res = Handle::unpack(h.pack())
            .try_unwrap_physical_handle()
            .expect("Failed to unwrap to PhysicalHandle");
        assert_eq!(res.local_id(), 42);
        assert_eq!(res.len(), 10086);

        let h: FixHandle = FixHandle::Object(Object::BlobObj(BlobName::Blob(
            PhysicalHandle::new(42, 10086).into(),
        )));
        let res = FixHandle::unpack(h.pack())
            .try_unwrap_object()
            .expect("Failed to unwrap to Object")
            .try_unwrap_blob_obj()
            .expect("Failed to unwrap to BlobName")
            .unwrap_blob()
            .try_unwrap_physical_handle()
            .expect("Failed to unwrap to PhysicalHandle");

        let h: FixHandle = FixHandle::Object(Object::TreeObj(TreeName::NotTag(
            PhysicalHandle::new(42, 10086).into(),
        )));
        let new_h = FixHandle::unpack(h.pack());
        assert_eq!(
            new_h
                .try_unwrap_object()
                .unwrap()
                .try_unwrap_tree_obj()
                .unwrap()
                .unwrap_not_tag()
                .try_unwrap_physical_handle()
                .unwrap()
                .local_id(),
            42
        );

        assert_eq!(res.local_id(), 42);
        assert_eq!(res.len(), 10086);
    }

    #[test]
    fn test_thunk_round_trip() {
        let h: FixHandle = FixHandle::Thunk(Thunk::Application(TreeName::NotTag(
            PhysicalHandle::new(42, 10086).into(),
        )));
        let res = FixHandle::unpack(h.pack())
            .try_unwrap_thunk()
            .expect("Failed to unwrap to Thunk")
            .unwrap_application()
            .unwrap_not_tag()
            .try_unwrap_physical_handle()
            .expect("Failed to unwrap to PhysicalHandle");

        assert_eq!(res.local_id(), 42);
        assert_eq!(res.len(), 10086);
    }
}
