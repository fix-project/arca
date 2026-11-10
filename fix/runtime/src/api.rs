extern crate alloc;

pub mod shell;
pub use shell::{Error as ShellError, Shell};

use crate::{
    handle::*,
    repository::{Repository, RepositoryError},
    runtime::{
        InvalidApplication,
        arca::{EffectAction, FixOnArca, trust::Trust},
    },
    storage::{Storage, memory, repository::RepositoryStorage},
    user_scheduler_driver::Outcome,
};
use alloc::{boxed::Box, format, string::String, sync::Arc, vec::Vec};
use bytes::Buf;
use fix::Memoization;

pub type Result<T> = core::result::Result<T, String>;

pub struct Backend {
    storage: Arc<RepositoryStorage>,
    runtime: Arc<FixOnArca<Arc<RepositoryStorage>>>,
    profile: bool,
}

impl Backend {
    pub fn new(storage: Arc<RepositoryStorage>, trust: Trust, memoize: Memoization) -> Self {
        Self {
            runtime: Arc::new(FixOnArca::memoized(storage.clone(), memoize).with_trust(trust)),
            storage,
            profile: false,
        }
    }

    pub fn with_profile(mut self, enabled: bool) -> Self {
        self.profile = enabled;
        if enabled {
            self.runtime.enable_profile();
        }
        self
    }

    pub fn storage(&self) -> &Arc<RepositoryStorage> {
        &self.storage
    }

    pub fn memory(&self) -> &memory::MemoryStorage {
        AsRef::<memory::MemoryStorage>::as_ref(&*self.storage)
    }

    pub fn repository(&self) -> &Repository {
        self.storage.repository()
    }

    pub fn lookup(&self, name: &str) -> core::result::Result<Handle, RepositoryError> {
        resolve(self.repository(), name)
    }

    pub fn blob(&self, bytes: &[u8]) -> Handle {
        self.storage.add_blob(bytes).into()
    }

    pub fn tree(&self, children: &[Handle]) -> Handle {
        self.storage.add_tree(children).into()
    }

    pub fn get(&self, tree: Handle, index: usize) -> Result<Handle> {
        let tree = Tree::try_from(tree).map_err(debug)?;
        if index >= tree.len() {
            return Err("Tree index out of bounds".into());
        }
        let backing = self.storage.get_tree_backing(tree).map_err(debug)?;
        let (page, offset) = backing.page(index * HANDLE_SIZE);
        let mut bytes = [0; HANDLE_SIZE];
        assert_eq!(page.read(offset, &mut bytes), HANDLE_SIZE);
        unsafe { Handle::parse(bytes) }.map_err(debug)
    }

    pub fn read_blob(&self, blob: Handle) -> Result<Box<[u8]>> {
        self.storage
            .get_blob(Blob::try_from(blob).map_err(debug)?)
            .map_err(debug)
    }

    pub fn read(&self, blob: Handle, offset: usize, out: &mut [u8]) -> Result<()> {
        let blob = Blob::try_from(blob).map_err(debug)?;
        let end = offset.checked_add(out.len()).ok_or("Blob range overflow")?;
        if end > blob.len() {
            return Err("Blob range out of bounds".into());
        }
        if let Some(bytes) = blob.literal_bytes() {
            out.copy_from_slice(&bytes[offset..end]);
        } else {
            let mut backing = self
                .storage
                .get_blob_backing(blob)
                .map_err(debug)?
                .into_buf();
            backing.advance(offset);
            backing.copy_to_slice(out);
        }
        Ok(())
    }

    pub fn read_tree(&self, tree: Handle) -> Result<Box<[Handle]>> {
        self.storage
            .get_tree(Tree::try_from(tree).map_err(debug)?)
            .map_err(debug)
    }

    pub fn save(&self, value: Handle) -> core::result::Result<Handle, RepositoryError> {
        self.storage.save(value)
    }

    pub fn label(&self, name: &str, value: Handle) -> core::result::Result<(), RepositoryError> {
        let value = self.save(value)?;
        self.repository().write_label(name, value)
    }

