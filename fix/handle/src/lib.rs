#![no_std]
#![feature(portable_simd)]

use core::{fmt, mem, simd::u8x32};

pub const HANDLE_SIZE: usize = 32;
pub const MAX_BYTE_LEN: u64 = (1 << 48) - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidType,
    InvalidName,
    ReservedBits,
    LiteralLength,
    LiteralPadding,
    ShortCanonicalBlob,
    TreeLength,
    LengthOverflow,
    WrongType,
    NonCanonicalChild,
    HashMismatch,
    InvalidEq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Name<'a> {
    Literal(&'a [u8]),
    Canonical(&'a [u8; 24]),
    Machine { index: u64, storage_id: u64 },
    Local { address: u64 },
}

/// A Fix handle whose naming provenance is established by its constructor.
/// Storage providers must still validate names before access.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Handle(u8x32);

/// Structurally validated bytes, without naming provenance or a native alignment requirement.
#[derive(Debug, Clone, Copy)]
pub struct HandleBytes<'a>(&'a [u8; HANDLE_SIZE]);

#[derive(Debug, Clone, Copy)]
pub enum HandleBytesView<'a> {
    Object(HandleBytes<'a>),
    Ref(HandleBytes<'a>),
    Thunk(HandleBytes<'a>),
    Encode(HandleBytes<'a>),
}

#[derive(Debug, Clone, Copy)]
pub enum HandleView<'a> {
    Object(&'a Object),
    Ref(&'a Ref),
    Thunk(&'a Thunk),
    Encode(&'a Encode),
}

#[derive(Debug, Clone, Copy)]
pub enum ObjectView<'a> {
    Blob(&'a Blob),
    Tree(&'a Tree),
}

#[derive(Debug, Clone, Copy)]
pub enum ThunkView {
    Identification(Ref),
    Application(TreeRef),
    Selection(TreeRef),
}

#[derive(Debug, Clone, Copy)]
pub enum EncodeView {
    Strict(Thunk),
    Shallow(Thunk),
}

macro_rules! wrapper {
    ($name:ident, $test:expr) => {
        #[derive(Debug, Clone, Copy)]
        #[repr(transparent)]
        pub struct $name(Handle);
        impl $name {
            #[inline]
            pub fn handle(self) -> Handle {
                self.0
            }
            #[inline]
            pub fn as_handle(&self) -> &Handle {
                &self.0
            }
            #[inline]
            pub fn len(&self) -> usize {
                self.0.len()
            }
            #[inline]
            pub fn checked_len(&self) -> Result<usize, Error> {
                self.0.checked_len()
            }
            #[inline]
            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
            #[inline]
            pub fn byte_len(&self) -> u64 {
                self.0.byte_len()
            }
        }
        impl From<$name> for Handle {
            #[inline]
            fn from(value: $name) -> Self {
                value.0
            }
        }
        impl TryFrom<Handle> for $name {
            type Error = Error;
            #[inline]
            fn try_from(value: Handle) -> Result<Self, Error> {
                if ($test)(&value) {
                    Ok(Self(value))
                } else {
                    Err(Error::WrongType)
                }
            }
        }
        impl<'a> TryFrom<&'a Handle> for &'a $name {
            type Error = Error;
            #[inline]
            fn try_from(value: &'a Handle) -> Result<Self, Error> {
                if ($test)(value) {
                    // The wrapper is transparent and has no additional fields.
                    Ok(unsafe { &*(value as *const Handle).cast::<$name>() })
                } else {
                    Err(Error::WrongType)
                }
            }
        }
    };
}

wrapper!(Object, |h: &Handle| h.0[31] & 0x1f == 0);
wrapper!(Ref, |h: &Handle| h.0[31] & 0x1f == 0x10);
wrapper!(Thunk, |h: &Handle| h.0[31] & 3 == 0 && h.0[31] & 12 != 0);
wrapper!(Encode, |h: &Handle| h.0[31] & 3 != 0);
wrapper!(Blob, |h: &Handle| h.0[31] & 0x1f == 0
    && h.0[31] & 0xe0 <= 0x80);
wrapper!(Tree, |h: &Handle| h.0[31] & 0x1f == 0
    && h.0[31] & 0xa0 == 0xa0);
wrapper!(TreeRef, |h: &Handle| h.0[31] & 0x1f == 0x10
    && h.0[31] & 0xa0 == 0xa0);

impl Handle {
    /// # Safety
    /// The caller must establish naming provenance for the resulting Handle.
    /// Local addresses must identify live data of the declared size in the
    /// address space where the Handle is used. Encoding checks do not establish this.
    #[inline]
    pub unsafe fn parse(bytes: [u8; HANDLE_SIZE]) -> Result<Self, Error> {
        Self::validate(&bytes)?;
        Ok(Self(u8x32::from_array(bytes)))
    }
    /// # Safety
    /// The caller must establish naming provenance for the resulting Handle.
    /// Local addresses must identify live data of the declared size in the
    /// address space where the Handle is used. Encoding checks do not establish this.
    #[inline]
    pub unsafe fn parse_native(native: u8x32) -> Result<Self, Error> {
        Self::validate(native.as_array())?;
        Ok(Self(native))
    }
    #[inline]
    pub fn parse_ref(bytes: &[u8; HANDLE_SIZE]) -> Result<HandleBytes<'_>, Error> {
        Self::validate(bytes)?;
        Ok(HandleBytes(bytes))
    }
    #[inline]
    pub fn into_native(self) -> u8x32 {
        self.0
    }
    #[inline]
    pub fn bytes_view(&self) -> HandleBytes<'_> {
        HandleBytes(self.as_bytes())
    }
    fn validate(bytes: &[u8; HANDLE_SIZE]) -> Result<(), Error> {
        let tag = bytes[31];
        let data = tag & 0xe0;
        if !matches!(data, 0 | 0x80 | 0xa0 | 0xe0) || tag & 3 == 3 {
            return Err(Error::InvalidType);
        }
        let thunk = (tag >> 2) & 3;
        if (tag & 3 != 0 && thunk == 0)
            || (thunk != 0 && tag & 0x10 == 0)
            || (matches!(thunk, 1 | 3) && !matches!(data, 0xa0 | 0xe0))
        {
            return Err(Error::InvalidType);
        }
        if data == 0 {
            if bytes[30] & 0xe0 != 0 {
                return Err(Error::ReservedBits);
            }
            let len = bytes[30] as usize;
            if len > 30 {
                return Err(Error::LiteralLength);
            }
            if bytes[len..30].iter().any(|&b| b != 0) {
                return Err(Error::LiteralPadding);
            }
        } else {
            if bytes[30] & 0xf8 != 0 {
                return Err(Error::ReservedBits);
            }
            match bytes[30] & 3 {
                0 => {
                    if data == 0x80 && byte_len(bytes) < 31 {
                        return Err(Error::ShortCanonicalBlob);
                    }
                }
                2 => {
                    if bytes[16..24].iter().any(|&b| b != 0) {
                        return Err(Error::ReservedBits);
                    }
                }
                3 => {
                    if bytes[8..24].iter().any(|&b| b != 0) {
                        return Err(Error::ReservedBits);
                    }
                }
                _ => return Err(Error::InvalidName),
            }
            if matches!(data, 0xa0 | 0xe0) && !byte_len(bytes).is_multiple_of(HANDLE_SIZE as u64) {
                return Err(Error::TreeLength);
            }
        }
        Ok(())
    }
    #[inline]
    pub fn as_bytes(&self) -> &[u8; HANDLE_SIZE] {
        self.0.as_array()
    }
    #[inline]
    pub fn into_bytes(self) -> [u8; HANDLE_SIZE] {
        self.0.to_array()
    }
    #[inline]
    pub fn same_encoding(&self, other: &Self) -> bool {
        self.0 == other.0
    }
    #[inline]
    pub fn byte_len(&self) -> u64 {
        byte_len(self.as_bytes())
    }
    #[inline]
    pub fn checked_len(&self) -> Result<usize, Error> {
        self.bytes_view().checked_len()
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.checked_len()
            .expect("handle length exceeds address space")
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.byte_len() == 0
    }
    #[inline]
    pub fn is_tree_data(&self) -> bool {
        self.bytes_view().is_tree_data()
    }
    #[inline]
    pub fn is_canonical(&self) -> bool {
        self.bytes_view().is_canonical()
    }
    #[inline]
    pub fn is_eq(&self) -> bool {
        self.0[31] & 0x0f == 0 && (self.0[31] & 0xe0 <= 0x80 || self.eq_bit() == Some(true))
    }
    #[inline]
    pub fn eq_bit(&self) -> Option<bool> {
        self.bytes_view().eq_bit()
    }
    #[inline]
    pub fn name(&self) -> Name<'_> {
        self.bytes_view().name()
    }
    #[inline]
    pub fn view(&self) -> HandleView<'_> {
        if self.0[31] & 3 != 0 {
            HandleView::Encode(<&Encode>::try_from(self).expect("validated encoding"))
        } else if self.0[31] & 12 != 0 {
            HandleView::Thunk(<&Thunk>::try_from(self).expect("validated thunk"))
        } else if self.0[31] & 0x10 != 0 {
            HandleView::Ref(<&Ref>::try_from(self).expect("validated ref"))
        } else {
            HandleView::Object(<&Object>::try_from(self).expect("validated object"))
        }
    }
    #[inline]
    fn flags<const SET: u8, const CLEAR: u8>(self) -> Self {
        let mut set = [0; HANDLE_SIZE];
        set[31] = SET;
        let mut keep = [u8::MAX; HANDLE_SIZE];
        keep[31] = !CLEAR;
        Self((self.0 & u8x32::from_array(keep)) | u8x32::from_array(set))
    }
}

impl<'a> HandleBytes<'a> {
    #[inline]
    pub fn as_bytes(self) -> &'a [u8; HANDLE_SIZE] {
        self.0
    }
    /// # Safety
    /// The caller must establish naming provenance for the resulting Handle.
    /// Local addresses must identify live data of the declared size in the
    /// address space where the Handle is used. Encoding checks do not establish this.
    #[inline]
    pub unsafe fn to_owned(self) -> Handle {
        Handle(u8x32::from_array(*self.0))
    }
    #[inline]
    pub fn byte_len(self) -> u64 {
        byte_len(self.0)
    }
    #[inline]
    pub fn checked_len(self) -> Result<usize, Error> {
        let len = if self.is_tree_data() {
            self.byte_len() / HANDLE_SIZE as u64
        } else {
            self.byte_len()
        };
        usize::try_from(len).map_err(|_| Error::LengthOverflow)
    }
    #[inline]
    pub fn is_tree_data(self) -> bool {
        self.0[31] & 0xa0 == 0xa0
    }
    #[inline]
    pub fn is_canonical(self) -> bool {
        self.0[31] & 0xe0 == 0 || self.0[30] & 3 == 0
    }
    #[inline]
    pub fn eq_bit(self) -> Option<bool> {
        (self.0[31] & 0xe0 != 0).then_some(self.0[30] & 4 != 0)
    }
    #[inline]
    pub fn name(self) -> Name<'a> {
        if self.0[31] & 0xe0 == 0 {
            return Name::Literal(&self.0[..self.0[30] as usize]);
        }
        match self.0[30] & 3 {
            0 => Name::Canonical(self.0[..24].try_into().expect("fixed hash width")),
            2 => Name::Machine {
                index: read_u64(&self.0[..8]),
                storage_id: read_u64(&self.0[8..16]),
            },
            3 => Name::Local {
                address: read_u64(&self.0[..8]),
            },
            _ => unreachable!(),
        }
    }
    #[inline]
    pub fn view(self) -> HandleBytesView<'a> {
        if self.0[31] & 3 != 0 {
            HandleBytesView::Encode(self)
        } else if self.0[31] & 12 != 0 {
            HandleBytesView::Thunk(self)
        } else if self.0[31] & 0x10 != 0 {
            HandleBytesView::Ref(self)
        } else {
            HandleBytesView::Object(self)
        }
    }
}

