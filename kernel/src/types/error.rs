use common::message::Handle;

use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    value: Box<Value>,
}

impl Error {
    pub fn new(value: Value) -> Self {
        Error {
            value: value.into(),
        }
    }
}

impl arca::RuntimeType for Error {
    type Runtime = Runtime;
}

impl arca::ValueType for Error {
    const DATATYPE: DataType = DataType::Error;
}

impl arca::Error for Error {
    fn read(self) -> Value {
        *self.value
    }
}

impl TryFrom<Handle> for Error {
    type Error = Handle;

    fn try_from(value: Handle) -> Result<Self, Self::Error> {
        if value.datatype() == <Self as arca::ValueType>::DATATYPE {
            let raw = value.read().0;
            unsafe {
                Ok(Error {
                    value: Box::from_raw(raw as *mut _),
                })
            }
        } else {
            Err(value)
        }
    }
}

impl From<Error> for Handle {
    fn from(value: Error) -> Self {
        let raw = Box::into_raw(value.value);
        Handle::new(DataType::Error, (raw as usize, 0))
    }
}
