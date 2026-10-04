use super::*;
#[derive(Clone, Copy)]
pub struct CreateBlob<B>(B);
impl<B: AsRef<[u8]>> Value for CreateBlob<B> {
    type Type = Object<Blob>;
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        at_root(path, ())?;
        let bytes = self.0.as_ref();
        status(unsafe { fix_sdk_blob(bytes.as_ptr(), bytes.len()) })
    }
    fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
        at_root(path, ValueType::Object)
    }
    fn data_type_at(&self, path: Option<&Path<'_>>) -> Result<DataType, Error> {
        at_root(path, DataType::Blob)
    }
    fn len_at(&self, path: Option<&Path<'_>>) -> Result<usize, Error> {
        at_root(path, self.0.as_ref().len())
    }
    fn read_at(&self, path: Option<&Path<'_>>, offset: usize, out: &mut [u8]) -> Result<(), Error> {
        at_root(path, ())?;
        let source = self
            .0
            .as_ref()
            .get(offset..)
            .and_then(|s| s.get(..out.len()))
            .ok_or(Error::OutOfBounds)?;
        out.copy_from_slice(source);
        Ok(())
    }
}
impl Blob {
    pub fn create<B: AsRef<[u8]>>(bytes: B) -> CreateBlob<B> {
        CreateBlob(bytes)
    }
}
macro_rules! integer {
    ($t:ty) => {
        impl Value for $t {
            type Type = Object<Blob>;
            delegate_value!(this, Blob::create(this.to_le_bytes()));
        }
    };
}
integer!(u64);
integer!(u32);
