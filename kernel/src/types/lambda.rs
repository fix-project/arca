use common::message::Handle;

use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lambda {
    pub arca: Box<Arca>,
}

impl Lambda {
    pub fn new<T: Into<Box<Arca>>>(arca: T) -> Lambda {
        Lambda { arca: arca.into() }
    }
}

impl arca::RuntimeType for Lambda {
    type Runtime = Runtime;

    fn runtime(&self) -> &Self::Runtime {
        &Runtime
    }
}

impl arca::ValueType for Lambda {
    const DATATYPE: DataType = DataType::Lambda;
}

impl arca::Lambda for Lambda {
    fn read(self) -> (arca::associated::Thunk<Self>, usize) {
        todo!()
    }
}

impl TryFrom<Handle> for Lambda {
    type Error = Handle;

    fn try_from(value: Handle) -> Result<Self, Self::Error> {
        if value.datatype() == <Self as arca::ValueType>::DATATYPE {
            let (raw, _) = value.read();
            unsafe {
                Ok(Lambda {
                    arca: Box::from_raw(raw as *mut _),
                })
            }
        } else {
            Err(value)
        }
    }
}

impl From<Lambda> for Handle {
    fn from(value: Lambda) -> Self {
        let ptr = Box::into_raw(value.arca);
        Handle::new(DataType::Lambda, (ptr as usize, 0))
    }
}
