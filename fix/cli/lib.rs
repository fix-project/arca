pub use fix::Memoization;
use fix::{DataType, EncodeType, ValueType};
use fixhandle::{Encode, EncodeView, Handle, HandleView, Object, Ref, Thunk, Tree, TreeRef};
use fixrepository::{Repository, RepositoryError};
use fixstorage::{Storage, repository::RepositoryStorage};
use std::{
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

mod archive;
pub mod bootstrap_store;
pub mod commands;
pub mod configuration;
pub mod expressions;
pub mod remotes;
pub mod shell;
pub use shell::Shell;
mod serve;
mod session;
mod trust;

#[derive(Debug)]
pub enum Error {
    Backend(String),
    Execution {
        status: u8,
        operation: String,
        diagnostic: String,
    },
    /// The evaluation failed with the `Fix.error` request carrying this message.
    Fix(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(message) => f.write_str(message),
            Self::Execution {
                status,
                operation,
                diagnostic,
            } => {
                if diagnostic.is_empty() {
                    write!(
                        f,
                        "{operation} failed with exit status {status}; runtime returned no diagnostic"
                    )
                } else {
                    write!(f, "{operation} failed: {diagnostic} (exit status {status})")
                }
            }
            Self::Fix(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for Error {}
impl From<fix::TypeError> for Error {
    fn from(error: fix::TypeError) -> Self {
        match error {
            fix::TypeError::WrongType => message("wrong Fix type"),
        }
    }
}

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
    shell: core::marker::PhantomData<std::rc::Rc<()>>,
}

impl fmt::Debug for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.handle.fmt(formatter)
    }
}

#[derive(Clone)]
pub struct Options {
    pub scheduler: String,
    pub collector: String,
    pub parser: String,
    pub compiler: String,
    pub trust: Option<PathBuf>,
    pub remotes: Vec<fixremote::Remote>,
    pub remote_schedulers: Vec<(String, String)>,
    pub remote_collectors: Vec<(String, String)>,
    pub identities: Vec<String>,
    pub unsigned: bool,
    pub jobs: usize,
    pub memory_mib: usize,
    pub memoize: Memoization,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            scheduler: "scheduler".into(),
            collector: "collector".into(),
            parser: "fix-parser".into(),
            compiler: "compile".into(),
            trust: None,
            remotes: Vec::new(),
            remote_schedulers: Vec::new(),
            remote_collectors: Vec::new(),
            identities: Vec::new(),
            unsigned: false,
            jobs: 1,
            memory_mib: 16384,
            memoize: Memoization::Auto,
        }
    }
}

