#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

use crate::runtime::{DeterministicEquivRuntime, ExecutionRuntime};
use arca::Runtime;
use bytemuck::bytes_of;
use kernel::types::{Tuple, Value, Word};

#[derive(Debug)]
pub enum Error {
    OOB,
    TypeMismatch,
}

include!(concat!(env!("OUT_DIR"), "/handle-bindings.rs"));

type BlobData = Value;
type TreeData = Tuple;

#[derive(Clone, Debug)]
pub enum Handle {
    BlobObject(BlobData),
    TreeObject(TreeData),
}

impl Handle {
    fn to_fix_type(&self) -> Word {
        match self {
            Handle::BlobObject(_) => Runtime::create_word(fix_type::BlobObject.into()),
            Handle::TreeObject(_) => Runtime::create_word(fix_type::TreeObject.into()),
        }
    }

    fn to_raw_value(&self) -> Value {
        match self {
            Handle::BlobObject(blob) => blob.clone(),
            Handle::TreeObject(tree) => Value::Tuple(tree.clone()),
        }
    }

    fn to_arca_tuple(&self) -> Value {
        let mut t = Runtime::create_tuple(2);
        t.set(0, self.to_fix_type());
        t.set(1, self.to_raw_value());
        Value::Tuple(t)
    }

    fn from_raw_type(fix_type: &Value) -> Result<u64, Error> {
        match fix_type {
            Value::Word(w) => Ok(w.read()),
            _ => Err(Error::TypeMismatch),
        }
    }

    fn from_raw_parts(fix_type: &Value, data: Value) -> Result<Self, Error> {
        match Self::from_raw_type(fix_type)? as u32 {
            fix_type::BlobObject => Ok(Handle::BlobObject(data)),
            fix_type::TreeObject => match data {
                Value::Tuple(t) => Ok(Handle::TreeObject(t)),
                _ => Err(Error::TypeMismatch),
            },
            _ => Err(Error::TypeMismatch),
        }
    }

    fn from_raw_tuple(tuple: &Tuple) -> Result<Self, Error> {
        Self::from_raw_parts(&tuple.get(0), tuple.get(1))
    }
}

pub struct FixRuntime {}

impl FixRuntime {
    fn new() -> Self {
        Self {}
    }

    pub fn create_scrach_tree(length: usize) -> TreeData {
        Runtime::create_tuple(length * 2)
    }

    fn get_tree(handle: &Handle) -> Result<&TreeData, Error> {
        match handle {
            Handle::TreeObject(treedata) => Ok(treedata),
            _ => Err(Error::TypeMismatch),
        }
    }
}

impl DeterministicEquivRuntime for FixRuntime {
    type BlobData = BlobData;
    type TreeData = TreeData;
    type Handle = Handle;
    type Error = Error;

    fn create_blob_i64(data: u64) -> Self::Handle {
        Handle::BlobObject(Value::Word(Runtime::create_word(data)))
    }

    fn create_blob(data: Self::BlobData) -> Self::Handle {
        Handle::BlobObject(data)
    }

    fn create_tree(data: Self::TreeData) -> Self::Handle {
        Handle::TreeObject(data)
    }

    fn length(handle: &Self::Handle) -> Result<usize, Self::Error> {
        match handle {
            Handle::BlobObject(data) => match data {
                Value::Word(word) => Ok(word.len()),
                Value::Blob(blob) => Ok(blob.len()),
                Value::Page(page) => Ok(page.len()),
                _ => Err(Error::TypeMismatch),
            },
            Handle::TreeObject(tuple) => Ok(tuple.len() / 2),
        }
    }

    fn get_blob(handle: &Self::Handle) -> Result<&[u8], Self::Error> {
        match handle {
            Handle::BlobObject(v) => match v {
                Value::Word(w) => Ok(bytes_of(w.inner().as_ref())),
                Value::Blob(b) => Ok(b.inner().as_ref()),
                Value::Page(p) => Ok(p.inner().as_ref()),
                _ => Err(Error::TypeMismatch),
            },
            Handle::TreeObject(_) => Err(Error::TypeMismatch),
        }
    }

    fn set_tree_entry(
        data: &mut Self::TreeData,
        index: usize,
        handle: &Self::Handle,
    ) -> Result<Self::Handle, Self::Error> {
        let prev_type = data.set(index * 2, handle.to_fix_type());
        let prev_data = data.set(index * 2 + 1, handle.to_raw_value());
        Self::Handle::from_raw_parts(&prev_type, prev_data)
    }

    fn get_tree_entry(data: &Self::TreeData, index: usize) -> Result<Self::Handle, Self::Error> {
        let fix_type = data.get(index * 2);
        let data = data.get(index * 2 + 1);
        Self::Handle::from_raw_parts(&fix_type, data)
    }

    fn is_blob(handle: &mut Self::Handle) -> bool {
        matches!(handle, Handle::BlobObject(_))
    }

    fn is_tree(handle: &mut Self::Handle) -> bool {
        matches!(handle, Handle::TreeObject(_))
    }
}

impl ExecutionRuntime for FixRuntime {
    fn execute(combination: &Self::Handle) -> Result<Self::Handle, Self::Error> {
        let tree = Self::get_tree(combination)?;
        let function_handle = Self::get_tree_entry(tree, 1)?;
        let elf = Self::get_blob(&function_handle)?;

        let f = common::elfloader::load_elf(elf).expect("Failed to load elf");
        let f = Runtime::apply_function(f, combination.to_arca_tuple());

        let result = f.force().try_into().unwrap();

        Handle::from_raw_tuple(&result)
    }
}
