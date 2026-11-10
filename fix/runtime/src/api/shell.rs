extern crate alloc;

use super::{Backend, application, identification, reference, selection};
use crate::handle::{self, EncodeView, Handle, HandleView, Thunk as RawThunk};
use crate::storage::Storage;
use alloc::string::String;
use fix::{DataType, EncodeType, FixShell, ValueType};
use kernel::prelude::{Arc, SpinLock};

#[derive(Debug)]
pub enum Error {
    Backend(String),
    Execution(handle::Tree),
}

pub struct Shell;
pub type Value = fix::Value<Shell>;
pub type Blob = fix::Blob<Shell>;
pub type Tree = fix::Tree<Shell>;
pub type Object = fix::Object<Shell>;
pub type Reference = fix::Reference<Shell>;
pub type Thunk = fix::Thunk<Shell>;
pub type Encode = fix::Encode<Shell>;

#[derive(Clone)]
struct Context {
    backend: Arc<Backend>,
    scheduler: handle::Tree,
    collector: handle::Tree,
}
static CURRENT: SpinLock<Option<Context>> = SpinLock::new(None);

impl Shell {
    pub fn initialize(backend: Arc<Backend>, scheduler: handle::Tree, collector: handle::Tree) {
        let mut current = CURRENT.lock();
        assert!(current.is_none(), "Fix shell is already initialized");
        *current = Some(Context {
            backend,
            scheduler,
            collector,
        });
    }
    fn with<R>(function: impl FnOnce(&Context) -> R) -> R {
        let context = CURRENT
            .lock()
            .as_ref()
            .expect("Fix shell is not initialized")
            .clone();
        function(&context)
    }
}

impl FixShell for Shell {
    type Value = Handle;
    type Error = Error;
    fn blob(bytes: &[u8]) -> Handle {
        Self::with(|context| context.backend.blob(bytes))
    }
    fn tree(children: &[Handle]) -> Handle {
        Self::with(|context| context.backend.tree(children))
    }
    fn get(tree: &Handle, index: usize) -> Result<Handle, Error> {
        Self::with(|context| context.backend.get(*tree, index).map_err(Error::Backend))
    }
    fn read(blob: &Handle, offset: usize, bytes: &mut [u8]) -> Result<(), Error> {
        Self::with(|context| {
            context
                .backend
                .read(*blob, offset, bytes)
                .map_err(Error::Backend)
        })
    }
    fn len(value: &Handle) -> u64 {
        assert!(matches!(
            value.view(),
            HandleView::Object(_) | HandleView::Ref(_)
        ));
        value.len() as u64
    }
    fn value_type(value: &Handle) -> ValueType {
        match value.view() {
            HandleView::Object(_) => ValueType::Object,
            HandleView::Ref(_) => ValueType::Ref,
            HandleView::Thunk(_) => ValueType::Thunk,
            HandleView::Encode(_) => ValueType::Encode,
        }
    }
    fn data_type(value: &Handle) -> DataType {
        assert!(matches!(
            value.view(),
            HandleView::Object(_) | HandleView::Ref(_)
        ));
        if value.is_tree_data() {
            DataType::Tree
        } else {
            DataType::Blob
        }
    }
    fn encode_type(value: &Handle) -> EncodeType {
        match handle::Encode::try_from(*value).expect("Encode").view() {
            EncodeView::Strict(_) => EncodeType::Strict,
            EncodeView::Shallow(_) => EncodeType::Shallow,
        }
    }
    fn is_tag(value: &Handle) -> bool {
        handle::Tree::try_from(*value).is_ok_and(|tree| tree.is_tag())
    }
    fn is_comparable(value: &Handle) -> bool {
        value.is_comparable()
    }
    fn equals(left: &Handle, right: &Handle) -> Result<bool, Error> {
        Self::with(|context| {
            context
                .backend
                .storage
                .equals(*left, *right)
                .map_err(|error| Error::Backend(super::debug(error)))
        })
    }
    fn aliases(left: &Handle, right: &Handle) -> bool {
        left.as_bytes() == right.as_bytes()
    }
    fn reference(value: &Handle) -> Handle {
        reference(*value).expect("Object").into()
    }
    fn identification(value: &Handle) -> Handle {
        identification(*value).expect("Object or Ref")
    }
    fn application(value: &Handle) -> Handle {
        application(*value).expect("Tree")
    }
    fn selection(value: &Handle) -> Handle {
        selection(*value).expect("Tree")
    }
    fn strict(value: &Handle) -> Handle {
        RawThunk::try_from(*value).expect("Thunk").strict().into()
    }
    fn shallow(value: &Handle) -> Handle {
        RawThunk::try_from(*value).expect("Thunk").shallow().into()
    }
    fn eval(value: &Handle) -> Result<Handle, Error> {
        Self::with(|context| {
            context
                .backend
                .eval(*value, context.scheduler, context.collector)
                .map_err(Error::Backend)?
                .map_err(Error::Execution)
        })
    }
}
