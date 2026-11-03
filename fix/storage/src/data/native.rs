use alloc::{boxed::Box, vec::Vec};
use bytes::{Buf, Bytes};

#[derive(Clone, Debug)]
pub struct ImmutableBytes(Bytes);

impl ImmutableBytes {
    pub fn new(bytes: &[u8]) -> Self {
        Self(Bytes::copy_from_slice(bytes))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn into_buf(self) -> impl Buf + Clone {
        self.0
    }

    pub fn with_bytes(self, offset: usize, bytes: &[u8]) -> Self {
        let end = offset
            .checked_add(bytes.len())
            .expect("byte range overflow");
        assert!(offset <= self.len() && end <= 4 * (1usize << 30));
        if bytes.is_empty() {
            return self;
        }
        let mut result = Vec::from(self.0.as_ref());
        result.resize(self.len().max(end), 0);
        result[offset..end].copy_from_slice(bytes);
        Self(result.into())
    }

    pub fn to_bytes(&self) -> Box<[u8]> {
        self.0.as_ref().into()
    }

    pub fn for_each_chunk(&self, mut f: impl FnMut(&[u8])) {
        f(&self.0);
    }
}