impl Options {
    pub(crate) fn configure(&self, repository: Repository) -> Repository {
        repository.with_overrides(self.identities.clone(), self.unsigned)
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
fn kernel() -> &'static Arc<[u8]> {
    static KERNEL: OnceLock<Arc<[u8]>> = OnceLock::new();
    KERNEL.get_or_init(|| Arc::from(&include_bytes!(env!("FIX_KERNEL"))[..]))
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
        let repository = options.configure(repository);
        let mut client = Self {
            storage: Arc::new(RepositoryStorage::new(repository)),
            options,
            directory,
            profile: false,
            persistent: false,
            session: None,
            retain_storage: false,
            reuse_storage: false,
        };
        let config = client.remote_config()?;
        let mut policy = config.policy;
        policy.identities.clear();
        let bootstrap = bootstrap_store::repository()?;
        let path = bootstrap
            .to_str()
            .ok_or_else(|| message("bootstrap repository path is not UTF-8"))?;
        match Repository::open(path) {
            Ok(source) if source.path() != client.storage.repository().path() => {
                Arc::get_mut(&mut client.storage)
                    .unwrap()
                    .add_source(source.with_policy(policy.clone()));
            }
            Ok(_) | Err(RepositoryError::NotInitialized) => (),
            Err(error) => return Err(self::error(error)),
        }
        for remote in config.remotes {
            if let Some(path) = remote.address.strip_prefix("file://") {
                match Repository::open(path) {
                    Ok(source) if source.path() != client.storage.repository().path() => {
                        Arc::get_mut(&mut client.storage)
                            .unwrap()
                            .add_source(source.with_policy(policy.clone()));
                    }
                    Ok(_) | Err(RepositoryError::NotInitialized) => (),
                    Err(error) => return Err(self::error(error)),
                }
            }
        }
        Ok(client)
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

    pub fn remote_config(&self) -> Result<fixremote::Config, Error> {
        let mut policy = self.storage.repository().policy().map_err(error)?;
        let compilers = if let Some(path) = &self.options.trust {
            Some(std::fs::read_to_string(path).map_err(error)?)
        } else {
            trust::load().map_err(error)?
        };
        if let Some(text) = compilers {
            policy.trusted_compilers = text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(String::from)
                .collect();
        }
        let mut remotes = self.storage.repository().remotes().map_err(error)?;
        for remote in &self.options.remotes {
            if let Some(existing) = remotes
                .iter_mut()
                .find(|existing| existing.name == remote.name)
            {
                existing.address = remote.address.clone();
            } else {
                remotes.push(remote.clone());
            }
        }
        for (name, scheduler) in &self.options.remote_schedulers {
            remotes
                .iter_mut()
                .find(|remote| &remote.name == name)
                .ok_or_else(|| message("unknown remote for scheduler override"))?
                .scheduler = Some(scheduler.clone());
        }
        for (name, collector) in &self.options.remote_collectors {
            remotes
                .iter_mut()
                .find(|remote| &remote.name == name)
                .ok_or_else(|| message("unknown remote for collector override"))?
                .collector = Some(collector.clone());
        }
        Ok(fixremote::Config { policy, remotes })
    }

    pub fn export(
        &mut self,
        values: &[Value],
        without_backing: &[Value],
    ) -> Result<Vec<u8>, Error> {
        let roots = values
            .iter()
            .map(|value| {
                self.handle(value)
                    .and_then(|handle| self.storage.save(handle).map_err(error))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let omitted = without_backing
            .iter()
            .map(|value| {
                let handle = self.storage.save(self.handle(value)?).map_err(error)?;
                reference(handle)
                    .map(|reference| reference.object_descriptor())
                    .map_err(error)
            })
            .collect::<Result<Vec<_>, _>>()?;
        fixremote::export(self.storage.repository(), &roots, &omitted).map_err(error)
    }

    pub fn certify(
        &self,
        name: [u8; 32],
        keys: &[String],
    ) -> Result<
        (
            fixremote::protocol::CertificateRef,
            fixremote::protocol::Record,
        ),
        Error,
    > {
        let config = self.remote_config()?;
        fixremote::certify(
            self.storage.repository(),
            name,
            keys,
            &config.policy.identities,
            |identity, namespace, message| {
                fixremote::linux::sign(identity, namespace, message)
                    .map_err(|error| fixcertificates::Error::Signing(error.to_string()))
            },
        )
        .map_err(error)
    }

    pub fn information(
        &self,
        collector: &Value,
    ) -> Result<fixremote::protocol::Information, Error> {
        let collector = self.storage.save(self.handle(collector)?).map_err(error)?;
        fixremote::information(
            self.storage.repository(),
            collector,
            self.options.jobs.try_into().map_err(error)?,
            Some((self.options.memory_mib as u64).saturating_mul(1 << 20)),
        )
        .map_err(error)
    }

    pub fn missing(&self, offers: &[fixremote::protocol::Offer]) -> Result<Vec<[u8; 32]>, Error> {
        fixremote::missing(self.storage.repository(), offers).map_err(error)
    }

    pub fn import(&mut self, bytes: &[u8]) -> Result<Vec<Value>, Error> {
        let values = fixremote::import(self.storage.repository(), bytes, &self.remote_config()?)
            .map_err(|error| Error::Backend(error.to_string()))?;
        Ok(values.into_iter().map(|handle| self.keep(handle)).collect())
    }
    pub fn set_profile(&mut self, enabled: bool) {
        self.profile = enabled;
    }
    fn keep(&self, handle: Handle) -> Value {
        Value {
            handle,
            storage: self.storage.clone(),
            shell: core::marker::PhantomData,
        }
    }
    fn handle(&self, value: &Value) -> Result<Handle, Error> {
        if !Arc::ptr_eq(&self.storage, &value.storage) {
            return Err(message("value belongs to another storage"));
        }
        Ok(value.handle)
    }
    /// Reads a label in the active repository.
    pub fn label_value(&mut self, name: &str) -> Result<Value, Error> {
        let handle = self.storage.repository().read_label(name).map_err(error)?;
        Ok(self.keep(handle))
    }
    pub fn lookup(&mut self, name: &str) -> Result<Value, Error> {
        if name.contains(':') {
            return crate::remotes::fetch(self, name, true)
                .map_err(|error| Error::Backend(error.to_string()));
        }
        if name.len() == 64 && name.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            let mut bytes = [0; 32];
            hex::decode_to_slice(name, &mut bytes).map_err(error)?;
            let handle = self.storage.identify(&bytes).map_err(|_| {
                self::error(RepositoryError::MissingObject {
                    name: name.into(),
                    repository: self.storage.repository().path().into(),
                })
            })?;
            Ok(self.keep(handle))
        } else {
            self.label_value(name)
        }
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
    pub(crate) fn label_imported(&mut self, name: &str, value: &Value) -> Result<(), Error> {
        let value = self.handle(value)?;
        self.storage
            .repository()
            .write_label(name, value)
            .map_err(error)
    }
    pub fn call(&mut self, value: &Value) -> Result<Value, Error> {
        let start = std::time::Instant::now();
        let name = self.save(value)?;
        if self.profile {
            common::progress::println(format!("profile call input: {:?}", start.elapsed()));
        }
        self.execute(&["call".into(), name])
    }
    pub fn schedule_stream(&mut self, value: &Value) -> Result<Value, Error> {
        use std::io::{Read, Write};
        use std::net::Shutdown;
        use std::os::unix::net::UnixStream;
        let name = self.save(value)?;
        let remotes = self.remote_config()?;
        let mut argv = vec![
            "fix".into(),
            "--trust".into(),
            remotes.policy.trusted_compilers.join("\n"),
            "--remotes".into(),
            serde_json::to_string(&remotes).map_err(error)?,
        ];
        if self.profile {
            argv.push("--profile".into());
        }
        argv.extend([
            "schedule".into(),
            "--memoize".into(),
            self.options.memoize.to_string(),
            name.clone(),
        ]);
        let memory = self
            .options
            .memory_mib
            .checked_mul(1 << 20)
            .filter(|size| *size >= 16 << 20 && size.is_power_of_two())
            .ok_or_else(|| message("memory must be a power of two, at least 16 MiB"))?;
        if self.options.jobs == 0 {
            return Err(message("jobs must be positive"));
        }
        let (vm, peer) = UnixStream::pair().map_err(error)?;
        let mut incoming = peer.try_clone().map_err(error)?;
        let mut outgoing = peer;
        std::thread::spawn(move || {
            let _ = std::io::copy(&mut std::io::stdin(), &mut incoming);
            let _ = incoming.shutdown(Shutdown::Write);
        });
        let output = std::thread::spawn(move || {
            let mut stdout = std::io::stdout().lock();
            let mut bytes = [0; 65536];
            loop {
                let length = outgoing.read(&mut bytes)?;
                if length == 0 {
                    return Ok::<(), std::io::Error>(());
                }
                stdout.write_all(&bytes[..length])?;
                stdout.flush()?;
            }
        });
        let (status, result) =
            vmm::runtime::Runtime::new(self.options.jobs, memory, kernel().clone())
                .run_stream_captured(argv, &self.directory, vm);
        output.join().map_err(error)?.map_err(error)?;
        if status == 0 {
            self.lookup(std::str::from_utf8(&result).map_err(error)?.trim())
        } else {
            Err(Error::Execution {
                status,
                operation: format!("schedule {name}"),
                diagnostic: String::from_utf8_lossy(&result).trim().into(),
            })
        }
    }

    pub fn schedule(&mut self, value: &Value) -> Result<Value, Error> {
        let start = std::time::Instant::now();
        let name = self.save(value)?;
        if self.profile {
            common::progress::println(format!("profile schedule input: {:?}", start.elapsed()));
        }
        self.execute(&[
            "schedule".into(),
            "--memoize".into(),
            self.options.memoize.to_string(),
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
    pub fn precompile(&mut self, requests: &[Value]) -> Result<Value, Error> {
        let requests = self.tree(requests)?;
        self.eval(&requests)
    }

    /// The strict application of `compile` to a Wasm module, with `compile`'s default
    /// unit count unless `units` is given.
    pub fn compilation_request(
        &mut self,
        module: &Value,
        units: Option<u64>,
    ) -> Result<Value, Error> {
        let limits = self.blob(b"")?;
        let compile = self.lookup(&self.options.compiler.clone())?;
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
        let limits = self.blob(b"")?;
        self.tree(&[limits, scheduler, mode, collector.clone(), input.clone()])
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
    pub fn claimed(
        &mut self,
        coupon: &Value,
        collector: &Value,
        operation: u64,
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
        let expected = fixhandle::Blob::canonical(&operation.to_le_bytes()).map_err(error)?;
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
        let operation = arguments.first().map(String::as_str).unwrap_or("execute");
        let input = arguments.last().map(String::as_str).unwrap_or("");
        let label = self.storage.repository().labels().ok().and_then(|labels| {
            labels.into_iter().find(|label| {
                std::fs::read_to_string(Path::new(self.repository()).join("labels").join(label))
                    .is_ok_and(|value| value.trim() == input)
            })
        });
        let progress = common::progress::Progress::advised(format!(
            "{operation} {}",
            label.as_deref().unwrap_or(input)
        ));
        let remotes = self.remote_config()?;
        let trust = remotes.policy.trusted_compilers.join("\n");
        let mut argv = vec!["fix".into(), "--trust".into(), trust];
        argv.extend([
            "--remotes".into(),
            serde_json::to_string(&remotes).map_err(error)?,
        ]);
        if self.profile {
            argv.push("--profile".into());
        }
        argv.extend_from_slice(arguments);
        let kernel = kernel();
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
            common::progress::println(format!(
                "profile {} VM: {:?} (status {status})",
                arguments[0],
                start.elapsed()
            ));
        }
        if status != 0 && status != fix::FIX_ERROR_STATUS {
            return Err(Error::Execution {
                status,
                operation: format!("{operation} {}", label.as_deref().unwrap_or(input)),
                diagnostic: String::from_utf8_lossy(&output).trim().into(),
            });
        }
        let name = std::str::from_utf8(&output).map_err(error)?.trim();
        let value = self.lookup(name)?;
        if status == 0 {
            progress.finish();
            return Ok(value);
        }
        let message = self.get(&value, 1)?;
        let mut bytes = vec![0; self.len(&message)? as usize];
        self.read(&message, 0, &mut bytes)?;
        Err(Error::Fix(String::from_utf8_lossy(&bytes).into_owned()))
    }
}

impl Client {
    pub fn blob(&mut self, bytes: &[u8]) -> Result<Value, Error> {
        Ok(self.keep(self.storage.add_blob(bytes).into()))
    }
    pub fn tree(&mut self, children: &[Value]) -> Result<Value, Error> {
        let children = children
            .iter()
            .map(|v| self.handle(v))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.keep(self.storage.add_tree(&children).into()))
    }
    pub fn get(&mut self, tree: &Value, index: usize) -> Result<Value, Error> {
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
    pub fn read(&mut self, blob: &Value, offset: usize, bytes: &mut [u8]) -> Result<(), Error> {
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
    pub fn len(&mut self, value: &Value) -> Result<u64, Error> {
        let value = self.handle(value)?;
        if !matches!(value.view(), HandleView::Object(_) | HandleView::Ref(_)) {
            return Err(message("length requires an Object or Ref"));
        }
        Ok(value.len() as u64)
    }
    pub fn value_type(&mut self, value: &Value) -> Result<ValueType, Error> {
        Ok(match self.handle(value)?.view() {
            HandleView::Object(_) => ValueType::Object,
            HandleView::Ref(_) => ValueType::Ref,
            HandleView::Thunk(_) => ValueType::Thunk,
            HandleView::Encode(_) => ValueType::Encode,
        })
    }
    pub fn data_type(&mut self, value: &Value) -> Result<DataType, Error> {
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
    pub fn encode_type(&mut self, value: &Value) -> Result<EncodeType, Error> {
        Ok(
            match Encode::try_from(self.handle(value)?).map_err(error)?.view() {
                EncodeView::Strict(_) => EncodeType::Strict,
                EncodeView::Shallow(_) => EncodeType::Shallow,
            },
        )
    }
    pub fn is_tag(&mut self, value: &Value) -> Result<bool, Error> {
        Ok(Tree::try_from(self.handle(value)?).is_ok_and(|v| v.is_tag()))
    }
    pub fn is_comparable(&mut self, value: &Value) -> Result<bool, Error> {
        Ok(self.handle(value)?.is_comparable())
    }
    pub fn equals(&mut self, a: &Value, b: &Value) -> Result<bool, Error> {
        self.storage
            .equals(self.handle(a)?, self.handle(b)?)
            .map_err(error)
    }
    pub fn aliases(&self, left: &Value, right: &Value) -> bool {
        Arc::ptr_eq(&left.storage, &right.storage)
            && left.handle.as_bytes() == right.handle.as_bytes()
    }
    pub fn reference(&mut self, value: &Value) -> Result<Value, Error> {
        Ok(self.keep(
            Object::try_from(self.handle(value)?)
                .map_err(error)?
                .into_ref()
                .into(),
        ))
    }
    pub fn identification(&mut self, value: &Value) -> Result<Value, Error> {
        Ok(self.keep(
            reference(self.handle(value)?)
                .map_err(error)?
                .identification()
                .into(),
        ))
    }
    pub fn application(&mut self, value: &Value) -> Result<Value, Error> {
        Ok(self.keep(
            TreeRef::try_from(Handle::from(reference(self.handle(value)?).map_err(error)?))
                .map_err(error)?
                .application()
                .into(),
        ))
    }
    pub fn selection(&mut self, value: &Value) -> Result<Value, Error> {
        Ok(self.keep(
            TreeRef::try_from(Handle::from(reference(self.handle(value)?).map_err(error)?))
                .map_err(error)?
                .selection()
                .into(),
        ))
    }
    pub fn strict(&mut self, value: &Value) -> Result<Value, Error> {
        Ok(self.keep(
            Thunk::try_from(self.handle(value)?)
                .map_err(error)?
                .strict()
                .into(),
        ))
    }
    pub fn shallow(&mut self, value: &Value) -> Result<Value, Error> {
        Ok(self.keep(
            Thunk::try_from(self.handle(value)?)
                .map_err(error)?
                .shallow()
                .into(),
        ))
    }
    pub fn eval(&mut self, value: &Value) -> Result<Value, Error> {
        let start = std::time::Instant::now();
        let name = self.save(value)?;
        let scheduler = self.lookup(&self.options.scheduler.clone())?;
        let scheduler = self.save(&scheduler)?;
        let collector = self.lookup(&self.options.collector.clone())?;
        let collector = self.save(&collector)?;
        if self.profile {
            common::progress::println(format!("profile evaluate input: {:?}", start.elapsed()));
        }
        self.execute(&[
            "evaluate".into(),
            "--memoize".into(),
            self.options.memoize.to_string(),
            scheduler,
            collector,
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