impl Object {
    #[inline]
    pub fn view(&self) -> ObjectView<'_> {
        if self.0.is_tree_data() {
            ObjectView::Tree(<&Tree>::try_from(&self.0).expect("tree object"))
        } else {
            ObjectView::Blob(<&Blob>::try_from(&self.0).expect("blob object"))
        }
    }
    #[inline]
    pub fn into_ref(self) -> Ref {
        Ref(self.0.flags::<0x10, 0>())
    }
}

impl Ref {
    #[inline]
    pub fn object_descriptor(self) -> Object {
        Object(self.0.flags::<0, 0x10>())
    }
    #[inline]
    pub fn identification(self) -> Thunk {
        Thunk(self.0.flags::<8, 0>())
    }
}

impl Blob {
    #[inline]
    pub fn literal(contents: &[u8]) -> Result<Self, Error> {
        if contents.len() > 30 {
            return Err(Error::LiteralLength);
        }
        let mut bytes = [0; HANDLE_SIZE];
        bytes[..contents.len()].copy_from_slice(contents);
        bytes[30] = contents.len() as u8;
        Ok(Self(Handle(u8x32::from_array(bytes))))
    }
    #[inline]
    pub fn canonical(contents: &[u8]) -> Result<Self, Error> {
        if contents.len() <= 30 {
            return Self::literal(contents);
        }
        let hash = blake3::hash(contents);
        unsafe {
            Self::named(
                Name::Canonical(hash.as_bytes()[..24].try_into().expect("hash width")),
                contents.len() as u64,
                true,
            )
        }
    }
    /// # Safety
    /// The caller must establish naming provenance for the resulting Handle.
    /// Local addresses must identify live data of the declared size in the
    /// address space where the Handle is used. Encoding checks do not establish this.
    #[inline]
    pub unsafe fn named(name: Name<'_>, byte_len: u64, eq_bit: bool) -> Result<Self, Error> {
        Ok(Self(unsafe { named(name, byte_len, eq_bit, 0x80)? }))
    }
    #[inline]
    pub fn literal_bytes(&self) -> Option<&[u8]> {
        match self.0.name() {
            Name::Literal(bytes) => Some(bytes),
            _ => None,
        }
    }
    #[inline]
    pub fn verify(&self, contents: &[u8]) -> Result<(), Error> {
        if self.0.eq_bit() == Some(false) {
            return Err(Error::InvalidEq);
        }
        if self.byte_len() != contents.len() as u64 {
            return Err(Error::HashMismatch);
        }
        match self.0.name() {
            Name::Literal(bytes) if bytes == contents => Ok(()),
            Name::Canonical(hash) if hash[..] == blake3::hash(contents).as_bytes()[..24] => Ok(()),
            _ => Err(Error::HashMismatch),
        }
    }
    #[inline]
    pub fn object(self) -> Object {
        Object(self.0)
    }
    #[inline]
    pub fn into_ref(self) -> Ref {
        self.object().into_ref()
    }
}

