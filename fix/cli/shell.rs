use crate::{Client, Error, Options, Value as RawValue};
use fix::{DataType, EncodeType, FixShell, ValueType};
use std::{cell::RefCell, path::Path};

std::thread_local! {
    static CURRENT: RefCell<Option<Client>> = const { RefCell::new(None) };
}

pub struct Shell(std::marker::PhantomData<std::rc::Rc<()>>);
pub type Value = fix::Value<Shell>;
pub type Blob = fix::Blob<Shell>;
pub type Tree = fix::Tree<Shell>;
pub type Object = fix::Object<Shell>;
pub type Reference = fix::Reference<Shell>;
pub type Thunk = fix::Thunk<Shell>;
pub type Encode = fix::Encode<Shell>;

impl Shell {
    fn with<R>(function: impl FnOnce(&mut Client) -> R) -> R {
        CURRENT.with(|current| {
            function(
                current
                    .borrow_mut()
                    .as_mut()
                    .expect("Fix shell is not initialized"),
            )
        })
    }
    pub fn open(directory: &Path, options: Options) -> Result<Self, Error> {
        let client = Client::open(directory, options)?;
        CURRENT.with(|current| {
            let mut current = current.borrow_mut();
            assert!(current.is_none(), "Fix shell is already initialized");
            *current = Some(client);
        });
        Ok(Self(std::marker::PhantomData))
    }
    pub fn close(self) -> Result<(), Error> {
        CURRENT
            .with(|current| current.borrow_mut().take())
            .expect("Fix shell is not initialized")
            .close()
    }
    pub fn reuse_vm(&mut self) {
        Self::with(Client::reuse_vm)
    }
    pub fn begin_request(&mut self) {
        Self::with(Client::begin_request)
    }
    pub fn end_request(&mut self) -> Result<(), Error> {
        Self::with(Client::end_request)
    }
    pub fn set_profile(&mut self, enabled: bool) {
        Self::with(|client| client.set_profile(enabled))
    }
    pub fn options(&self) -> Options {
        Self::with(|client| client.options().clone())
    }
    pub fn repository(&self) -> String {
        Self::with(|client| client.repository().to_owned())
    }
    pub fn label_value(&mut self, name: &str) -> Result<Value, Error> {
        Self::with(|client| client.label_value(name)).map(Value::from_inner)
    }
    pub fn lookup(&mut self, name: &str) -> Result<Value, Error> {
        Self::with(|client| client.lookup(name)).map(Value::from_inner)
    }
    pub fn save(&mut self, value: &Value) -> Result<String, Error> {
        Self::with(|client| client.save(value.inner()))
    }
    pub fn label(&mut self, name: &str, value: &Value) -> Result<(), Error> {
        Self::with(|client| client.label(name, value.inner()))
    }
    pub fn call(&mut self, value: &Value) -> Result<Value, Error> {
        Self::with(|client| client.call(value.inner())).map(Value::from_inner)
    }
    pub fn schedule(&mut self, value: &Value) -> Result<Value, Error> {
        Self::with(|client| client.schedule(value.inner())).map(Value::from_inner)
    }
    pub fn file(&mut self, path: &Path) -> Result<Value, Error> {
        Self::with(|client| client.file(path)).map(Value::from_inner)
    }
    pub fn compilation(&mut self, module: &Value, units: Option<u64>) -> Result<Value, Error> {
        Self::with(|client| client.compilation(module.inner(), units)).map(Value::from_inner)
    }
    pub fn precompile(&mut self, requests: &[Value]) -> Result<(), Error> {
        let requests: Vec<_> = requests.iter().map(|value| value.inner().clone()).collect();
        Self::with(|client| client.precompile(&requests))
    }
    pub fn compilation_request(
        &mut self,
        module: &Value,
        units: Option<u64>,
    ) -> Result<Value, Error> {
        Self::with(|client| client.compilation_request(module.inner(), units))
            .map(Value::from_inner)
    }
    pub fn think(&mut self, value: &Value) -> Result<Value, Error> {
        Self::with(|client| client.think(value.inner())).map(Value::from_inner)
    }
    pub fn todo_import(&mut self, uri: &str) -> Result<Value, Error> {
        Self::with(|client| client.todo_import(uri)).map(Value::from_inner)
    }
    pub fn todo_add(&mut self, value: &Value, uris: &[String]) -> Result<(), Error> {
        Self::with(|client| client.todo_add(value.inner(), uris))
    }
    pub fn todo_list(&mut self, value: &Value) -> Result<Vec<Vec<u8>>, Error> {
        Self::with(|client| client.todo_list(value.inner()))
    }
}
impl Drop for Shell {
    fn drop(&mut self) {
        CURRENT.with(|current| {
            current.borrow_mut().take();
        });
    }
}
impl FixShell for Shell {
    type Value = RawValue;
    type Error = Error;
    fn blob(bytes: &[u8]) -> RawValue {
        Self::with(|context| context.blob(bytes)).expect("valid Fix value")
    }
    fn tree(children: &[RawValue]) -> RawValue {
        Self::with(|context| context.tree(children)).expect("valid Fix value")
    }
    fn get(tree: &RawValue, index: usize) -> Result<RawValue, Error> {
        Self::with(|context| context.get(tree, index))
    }
    fn read(blob: &RawValue, offset: usize, bytes: &mut [u8]) -> Result<(), Error> {
        Self::with(|context| context.read(blob, offset, bytes))
    }
    fn len(value: &RawValue) -> u64 {
        Self::with(|context| context.len(value)).expect("valid Fix value")
    }
    fn value_type(value: &RawValue) -> ValueType {
        Self::with(|context| context.value_type(value)).expect("valid Fix value")
    }
    fn data_type(value: &RawValue) -> DataType {
        Self::with(|context| context.data_type(value)).expect("valid Fix value")
    }
    fn encode_type(value: &RawValue) -> EncodeType {
        Self::with(|context| context.encode_type(value)).expect("valid Fix value")
    }
    fn is_tag(value: &RawValue) -> bool {
        Self::with(|context| context.is_tag(value)).expect("valid Fix value")
    }
    fn is_comparable(value: &RawValue) -> bool {
        Self::with(|context| context.is_comparable(value)).expect("valid Fix value")
    }
    fn equals(a: &RawValue, b: &RawValue) -> Result<bool, Error> {
        Self::with(|context| context.equals(a, b))
    }
    fn aliases(left: &RawValue, right: &RawValue) -> bool {
        Self::with(|context| context.aliases(left, right))
    }
    fn reference(value: &RawValue) -> RawValue {
        Self::with(|context| context.reference(value)).expect("valid Fix value")
    }
    fn identification(value: &RawValue) -> RawValue {
        Self::with(|context| context.identification(value)).expect("valid Fix value")
    }
    fn application(value: &RawValue) -> RawValue {
        Self::with(|context| context.application(value)).expect("valid Fix value")
    }
    fn selection(value: &RawValue) -> RawValue {
        Self::with(|context| context.selection(value)).expect("valid Fix value")
    }
    fn strict(value: &RawValue) -> RawValue {
        Self::with(|context| context.strict(value)).expect("valid Fix value")
    }
    fn shallow(value: &RawValue) -> RawValue {
        Self::with(|context| context.shallow(value)).expect("valid Fix value")
    }
    fn eval(value: &RawValue) -> Result<RawValue, Error> {
        Self::with(|context| context.eval(value))
    }
}
