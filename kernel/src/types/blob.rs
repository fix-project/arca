use core::ops::{Deref, DerefMut};

use alloc::string::ToString as _;
use common::message::Handle;

use crate::prelude::*;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Blob {
    Raw(Box<[u8]>),
    String(String),
}

impl Blob {
    pub fn new<T: Into<Box<[u8]>>>(x: T) -> Self {
        Blob::Raw(x.into())
    }

    pub fn string(x: String) -> Self {
        Blob::String(x)
    }

    fn make_blob(&mut self) -> &mut Box<[u8]> {
        if let Blob::String(s) = self {
            *self = Blob::Raw(s.as_bytes().into())
        };
        let Blob::Raw(items) = self else {
            unreachable!();
        };
        items
    }

    pub fn into_inner(self) -> Box<[u8]> {
        match self {
            Blob::Raw(items) => items,
            Blob::String(s) => s.as_bytes().into(),
        }
    }
}

impl arca::RuntimeType for Blob {
    type Runtime = Runtime;

    fn runtime(&self) -> &Self::Runtime {
        &Runtime
    }
}

impl arca::ValueType for Blob {
    const DATATYPE: DataType = DataType::Blob;
}

impl arca::Blob for Blob {
    fn read(&self, buffer: &mut [u8]) {
        match self {
            Blob::Raw(items) => buffer.copy_from_slice(items),
            Blob::String(s) => buffer.copy_from_slice(s.as_bytes()),
        }
    }

    fn len(&self) -> usize {
        match self {
            Blob::Raw(items) => items.len(),
            Blob::String(s) => s.len(),
        }
    }
}

impl Deref for Blob {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self {
            Blob::Raw(items) => items,
            Blob::String(s) => s.as_bytes(),
        }
    }
}

impl DerefMut for Blob {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.make_blob()
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

impl From<&str> for Blob {
    fn from(value: &str) -> Self {
        Blob::from(value.to_string())
    }
}

impl TryFrom<Handle> for Blob {
    type Error = Handle;

    fn try_from(value: Handle) -> Result<Self, Self::Error> {
        if value.datatype() == <Self as arca::ValueType>::DATATYPE {
            let raw = core::ptr::from_raw_parts_mut(value.read().0 as *mut (), value.read().1);
            unsafe { Ok(Blob::Raw(Box::from_raw(raw))) }
        } else {
            Err(value)
        }
    }
}

impl From<Blob> for Handle {
    fn from(value: Blob) -> Self {
        let value = value.into_inner();
        let raw = Box::into_raw(value);
        let (ptr, len) = raw.to_raw_parts();
        Handle::new(DataType::Blob, (ptr as usize, len))
    }
}
