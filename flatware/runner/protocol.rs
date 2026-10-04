#![no_std]
extern crate alloc;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Object {
    Blob(Vec<u8>),
    Tree(Vec<Object>),
}

impl Object {
    pub fn blob(bytes: impl AsRef<[u8]>) -> Self {
        Self::Blob(bytes.as_ref().to_vec())
    }
    pub fn integer(value: u64) -> Self {
        Self::blob(value.to_le_bytes())
    }
    pub fn tree(&self) -> Option<&[Self]> {
        if let Self::Tree(children) = self {
            Some(children)
        } else {
            None
        }
    }
    pub fn bytes(&self) -> Option<&[u8]> {
        if let Self::Blob(bytes) = self {
            Some(bytes)
        } else {
            None
        }
    }
}
