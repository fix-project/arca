use alloc::rc::Rc;
use core::cell::Cell;
use fix::{Any, DataType, EncodeType, Error, Path, Value, ValueType};

unsafe extern "C" {
    fn fix_json_cache_prepare(length: u32) -> u32;
    fn fix_json_cache_load(index: u32);
    fn fix_json_cache_save(index: u32);
}

pub fn prepare(length: usize) -> Result<(), Error> {
    if unsafe { fix_json_cache_prepare(length as u32) } == 0 {
        Ok(())
    } else {
        Err(Error::GrowFailed)
    }
}

pub struct Node<'a> {
    pub index: u32,
    pub built: Cell<bool>,
    pub value: Rc<dyn Value<Type = Any> + 'a>,
}

impl Value for Node<'_> {
    type Type = Any;

    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        if path.is_some() {
            return self.value.build_at(path);
        }
        if self.built.get() {
            unsafe { fix_json_cache_load(self.index) };
        } else {
            self.value.build_at(None)?;
            unsafe { fix_json_cache_save(self.index) };
            self.built.set(true);
        }
        Ok(())
    }

    fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
        self.value.value_type_at(path)
    }

    fn data_type_at(&self, path: Option<&Path<'_>>) -> Result<DataType, Error> {
        self.value.data_type_at(path)
    }

    fn encode_type_at(&self, path: Option<&Path<'_>>) -> Result<EncodeType, Error> {
        self.value.encode_type_at(path)
    }

    fn len_at(&self, path: Option<&Path<'_>>) -> Result<u64, Error> {
        self.value.len_at(path)
    }

    fn is_tag_at(&self, path: Option<&Path<'_>>) -> Result<bool, Error> {
        self.value.is_tag_at(path)
    }

    fn read_at(
        &self,
        path: Option<&Path<'_>>,
        offset: usize,
        bytes: &mut [u8],
    ) -> Result<(), Error> {
        self.value.read_at(path, offset, bytes)
    }
}
