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