impl Tree {
    /// # Safety
    /// The caller must establish naming provenance for the resulting Handle.
    /// Local addresses must identify live data of the declared size in the
    /// address space where the Handle is used. Encoding checks do not establish this.
    #[inline]
    pub unsafe fn named(name: Name<'_>, byte_len: u64, eq_bit: bool) -> Result<Self, Error> {
        Ok(Self(unsafe { named(name, byte_len, eq_bit, 0xa0)? }))
    }
    #[inline]
    pub fn canonical(children: &[Handle]) -> Result<Self, Error> {
        let len = (children.len() as u64)
            .checked_mul(HANDLE_SIZE as u64)
            .ok_or(Error::LengthOverflow)?;
        let mut hasher = blake3::Hasher::new();
        for child in children {
            if !child.is_canonical() {
                return Err(Error::NonCanonicalChild);
            }
            hasher.update(child.as_bytes());
        }
        let hash = hasher.finalize();
        unsafe {
            Self::named(
                Name::Canonical(hash.as_bytes()[..24].try_into().expect("hash width")),
                len,
                children.iter().all(Handle::is_eq),
            )
        }
    }
    #[inline]
    pub fn verify(&self, children: &[Handle]) -> Result<(), Error> {
        let expected = Self::canonical(children)?;
        if self.0.eq_bit() != expected.0.eq_bit() {
            return Err(Error::InvalidEq);
        }
        if self.byte_len() != expected.byte_len() {
            return Err(Error::HashMismatch);
        }
        match (self.0.name(), expected.0.name()) {
            (Name::Canonical(a), Name::Canonical(b)) if a == b => Ok(()),
            _ => Err(Error::HashMismatch),
        }
    }
    #[inline]
    pub fn entry_count(&self) -> u64 {
        self.byte_len() / HANDLE_SIZE as u64
    }
    #[inline]
    pub fn is_tag(&self) -> bool {
        self.0.0[31] & 0x40 != 0
    }
    #[inline]
    pub fn tag_descriptor(self) -> Self {
        Self(self.0.flags::<0x40, 0>())
    }
    #[inline]
    pub fn object(self) -> Object {
        Object(self.0)
    }
    #[inline]
    pub fn into_ref(self) -> TreeRef {
        TreeRef(self.0.flags::<0x10, 0>())
    }
}

