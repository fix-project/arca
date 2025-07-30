use core::ops::{Deref, DerefMut};

use crate::prelude::*;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Blob {
    Raw(Box<[u8]>),
    String(String),
}

impl Blob {
    pub fn new<T: Into<Box<[u8]>>>(x: T) -> Self {
        let x = x.into();
        match String::from_utf8(x.into()) {
            Ok(x) => Blob::String(x),
            Err(e) => Blob::Raw(e.into_bytes().into()),
        }
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

    pub fn len(&self) -> usize {
        match self {
            Blob::Raw(items) => items.len(),
            Blob::String(s) => s.len(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
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
