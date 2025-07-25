use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Exception {
    value: Box<Value>,
}

impl Exception {
    pub fn new(value: Value) -> Self {
        Exception {
            value: value.into(),
        }
    }

    pub fn inner(&self) -> &Value {
        &self.value
    }

    pub fn into_inner(self) -> Value {
        *self.value
    }

    pub fn read(self) -> Value {
        self.into_inner()
    }
}
