#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

use crate::{
    data::{BlobData, TreeData},
    runtime::{DeterministicEquivRuntime, ExecutionRuntime},
    storage::ObjectStore,
};
use arca::Runtime;
use bytemuck::bytes_of;
use fixhandle::rawhandle::FixHandle;

#[derive(Debug)]
pub enum Error {
    OOB,
    TypeMismatch,
}

#[derive(Default, Debug)]
pub struct Evaluator {
    store: ObjectStore,
}

impl Evaluator {
    fn new() -> Self {
        Self::default()
    }
}

impl DeterministicEquivRuntime for Evaluator {
    type BlobData = BlobData;
    type TreeData = TreeData;
    type Handle = FixHandle;
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

    fn is_blob(handle: &mut Self::Handle) -> bool {
        matches!(handle, Handle::BlobObject(_))
    }

    fn is_tree(handle: &mut Self::Handle) -> bool {
        matches!(handle, Handle::TreeObject(_))
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