impl TreeRef {
    #[inline]
    pub fn object_descriptor(self) -> Tree {
        Tree(self.0.flags::<0, 0x10>())
    }
    #[inline]
    pub fn reference(self) -> Ref {
        Ref(self.0)
    }
    #[inline]
    pub fn application(self) -> Thunk {
        Thunk(self.0.flags::<4, 0>())
    }
    #[inline]
    pub fn selection(self) -> Thunk {
        Thunk(self.0.flags::<12, 0>())
    }
}

impl Thunk {
    #[inline]
    pub fn view(&self) -> ThunkView {
        let data = self.0.flags::<0, 12>();
        match self.0.0[31] & 12 {
            4 => ThunkView::Application(TreeRef(data)),
            8 => ThunkView::Identification(Ref(data)),
            12 => ThunkView::Selection(TreeRef(data)),
            _ => unreachable!(),
        }
    }
    #[inline]
    pub fn strict(self) -> Encode {
        Encode(self.0.flags::<1, 0>())
    }
    #[inline]
    pub fn shallow(self) -> Encode {
        Encode(self.0.flags::<2, 0>())
    }
}

impl Encode {
    #[inline]
    pub fn view(&self) -> EncodeView {
        let thunk = Thunk(self.0.flags::<0, 3>());
        match self.0.0[31] & 3 {
            1 => EncodeView::Strict(thunk),
            2 => EncodeView::Shallow(thunk),
            _ => unreachable!(),
        }
    }
}

