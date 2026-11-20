use alloc::{
    boxed::Box,
    sync::{Arc, Weak},
    vec::Vec,
};
use bytes::{Buf, Bytes};

#[derive(Clone, Debug)]
pub struct ImmutableBytes(Arc<Bytes>);

// a reference that does not keep the bytes alive
#[derive(Debug)]
pub(crate) struct WeakBytes(Weak<Bytes>);

impl WeakBytes {
    pub(crate) fn upgrade(&self) -> Option<ImmutableBytes> {
        self.0.upgrade().map(ImmutableBytes)
    }
}

impl ImmutableBytes {
    pub fn new(bytes: &[u8]) -> Self {
        Self(Arc::new(Bytes::copy_from_slice(bytes)))
    }

    pub(crate) fn downgrade(&self) -> WeakBytes {
        WeakBytes(Arc::downgrade(&self.0))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn into_buf(self) -> impl Buf + Clone {
        Bytes::clone(&self.0)
    }

    pub fn with_bytes(self, offset: usize, bytes: &[u8]) -> Self {
        let end = offset
            .checked_add(bytes.len())
            .expect("byte range overflow");
        assert!(offset <= self.len() && end <= 4 * (1usize << 30));
        if bytes.is_empty() {
            return self;
        }
        let mut result = Vec::from(&self.0[..]);
        result.resize(self.len().max(end), 0);
        result[offset..end].copy_from_slice(bytes);
        Self(Arc::new(result.into()))
    }

    pub fn to_bytes(&self) -> Box<[u8]> {
        self.0[..].into()
    }

    pub fn for_each_chunk(&self, mut f: impl FnMut(&[u8])) {
        f(&self.0);
    }
}
