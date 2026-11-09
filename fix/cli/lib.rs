pub use fixapi::Memoization;
use fixapi::{DataType, EncodeType, Fix, ValueType};
use fixhandle::{Encode, EncodeView, Handle, HandleView, Object, Ref, Thunk, Tree, TreeRef};
use fixrepository::Repository;
use fixstorage::{Storage, repository::RepositoryStorage};
use std::{
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

mod archive;
pub mod commands;
pub mod expressions;
mod session;
mod trust;

#[derive(Debug)]
pub enum Error {
    Backend(String),
    Execution(u8),
    /// The evaluation failed with the `Fix.error` request carrying this message.
    Fix(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(message) => f.write_str(message),
            Self::Execution(status) => write!(f, "Fix execution exited with status {status}"),
            Self::Fix(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for Error {}
fn error(value: impl fmt::Debug) -> Error {
    Error::Backend(format!("{value:?}"))
}
fn message(text: &str) -> Error {
    Error::Backend(text.into())
}

#[derive(Clone)]
pub struct Value {
    handle: Handle,
    storage: Arc<RepositoryStorage>,
}

#[derive(Clone)]
pub struct Options {
    pub scheduler: String,
    pub collector: String,
    pub trust: Option<PathBuf>,
    pub jobs: usize,
    pub memory_mib: usize,
    pub memoize: Memoization,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            scheduler: "scheduler".into(),
            collector: "collector".into(),
            trust: None,
            jobs: 1,
            memory_mib: 16384,
            memoize: Memoization::Auto,
        }
    }
}

pub struct Client {
    storage: Arc<RepositoryStorage>,
    options: Options,
    directory: PathBuf,
    profile: bool,
    persistent: bool,
    session: Option<session::Session>,
    retain_storage: bool,
    reuse_storage: bool,
}
impl Client {
    pub fn open(directory: &Path, options: Options) -> Result<Self, Error> {
        let directory = directory.canonicalize().map_err(error)?;
        let repository = Repository::discover(
            directory
                .to_str()
                .ok_or_else(|| message("non-UTF-8 path"))?,
        )
        .map_err(error)?;
        Ok(Self {
            storage: Arc::new(RepositoryStorage::new(repository)),
            options,
            directory,
            profile: false,
            persistent: false,
            session: None,
            retain_storage: false,
            reuse_storage: false,
        })
    }
    pub fn reuse_vm(&mut self) {
        self.persistent = true;
    }
    pub fn begin_request(&mut self) {
        self.retain_storage = true;
        self.reuse_storage = false;
    }

    pub fn end_request(&mut self) -> Result<(), Error> {
        self.retain_storage = false;
        self.reuse_storage = false;
        if let Some(session) = &mut self.session {
            let (status, output) = session.execute(&[], false).map_err(error)?;
            if status != 0 {
                return Err(Error::Backend(
                    String::from_utf8_lossy(&output).into_owned(),
                ));
            }
        }
        Ok(())
    }

    pub fn options(&self) -> &Options {
        &self.options
    }
    pub fn set_profile(&mut self, enabled: bool) {
        self.profile = enabled;
    }
    fn keep(&self, handle: Handle) -> Value {
        Value {
            handle,
            storage: self.storage.clone(),
        }
    }
    fn handle(&self, value: &Value) -> Result<Handle, Error> {
        if !Arc::ptr_eq(&self.storage, &value.storage) {
            return Err(message("value belongs to another storage"));
        }
        Ok(value.handle)
    }
    /// Reads the value of the label `name`.
    pub fn label_value(&mut self, name: &str) -> Result<Value, Error> {
        let handle = self.storage.repository().read_label(name).map_err(error)?;
        Ok(self.keep(handle))
    }
    pub fn lookup(&mut self, name: &str) -> Result<Value, Error> {
        Ok(self.keep(resolve(self.storage.repository(), name).map_err(error)?))
    }
    pub fn save(&mut self, value: &Value) -> Result<String, Error> {
        Ok(self
            .storage
            .save(self.handle(value)?)
            .map_err(error)?
            .to_string())
    }
    pub fn label(&mut self, name: &str, value: &Value) -> Result<(), Error> {
        let value = self.storage.save(self.handle(value)?).map_err(error)?;
        self.storage
            .repository()
            .write_label(name, value)
            .map_err(error)
    }
    pub fn call(&mut self, value: &Value) -> Result<Value, Error> {
        let start = std::time::Instant::now();
        let name = self.save(value)?;
        if self.profile {
            eprintln!("profile call input: {:?}", start.elapsed());
        }
        self.execute(&["call".into(), name])
    }
    pub fn schedule(&mut self, value: &Value) -> Result<Value, Error> {
        let start = std::time::Instant::now();
        let name = self.save(value)?;
        if self.profile {
            eprintln!("profile schedule input: {:?}", start.elapsed());
        }
        self.execute(&[
            "schedule".into(),
            "--memoize".into(),
            self.options.memoize.to_string(),
            "--trusted-tagger".into(),
            self.options.collector.clone(),
            name,
        ])
    }
    pub fn close(self) -> Result<(), Error> {
        Ok(())
    }

    /// Returns the absolute path of the active `.fix` directory.
    pub fn repository(&self) -> &str {
        self.storage.repository().path()
    }

    /// Imports a host file's bytes or a directory's child descriptors.
    pub fn file(&mut self, path: &Path) -> Result<Value, Error> {
        let failed = |e: &dyn fmt::Display| Error::Backend(format!("{}: {e}", path.display()));
        let path = std::fs::canonicalize(path).map_err(|e| failed(&e))?;
        let value = flatware_tree::import_contents(
            self.storage.repository(),
            &path,
            flatware_tree::Symlinks::Preserve,
        )
        .map_err(|e| failed(&e))?;
        Ok(self.keep(value))
    }

    /// Evaluates the `compile` Runnable applied to a Wasm module value.
    pub fn compilation(&mut self, module: &Value, units: Option<u64>) -> Result<Value, Error> {
        let request = self.compilation_request(module, units)?;
        self.eval(&request)
    }

    /// Evaluates compilation requests together, persisting their coupons.
    pub fn precompile(&mut self, requests: &[Value]) -> Result<(), Error> {
        let requests = self.tree(requests)?;
        self.eval(&requests)?;
        Ok(())
    }

    /// The strict application of `compile` to a Wasm module, with `compile`'s default
    /// unit count unless `units` is given.
    pub fn compilation_request(
        &mut self,
        module: &Value,
        units: Option<u64>,
    ) -> Result<Value, Error> {
        let limits = self.blob(b"")?;
        let compile = self.lookup("compile")?;
        let mut fields = vec![limits, compile, module.clone()];
        if let Some(units) = units {
            fields.push(self.blob(&units.to_le_bytes())?);
        }
        let combination = self.tree(&fields)?;
        let application = self.application(&combination)?;
        self.strict(&application)
    }

    fn scheduler_request(
        &mut self,
        mode: &str,
        collector: &Value,
        input: &Value,
    ) -> Result<Value, Error> {
        let mode = self.blob(mode.as_bytes())?;
        let scheduler = self.lookup(&self.options.scheduler.clone())?;
        self.tree(&[mode, scheduler, collector.clone(), input.clone()])
    }

    /// Performs one Think step on `value` and returns its result.
    pub fn think(&mut self, value: &Value) -> Result<Value, Error> {
        let collector = self.lookup(&self.options.collector.clone())?;
        let combination = self.scheduler_request("think", &collector, value)?;
        let coupon = self.schedule(&combination)?;
        if self.options.memoize == Memoization::Never {
            return Ok(coupon);
        }
        self.claimed(&coupon, &collector, 4, value)
    }

    /// Returns the result claimed by `coupon` after checking its author and request.
    fn claimed(
        &mut self,
        coupon: &Value,
        collector: &Value,
        statement: u64,
        input: &Value,
    ) -> Result<Value, Error> {
        let invalid = || message("scheduler returned an invalid coupon");
        let saved = |client: &mut Self, value: &Value| {
            let handle = client.handle(value)?;
            client.storage.save(handle).map_err(error)
        };
        let coupon = Tree::try_from(saved(self, coupon)?).map_err(|_| invalid())?;
        if !coupon.is_tag() || coupon.len() != 2 {
            return Err(invalid());
        }
        let fields = self.storage.repository().read_tree(coupon).map_err(error)?;
        let author = fields[0];
        let kernel = Tree::try_from(author).is_ok_and(|tree| tree.is_empty() && !tree.is_tag());
        if !kernel && !author.same_encoding(&saved(self, collector)?) {
            return Err(message(
                "scheduler returned a coupon with untrusted assumptions",
            ));
        }
        let claim = Ref::try_from(fields[1])
            .map_err(|_| invalid())?
            .object_descriptor();
        let claim = self
            .storage
            .repository()
            .read_tree(Tree::try_from(Handle::from(claim)).map_err(|_| invalid())?)
            .map_err(error)?;
        let input = saved(self, input)?;
        let expected = fixhandle::Blob::canonical(&statement.to_le_bytes()).map_err(error)?;
        if claim.len() != 3
            || !claim[0].same_encoding(expected.as_handle())
            || !claim[1].same_encoding(&input)
        {
            return Err(message(
                "scheduler returned a coupon for a different request",
            ));
        }
        self.lookup(&claim[2].to_string())
    }

    fn todo(&mut self, request: &Value) -> Result<Value, Error> {
        let empty = self.tree(&[])?;
        let combination = self.scheduler_request("todo", &empty, request)?;
        let name = self.save(&combination)?;
        self.execute(&["schedule".into(), name])
    }

    /// Fetches `uri`, persists its content, records the URI as its hint, and returns it.
    pub fn todo_import(&mut self, uri: &str) -> Result<Value, Error> {
        let action = self.blob(b"import")?;
        let uri_value = self.blob(uri.as_bytes())?;
        let request = self.tree(&[action, uri_value])?;
        let result = self.todo(&request)?;
        let status = self.get(&result, 0)?;
        let value = self.get(&result, 1)?;
        let mut code = [0; 8];
        self.read(&status, 0, &mut code)?;
        if u64::from_le_bytes(code) != 0 {
            let mut message = vec![0; self.len(&value)? as usize];
            self.read(&value, 0, &mut message)?;
            return Err(Error::Backend(format!(
                "{}: {uri}",
                String::from_utf8_lossy(&message)
            )));
        }
        Ok(value)
    }

    /// Records acquisition hints for an existing value.
    pub fn todo_add(&mut self, value: &Value, uris: &[String]) -> Result<(), Error> {
        let action = self.blob(b"add")?;
        let hints = uris
            .iter()
            .map(|uri| self.blob(uri.as_bytes()))
            .collect::<Result<Vec<_>, _>>()?;
        let hints = self.tree(&hints)?;
        let request = self.tree(&[action, value.clone(), hints])?;
        self.todo(&request)?;
        Ok(())
    }

    /// Lists the acquisition hints recorded for `value`.
    pub fn todo_list(&mut self, value: &Value) -> Result<Vec<Vec<u8>>, Error> {
        let action = self.blob(b"list")?;
        let hints = self.tree(&[])?;
        let request = self.tree(&[action, value.clone(), hints])?;
        let result = self.todo(&request)?;
        (0..self.len(&result)? as usize)
            .map(|index| {
                let uri = self.get(&result, index)?;
                let mut bytes = vec![0; self.len(&uri)? as usize];
                self.read(&uri, 0, &mut bytes)?;
                Ok(bytes)
            })
            .collect()
    }

    pub(crate) fn execute(&mut self, arguments: &[String]) -> Result<Value, Error> {
        let trust = if let Some(path) = &self.options.trust {
            std::fs::read_to_string(path).map_err(error)?
        } else {
            trust::load().map_err(error)?
        };
        let mut argv = vec!["fix".into(), "--trust".into(), trust];
        if self.profile {
            argv.push("--profile".into());
        }
        argv.extend_from_slice(arguments);
        static KERNEL: OnceLock<Arc<[u8]>> = OnceLock::new();
        let kernel = KERNEL.get_or_init(|| Arc::from(&include_bytes!(env!("FIX_KERNEL"))[..]));
        let memory = self
            .options
            .memory_mib
            .checked_mul(1 << 20)
            .filter(|size| *size >= 16 << 20 && size.is_power_of_two())
            .ok_or_else(|| message("memory must be a power of two, at least 16 MiB"))?;
        if self.options.jobs == 0 {
            return Err(message("jobs must be positive"));
        }
        let start = std::time::Instant::now();
        let (status, output) = if self.persistent {
            if self.session.is_none() {
                self.session = Some(
                    session::Session::new(
                        self.options.jobs,
                        memory,
                        kernel.clone(),
                        self.directory.clone(),
                    )
                    .map_err(error)?,
                );
            }
            self.session
                .as_mut()
                .unwrap()
                .execute(&argv[1..], self.reuse_storage)
                .map_err(error)?
        } else {
            vmm::runtime::Runtime::new(self.options.jobs, memory, kernel.clone())
                .run_captured(argv, &self.directory)
        };
        self.reuse_storage = self.retain_storage;
        if self.profile {
            eprintln!(
                "profile {} VM: {:?} (status {status})",
                arguments[0],
                start.elapsed()
            );
        }
        if status != 0 && status != fixapi::FIX_ERROR_STATUS {
            if self.persistent && status == 2 {
                return Err(Error::Backend(
                    String::from_utf8_lossy(&output).into_owned(),
                ));
            }
            return Err(Error::Execution(status));
        }
        let name = std::str::from_utf8(&output).map_err(error)?.trim();
        let value = self.lookup(name)?;
        if status == 0 {
            return Ok(value);
        }
        let message = self.get(&value, 1)?;
        let mut bytes = vec![0; self.len(&message)? as usize];
        self.read(&message, 0, &mut bytes)?;
        Err(Error::Fix(String::from_utf8_lossy(&bytes).into_owned()))
    }
}

impl Fix for Client {
    type Value = Value;
    type Error = Error;
    fn blob(&mut self, bytes: &[u8]) -> Result<Value, Error> {
        Ok(self.keep(self.storage.add_blob(bytes).into()))
    }
    fn tree(&mut self, children: &[Value]) -> Result<Value, Error> {
        let children = children
            .iter()
            .map(|v| self.handle(v))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.keep(self.storage.add_tree(&children).into()))
    }
    fn get(&mut self, tree: &Value, index: usize) -> Result<Value, Error> {
        use bytes::Buf;
        let tree = Tree::try_from(self.handle(tree)?).map_err(error)?;
        if index >= tree.len() {
            return Err(message("Tree index out of bounds"));
        }
        let mut backing = self
            .storage
            .get_tree_backing(tree)
            .map_err(error)?
            .into_buf();
        backing.advance(index * fixhandle::HANDLE_SIZE);
        let mut bytes = [0; fixhandle::HANDLE_SIZE];
        backing.copy_to_slice(&mut bytes);
        Ok(self.keep(unsafe { Handle::parse(bytes) }.map_err(error)?))
    }
    fn read(&mut self, blob: &Value, offset: usize, bytes: &mut [u8]) -> Result<(), Error> {
        use bytes::Buf;
        let blob = fixhandle::Blob::try_from(self.handle(blob)?).map_err(error)?;
        if offset
            .checked_add(bytes.len())
            .is_none_or(|end| end > blob.len())
        {
            return Err(message("Blob range out of bounds"));
        }
        if let Some(data) = blob.literal_bytes() {
            bytes.copy_from_slice(&data[offset..offset + bytes.len()]);
        } else {
            let mut data = self
                .storage
                .get_blob_backing(blob)
                .map_err(error)?
                .into_buf();
            data.advance(offset);
            data.copy_to_slice(bytes);
        }
        Ok(())
    }
    fn len(&mut self, value: &Value) -> Result<u64, Error> {
        let value = self.handle(value)?;
        if !matches!(value.view(), HandleView::Object(_) | HandleView::Ref(_)) {
            return Err(message("length requires an Object or Ref"));
        }
        Ok(value.len() as u64)
    }
    fn value_type(&mut self, value: &Value) -> Result<ValueType, Error> {
        Ok(match self.handle(value)?.view() {
            HandleView::Object(_) => ValueType::Object,
            HandleView::Ref(_) => ValueType::Ref,
            HandleView::Thunk(_) => ValueType::Thunk,
            HandleView::Encode(_) => ValueType::Encode,
        })
    }
    fn data_type(&mut self, value: &Value) -> Result<DataType, Error> {
        let value = self.handle(value)?;
        if !matches!(value.view(), HandleView::Object(_) | HandleView::Ref(_)) {
            return Err(message("expected Object or Ref"));
        }
        Ok(if value.is_tree_data() {
            DataType::Tree
        } else {
            DataType::Blob
        })
    }
    fn encode_type(&mut self, value: &Value) -> Result<EncodeType, Error> {
        Ok(
            match Encode::try_from(self.handle(value)?).map_err(error)?.view() {
                EncodeView::Strict(_) => EncodeType::Strict,
                EncodeView::Shallow(_) => EncodeType::Shallow,
            },
        )
    }
    fn is_tag(&mut self, value: &Value) -> Result<bool, Error> {
        Ok(Tree::try_from(self.handle(value)?).is_ok_and(|v| v.is_tag()))
    }
    fn is_comparable(&mut self, value: &Value) -> Result<bool, Error> {
        Ok(self.handle(value)?.is_comparable())
    }
    fn equals(&mut self, a: &Value, b: &Value) -> Result<bool, Error> {
        self.storage
            .equals(self.handle(a)?, self.handle(b)?)
            .map_err(error)
    }
    fn aliases(&self, left: &Value, right: &Value) -> bool {
        Arc::ptr_eq(&left.storage, &right.storage)
            && left.handle.as_bytes() == right.handle.as_bytes()
    }
    fn reference(&mut self, value: &Value) -> Result<Value, Error> {
        Ok(self.keep(
            Object::try_from(self.handle(value)?)
                .map_err(error)?
                .into_ref()
                .into(),
        ))
    }
    fn identification(&mut self, value: &Value) -> Result<Value, Error> {
        Ok(self.keep(
            reference(self.handle(value)?)
                .map_err(error)?
                .identification()
                .into(),
        ))
    }
    fn application(&mut self, value: &Value) -> Result<Value, Error> {
        Ok(self.keep(
            TreeRef::try_from(Handle::from(reference(self.handle(value)?).map_err(error)?))
                .map_err(error)?
                .application()
                .into(),
        ))
    }
    fn selection(&mut self, value: &Value) -> Result<Value, Error> {
        Ok(self.keep(
            TreeRef::try_from(Handle::from(reference(self.handle(value)?).map_err(error)?))
                .map_err(error)?
                .selection()
                .into(),
        ))
    }
    fn strict(&mut self, value: &Value) -> Result<Value, Error> {
        Ok(self.keep(
            Thunk::try_from(self.handle(value)?)
                .map_err(error)?
                .strict()
                .into(),
        ))
    }
    fn shallow(&mut self, value: &Value) -> Result<Value, Error> {
        Ok(self.keep(
            Thunk::try_from(self.handle(value)?)
                .map_err(error)?
                .shallow()
                .into(),
        ))
    }
    fn eval(&mut self, value: &Value) -> Result<Value, Error> {
        let start = std::time::Instant::now();
        let name = self.save(value)?;
        if self.profile {
            eprintln!("profile evaluate input: {:?}", start.elapsed());
        }
        self.execute(&[
            "evaluate".into(),
            "--memoize".into(),
            self.options.memoize.to_string(),
            self.options.scheduler.clone(),
            self.options.collector.clone(),
            name,
        ])
    }
}

pub(crate) fn resolve(
    repository: &Repository,
    name: &str,
) -> Result<Handle, fixrepository::RepositoryError> {
    if name.len() == 64 && name.bytes().all(|b| b.is_ascii_hexdigit()) {
        let mut bytes = [0; 32];
        hex::decode_to_slice(name, &mut bytes)
            .map_err(|_| fixrepository::RepositoryError::Handle(fixhandle::Error::InvalidName))?;
        repository.lookup(&bytes)
    } else {
        repository.read_label(name)
    }
}
pub(crate) fn reference(value: Handle) -> Result<Ref, fixhandle::Error> {
    match value.view() {
        HandleView::Object(object) => Ok(object.into_ref()),
        _ => Ref::try_from(value),
    }
}

pub use vmm::profiling::Capture as PerfCapture;
