use common::message::Handle;

use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lambda {
    pub arca: Box<Arca>,
    pub idx: usize,
}

impl Lambda {
    pub fn new<T: Into<Box<Arca>>>(arca: T, idx: usize) -> Lambda {
        Lambda {
            arca: arca.into(),
            idx,
        }
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
    fn apply(self, argument: arca::associated::Value<Self>) -> arca::associated::Thunk<Self> {
        let mut arca = self.arca;
        let idx = self.idx;
        arca.descriptors_mut()[idx] = argument;
        Thunk::new(arca)
    }

    fn read(self) -> (arca::associated::Thunk<Self>, usize) {
        todo!()
    }
}

impl TryFrom<Handle> for Lambda {
    type Error = Handle;

    fn try_from(value: Handle) -> Result<Self, Self::Error> {
        if value.datatype() == <Self as arca::ValueType>::DATATYPE {
            let (raw, idx) = value.read();
            unsafe {
                Ok(Lambda {
                    arca: Box::from_raw(raw as *mut _),
                    idx,
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
        Handle::new(DataType::Lambda, (ptr as usize, value.idx))
    }
}