    pub fn call(&self, combination: Tree) -> Outcome {
        self.runtime
            .execute_with_effects(combination, false, &mut |_| EffectAction::Unhandled)
            .map_err(|InvalidApplication| InvalidApplication::request(&*self.storage))
    }

    pub fn schedule(&self, combination: Tree, todos: Vec<(Ref, Blob)>) -> Result<Outcome> {
        let report = crate::user_scheduler_driver::start_profiled(
            self.runtime.clone(),
            combination,
            kernel::ncores(),
            crate::user_scheduler_driver::RepositoryServices {
                storage: self.storage.clone(),
                todos,
            },
            self.profile,
        )
        .map_err(String::from)?;
        Ok(report.result)
    }

    pub fn eval(&self, value: Handle, scheduler: Tree, collector: Tree) -> Result<Outcome> {
        let storage = &*self.storage;
        let mode = storage.add_blob(b"eval");
        let combination =
            storage.add_tree(&[mode.into(), scheduler.into(), collector.into(), value]);
        let result = match self.schedule(combination, Vec::new())? {
            Ok(result) => result,
            failure => return Ok(failure),
        };
        if self.runtime.memoize == Memoization::Never {
            return Ok(Ok(result));
        }
        let coupon = Tree::try_from(result).map_err(debug)?;
        if !coupon.is_tag() {
            return Err("scheduler did not return a coupon".into());
        }
        let fields = storage.get_tree(coupon).map_err(debug)?;
        if fields.len() != 2 {
            return Err("invalid evaluation coupon".into());
        }
        let author: Handle = storage.canonicalize(fields[0]).map_err(debug)?.into();
        let collector: Handle = storage
            .canonicalize(collector.into())
            .map_err(debug)?
            .into();
        let kernel: Handle = storage
            .canonicalize(storage.add_tree(&[]).into())
            .map_err(debug)?
            .into();
        if author.as_bytes() != collector.as_bytes() && author.as_bytes() != kernel.as_bytes() {
            return Err("evaluation coupon has untrusted assumptions".into());
        }
        let reference = TreeRef::try_from(fields[1]).map_err(debug)?;
        let claim = storage
            .get_tree(reference.object_descriptor())
            .map_err(debug)?;
        if claim.len() != 3
            || storage
                .get_blob(Blob::try_from(claim[0]).map_err(debug)?)
                .map_err(debug)?
                .as_ref()
                != 1u64.to_le_bytes()
        {
            return Err("invalid evaluation claim".into());
        }
        let actual: Handle = storage.canonicalize(claim[1]).map_err(debug)?.into();
        let expected: Handle = storage.canonicalize(value).map_err(debug)?.into();
        if actual.as_bytes() != expected.as_bytes() {
            return Err("evaluation coupon names another input".into());
        }
        Ok(Ok(claim[2]))
    }
}

pub fn resolve(
    repository: &Repository,
    name: &str,
) -> core::result::Result<Handle, RepositoryError> {
    if name.len() == 64 && name.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        let mut bytes = [0; HANDLE_SIZE];
        hex::decode_to_slice(name, &mut bytes)
            .map_err(|_| RepositoryError::Handle(Error::InvalidName))?;
        repository.lookup(&bytes)
    } else {
        repository.read_label(name)
    }
}

pub fn reference(value: Handle) -> core::result::Result<Ref, Error> {
    match value.view() {
        HandleView::Object(object) => Ok(object.into_ref()),
        _ => Ref::try_from(value),
    }
}

pub fn identification(value: Handle) -> core::result::Result<Handle, Error> {
    Ok(reference(value)?.identification().into())
}

pub fn application(value: Handle) -> core::result::Result<Handle, Error> {
    Ok(TreeRef::try_from(Handle::from(reference(value)?))?
        .application()
        .into())
}

pub fn selection(value: Handle) -> core::result::Result<Handle, Error> {
    Ok(TreeRef::try_from(Handle::from(reference(value)?))?
        .selection()
        .into())
}

pub(crate) fn debug(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
