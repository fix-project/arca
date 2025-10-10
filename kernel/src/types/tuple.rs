use core::ops::{Deref, DerefMut};

use crate::prelude::*;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Tuple {
    contents: Box<[Value]>,
}

impl Tuple {
    pub fn new<T: Into<Box<[Value]>>>(x: T) -> Self {
        Tuple { contents: x.into() }
    }

    pub fn new_with_len(len: usize) -> Self {
        let v = vec![Value::default(); len];
        Tuple { contents: v.into() }
    }

    pub fn into_inner(self) -> Box<[Value]> {
        self.contents
    }
}

impl Deref for Tuple {
    type Target = [Value];

    fn deref(&self) -> &Self::Target {
        &self.contents
    }
}

impl DerefMut for Tuple {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.contents
    }
}

impl FromIterator<Value> for Tuple {
    fn from_iter<T: IntoIterator<Item = Value>>(iter: T) -> Self {
        let v: Box<[Value]> = iter.into_iter().collect();
        Tuple::new(v)
    }
}
