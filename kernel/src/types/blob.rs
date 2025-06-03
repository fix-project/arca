use core::ops::{Deref, DerefMut};

use crate::prelude::*;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Blob {
    contents: Box<[u8]>,
}

impl Blob {
    pub fn new<T: Into<Box<[u8]>>>(x: T) -> Self {
        Blob { contents: x.into() }
    }
}

impl arca::RuntimeType for Blob {
    type Runtime = Runtime;
}

impl arca::ValueType for Blob {
    const DATATYPE: DataType = DataType::Blob;
}

impl arca::Blob for Blob {
    fn read(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(&self.contents);
    }

    fn len(&self) -> usize {
        self.contents.len()
    }
}

impl Deref for Blob {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.contents
    }
}

impl DerefMut for Blob {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.contents
    }
}

impl From<Box<[u8]>> for Blob {
    fn from(value: Box<[u8]>) -> Self {
        Blob::new(value)
    }
}

impl From<Vec<u8>> for Blob {
    fn from(value: Vec<u8>) -> Self {
        Blob::new(value)
    }
}

impl From<String> for Blob {
    fn from(value: String) -> Self {
        Blob::new(value.into_bytes())
    }
}
