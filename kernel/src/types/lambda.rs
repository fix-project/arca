use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lambda {
    pub arca: Arca,
    pub idx: usize,
}

impl Lambda {
    pub fn apply<T: Into<Value>>(self, x: T) -> Thunk {
        Thunk::new(self, x)
    }
}