impl From<Blob> for Object {
    #[inline]
    fn from(value: Blob) -> Self {
        value.object()
    }
}
impl From<Tree> for Object {
    #[inline]
    fn from(value: Tree) -> Self {
        value.object()
    }
}
impl From<TreeRef> for Ref {
    #[inline]
    fn from(value: TreeRef) -> Self {
        value.reference()
    }
}
impl From<Object> for Ref {
    #[inline]
    fn from(value: Object) -> Self {
        value.into_ref()
    }
}

unsafe fn named(name: Name<'_>, len: u64, eq_bit: bool, tag: u8) -> Result<Handle, Error> {
    if len > MAX_BYTE_LEN {
        return Err(Error::LengthOverflow);
    }
    let mut bytes = [0; HANDLE_SIZE];
    bytes[24..30].copy_from_slice(&len.to_le_bytes()[..6]);
    bytes[30] = if eq_bit { 4 } else { 0 };
    bytes[31] = tag;
    match name {
        Name::Canonical(hash) => bytes[..24].copy_from_slice(hash),
        Name::Machine { index, storage_id } => {
            bytes[..8].copy_from_slice(&index.to_le_bytes());
            bytes[8..16].copy_from_slice(&storage_id.to_le_bytes());
            bytes[30] |= 2;
        }
        Name::Local { address } => {
            bytes[..8].copy_from_slice(&address.to_le_bytes());
            bytes[30] |= 3;
        }
        Name::Literal(_) => return Err(Error::InvalidName),
    }
    unsafe { Handle::parse(bytes) }
}

#[inline]
fn read_u64(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("fixed integer width"))
}
#[inline]
fn byte_len(bytes: &[u8; HANDLE_SIZE]) -> u64 {
    if bytes[31] & 0xe0 == 0 {
        return bytes[30] as u64;
    }
    let mut len = [0; 8];
    len[..6].copy_from_slice(&bytes[24..30]);
    u64::from_le_bytes(len)
}

impl fmt::Debug for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl fmt::Display for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.as_bytes() {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
const _: () =
    assert!(mem::size_of::<Handle>() == HANDLE_SIZE && mem::align_of::<Handle>() == HANDLE_SIZE);

#[cfg(test)]
mod tests;
