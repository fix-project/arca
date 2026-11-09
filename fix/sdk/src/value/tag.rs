use super::*;

#[derive(Clone, Copy)]
pub struct CreateTag<C> {
    fields: CreateTree<C>,
}
impl<C: Children> CreateTree<C> {
    pub fn tag(self) -> Result<CreateTag<C>, Error> {
        if !self.get(0).is_comparable()? {
            return Err(Error::NotComparable);
        }
        Ok(CreateTag { fields: self })
    }
}
impl<C: Children> Value for CreateTag<C> {
    type Type = Object<Tree>;
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        if path.is_none() {
            self.fields.build(true)
        } else {
            self.fields.build_at(path)
        }
    }
    fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
        self.fields.value_type_at(path)
    }
    fn data_type_at(&self, path: Option<&Path<'_>>) -> Result<DataType, Error> {
        self.fields.data_type_at(path)
    }
    fn encode_type_at(&self, path: Option<&Path<'_>>) -> Result<EncodeType, Error> {
        self.fields.encode_type_at(path)
    }
    fn len_at(&self, path: Option<&Path<'_>>) -> Result<u64, Error> {
        self.fields.len_at(path)
    }
    fn is_tag_at(&self, path: Option<&Path<'_>>) -> Result<bool, Error> {
        if path.is_none() {
            Ok(true)
        } else {
            self.fields.is_tag_at(path)
        }
    }
    fn read_exact_at(&self, path: Option<&Path<'_>>, bytes: &mut [u8]) -> Result<(), Error> {
        self.fields.read_exact_at(path, bytes)
    }
    fn read_at(
        &self,
        path: Option<&Path<'_>>,
        offset: usize,
        bytes: &mut [u8],
    ) -> Result<(), Error> {
        self.fields.read_at(path, offset, bytes)
    }
}
impl<C: Children, V: Value> PartialEq<V> for CreateTag<C> {
    fn eq(&self, other: &V) -> bool {
        equal(self, other)
    }
}
