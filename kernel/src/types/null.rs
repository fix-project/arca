use crate::prelude::*;
use common::message::Handle;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Null;

impl Null {
    pub fn new() -> Self {
        Null
    }
}

impl Default for Null {
    fn default() -> Self {
        Self::new()
    }
}

impl arca::RuntimeType for Null {
    type Runtime = super::Runtime;

    fn runtime(&self) -> &Self::Runtime {
        &Runtime
    }
}

impl arca::ValueType for Null {
    const DATATYPE: DataType = DataType::Null;
}

impl arca::Null for Null {}

impl TryFrom<Handle> for Null {
    type Error = Handle;

    fn try_from(value: Handle) -> Result<Self, Self::Error> {
        if value.datatype() == <Self as arca::ValueType>::DATATYPE {
            Ok(Null)
        } else {
            Err(value)
        }
    }
}

impl From<Null> for Handle {
    fn from(_: Null) -> Self {
        Handle::new(DataType::Null, (0, 0))
    }
}
