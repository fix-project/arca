use crate::{
    Any, Blob as RawBlob, Error, Handle, Object as RawObject, Thunk as RawThunk, Tree as RawTree,
    Value as AsValue,
};
use fix::{DataType, EncodeType, FixShell, Invocation, ValueType};

pub struct Shell;
pub type Value = fix::Value<Shell>;
pub type Blob = fix::Blob<Shell>;
pub type Tree = fix::Tree<Shell>;
pub type Object = fix::Object<Shell>;
pub type Reference = fix::Reference<Shell>;
pub type Thunk = fix::Thunk<Shell>;
pub type Encode = fix::Encode<Shell>;

static mut COMBINATION: Option<u32> = None;
pub(crate) struct InvocationGuard;
impl InvocationGuard {
    pub(crate) fn new(combination: u32) -> Self {
        unsafe {
            assert!((&raw const COMBINATION).read().is_none());
            (&raw mut COMBINATION).write(Some(combination));
        }
        Self
    }
}
impl Drop for InvocationGuard {
    fn drop(&mut self) {
        unsafe { (&raw mut COMBINATION).write(None) };
    }
}

impl FixShell for Shell {
    type Value = Handle<Any>;
    type Error = Error;
    fn advise(value: &Self::Value, key: &str, payload: &[u8]) {
        crate::advise(value, key, payload).expect("advice exceeds address space")
    }
    fn blob(bytes: &[u8]) -> Self::Value {
        RawBlob::create(bytes).into_any()
    }
    fn tree(children: &[Self::Value]) -> Self::Value {
        RawTree::create(children).into_any()
    }
    fn get(tree: &Self::Value, index: usize) -> Result<Self::Value, Error> {
        Ok(Handle::<RawObject<RawTree>>::try_from(tree)?.get(index))
    }
    fn read(blob: &Self::Value, offset: usize, bytes: &mut [u8]) -> Result<(), Error> {
        blob.read_into(offset, bytes)
    }
    fn len(value: &Self::Value) -> u64 {
        value.len().expect("Object or Ref")
    }
    fn value_type(value: &Self::Value) -> ValueType {
        value.value_type().expect("Fix value")
    }
    fn data_type(value: &Self::Value) -> DataType {
        value.data_type().expect("Object or Ref")
    }
    fn encode_type(value: &Self::Value) -> EncodeType {
        value.encode_type().expect("Encode")
    }
    fn is_tag(value: &Self::Value) -> bool {
        value.is_tag().expect("Fix value")
    }
    fn is_comparable(value: &Self::Value) -> bool {
        value.is_comparable().expect("Fix value")
    }
    fn equals(left: &Self::Value, right: &Self::Value) -> Result<bool, Error> {
        left.equals(right)
    }
    fn aliases(left: &Self::Value, right: &Self::Value) -> bool {
        left.as_raw() == right.as_raw()
    }
    fn reference(value: &Self::Value) -> Self::Value {
        Handle::<RawObject<Any>>::try_from(value)
            .expect("Object")
            .reference()
            .into_any()
    }
    fn identification(value: &Self::Value) -> Self::Value {
        value.identification().into_any()
    }
    fn application(value: &Self::Value) -> Self::Value {
        value.application().into_any()
    }
    fn selection(value: &Self::Value) -> Self::Value {
        value.selection().into_any()
    }
    fn strict(value: &Self::Value) -> Self::Value {
        Handle::<RawThunk>::try_from(value)
            .expect("Thunk")
            .strict()
            .into_any()
    }
    fn shallow(value: &Self::Value) -> Self::Value {
        Handle::<RawThunk>::try_from(value)
            .expect("Thunk")
            .shallow()
            .into_any()
    }
    fn eval(value: &Self::Value) -> Result<Self::Value, Error> {
        value.eval()
    }
}
impl Invocation for Shell {
    fn combination() -> Self::Value {
        let index = unsafe { (&raw const COMBINATION).read() }.expect("no Fix invocation");
        unsafe { Handle::from_raw(crate::ffi::clone(index)) }
    }
    fn tag(fields: &[Self::Value]) -> Result<Self::Value, Error> {
        Ok(RawTree::create(fields).tag()?.into_any())
    }
    fn effect(combination: &Self::Value) -> Self::Value {
        unsafe { Handle::from_raw(crate::ffi::effect(combination.as_raw())) }
    }
}
