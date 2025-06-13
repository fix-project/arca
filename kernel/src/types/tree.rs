use core::ops::{Deref, DerefMut};

use common::message::Handle;

use crate::prelude::*;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Tree {
    contents: Box<[Value]>,
}

impl Tree {
    pub fn new<T: Into<Box<[Value]>>>(x: T) -> Self {
        Tree { contents: x.into() }
    }

    pub fn new_with_len(len: usize) -> Self {
        let v = vec![Value::Null; len];
        Tree { contents: v.into() }
    }

    pub fn into_inner(self) -> Box<[Value]> {
        self.contents
    }
}

impl arca::RuntimeType for Tree {
    type Runtime = Runtime;

    fn runtime(&self) -> &Self::Runtime {
        &Runtime
    }
}

impl arca::ValueType for Tree {
    const DATATYPE: DataType = DataType::Tree;
}

impl arca::Tree for Tree {
    fn take(&mut self, index: usize) -> arca::associated::Value<Self> {
        core::mem::take(&mut self.contents[index])
    }

    fn put(
        &mut self,
        index: usize,
        value: arca::associated::Value<Self>,
    ) -> arca::associated::Value<Self> {
        let old = core::mem::take(&mut self.contents[index]);
        self.contents[index] = value;
        old
    }

    fn get(&self, index: usize) -> arca::associated::Value<Self> {
        self.contents[index].clone()
    }

    fn set(&mut self, index: usize, value: arca::associated::Value<Self>) {
        self.contents[index] = value;
    }

    fn len(&self) -> usize {
        self.contents.len()
    }
}

impl Deref for Tree {
    type Target = [Value];

    fn deref(&self) -> &Self::Target {
        &self.contents
    }
}

impl DerefMut for Tree {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.contents
    }
}

impl FromIterator<Value> for Tree {
    fn from_iter<T: IntoIterator<Item = Value>>(iter: T) -> Self {
        let v: Box<[Value]> = iter.into_iter().collect();
        Tree::new(v)
    }
}

impl TryFrom<Handle> for Tree {
    type Error = Handle;

    fn try_from(value: Handle) -> Result<Self, Self::Error> {
        if value.datatype() == <Self as arca::ValueType>::DATATYPE {
            let raw = core::ptr::from_raw_parts_mut(value.read().0 as *mut (), value.read().1);
            unsafe {
                Ok(Tree {
                    contents: Box::from_raw(raw),
                })
            }
        } else {
            Err(value)
        }
    }
}

impl From<Tree> for Handle {
    fn from(value: Tree) -> Self {
        let raw = Box::into_raw(value.contents);
        let (ptr, len) = raw.to_raw_parts();
        Handle::new(DataType::Tree, (ptr as usize, len))
    }
}
