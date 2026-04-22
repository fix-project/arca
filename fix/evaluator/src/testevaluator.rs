extern crate alloc;
use crate::mockruntime::MockRuntime;
use alloc::{boxed::Box, vec::Vec};

#[derive(Debug)]
pub enum Error {
    OOB,
    TypeMismatch,
    UnexpectedFunction,
}

enum Object {
    Blob(Box<[u8]>),
    Tree(Box<[usize]>),
}

pub struct FakeRuntime {
    store: Vec<Object>,
}

impl FakeRuntime {
    fn new() -> Self {
        Self { store: Vec::new() }
    }

    fn create_blob_i64(&mut self, data: i64) -> <FakeRuntime as MockRuntime>::Handle {
        self.create_blob(Box::from(data.to_le_bytes()))
    }
}

impl MockRuntime for FakeRuntime {
    type Handle = usize;
    type BlobData = Box<[u8]>;
    type TreeData = Box<[usize]>;
    type Error = Error;

    fn create_blob(&mut self, data: Self::BlobData) -> Self::Handle {
        let index = self.store.len();
        self.store.push(Object::Blob(data));
        index
    }

    fn create_tree(&mut self, data: Self::TreeData) -> Self::Handle {
        let index = self.store.len();
        self.store.push(Object::Tree(data));
        index
    }

    fn get_blob(&self, handle: &Self::Handle) -> Result<Self::BlobData, Self::Error> {
        match self.store.get(*handle) {
            Some(Object::Blob(data)) => Ok(data.clone()),
            Some(Object::Tree(_)) => Err(Error::TypeMismatch),
            None => Err(Error::OOB),
        }
    }

    fn get_tree(&self, handle: &Self::Handle) -> Result<Self::TreeData, Self::Error> {
        match self.store.get(*handle) {
            Some(Object::Tree(data)) => Ok(data.clone()),
            Some(Object::Blob(_)) => Err(Error::TypeMismatch),
            None => Err(Error::OOB),
        }
    }

    fn apply(&mut self, combination: &Self::Handle) -> Result<Self::Handle, Self::Error> {
        let span = self.get_tree(combination)?;
        if span.len() < 2 {
            return Err(Error::OOB);
        }

        let function = self.get_blob(&span[0])?;
        if function.as_ref() != b"+" {
            return Err(Error::UnexpectedFunction);
        }

        let left_bytes: [u8; 8] = (*self.get_blob(&span[1])?)
            .try_into()
            .map_err(|_| Error::OOB)?;
        let left = i64::from_le_bytes(left_bytes);
        let right_bytes: [u8; 8] = (*self.get_blob(&span[2])?)
            .try_into()
            .map_err(|_| Error::OOB)?;
        let right = i64::from_le_bytes(right_bytes);

        Ok(self.create_blob_i64(left + right))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add() {
        let mut fake_runtime = FakeRuntime::new();

        let one_literal = fake_runtime.create_blob_i64(1);
        let two_literal = fake_runtime.create_blob_i64(2);
        let plus_literal = fake_runtime.create_blob(Box::from(*b"+"));

        let tree = fake_runtime.create_tree(Box::from([plus_literal, one_literal, two_literal]));
        let result = fake_runtime.apply(&tree).expect("valid apply addition");

        assert_eq!(
            fake_runtime.get_blob(&result).expect("valid result blob"),
            Box::from(3_i64.to_le_bytes())
        );
    }
}
