use common::message::Handle;

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

impl arca::RuntimeType for Atom {
    type Runtime = Runtime;
}

impl arca::ValueType for Atom {
    const DATATYPE: DataType = DataType::Atom;
}

impl arca::Atom for Atom {}

impl TryFrom<Handle> for Atom {
    type Error = Handle;

    fn try_from(value: Handle) -> Result<Self, Self::Error> {
        if value.datatype() == <Self as arca::ValueType>::DATATYPE {
            let raw = value.read().0;
            unsafe {
                Ok(Atom {
                    hash: Box::from_raw(raw as *mut _),
                })
            }
        } else {
            Err(value)
        }
    }
}

impl From<Atom> for Handle {
    fn from(value: Atom) -> Self {
        let raw = Box::into_raw(value.hash);
        Handle::new(DataType::Error, (raw as usize, 0))
    }
}
