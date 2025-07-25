use crate::prelude::*;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Atom {
    hash: Box<[u8; 32]>,
}

impl Atom {
    pub fn new<T: AsRef<[u8]>>(x: T) -> Self {
        let data = x.as_ref();
        let hash = blake3::hash(data);
        Atom {
            hash: Box::new(hash.into()),
        }
    }
}
