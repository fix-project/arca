extern crate alloc;

use crate::handle::*;
use alloc::{boxed::Box, format, string::String, vec::Vec};
use common::protocol::control::{ErrorKind, FileMode};
use core::sync::atomic::{AtomicU64, Ordering};
use kernel::host::fs;

#[derive(Debug)]
pub enum RepositoryError {
    Io(ErrorKind),
    Handle(Error),
    Storage(crate::storage::StorageError),
    NotInitialized,
    NonCanonical,
    InvalidRepository,
    InvalidLabel,
    MissingLabel { name: String, repository: String },
}

impl From<ErrorKind> for RepositoryError {
    fn from(error: ErrorKind) -> Self {
        Self::Io(error)
    }
}
impl From<Error> for RepositoryError {
    fn from(error: Error) -> Self {
        Self::Handle(error)
    }
}

#[derive(Debug)]
pub struct Repository {
    path: String,
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct Temporary(Option<String>);
impl Drop for Temporary {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

impl Repository {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn visit(
        &self,
        value: Handle,
        mut visitor: impl FnMut(Object) -> Result<(), RepositoryError>,
    ) -> Result<(), RepositoryError> {
        let mut pending = alloc::vec![value];
        let mut seen = hashbrown::HashSet::new();
        while let Some(value) = pending.pop() {
            let reference = match value.view() {
                HandleView::Object(object) => object.into_ref(),
                HandleView::Ref(reference) => *reference,
                HandleView::Thunk(thunk) => match thunk.view() {
                    ThunkView::Identification(reference) => reference,
                    ThunkView::Application(reference) | ThunkView::Selection(reference) => {
                        reference.into()
                    }
                },
                HandleView::Encode(encode) => {
                    let thunk = match encode.view() {
                        EncodeView::Strict(thunk) | EncodeView::Shallow(thunk) => thunk,
                    };
                    pending.push(thunk.into());
                    continue;
                }
            };
            let object = reference.object_descriptor();
            if matches!(object.as_handle().name(), Name::Literal(_))
                || !seen.insert(*object.as_handle().as_bytes())
            {
                continue;
            }
            self.lookup(object.as_handle().as_bytes())?;
            visitor(object)?;
            if let ObjectView::Tree(tree) = object.view() {
                pending.extend(self.read_tree(*tree)?.iter().rev());
            }
        }
        Ok(())
    }

    pub fn init(path: &str) -> Result<Self, RepositoryError> {
        for directory in ["data", "objects", "labels"] {
            fs::mkdir(&format!("{path}/{directory}"))?;
        }
        Self::open(path)
    }

    pub fn discover(start: &str) -> Result<Self, RepositoryError> {
        let mut directory = fs::canonicalize(start)?;
        loop {
            let path = format!("{}/.fix", directory.trim_end_matches('/'));
            match fs::is_directory(&path) {
                Ok(true) => return Self::open(&path),
                Ok(false) => return Err(RepositoryError::InvalidRepository),
                Err(ErrorKind::NotFound) => (),
                Err(error) => return Err(error.into()),
            }
            if directory == "/" {
                return Err(RepositoryError::NotInitialized);
            }
            directory.truncate(directory.rfind('/').unwrap().max(1));
        }
    }

    pub fn open(path: &str) -> Result<Self, RepositoryError> {
        for directory in ["data", "objects", "labels"] {
            match fs::is_directory(&format!("{path}/{directory}")) {
                Ok(true) => (),
                Ok(false) | Err(ErrorKind::NotFound) => {
                    return Err(RepositoryError::NotInitialized);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(Self { path: path.into() })
    }

    pub fn create_blob(&self, bytes: &[u8]) -> Result<Blob, RepositoryError> {
        let blob = Blob::canonical(bytes)?;
        self.publish(blob.into(), bytes)?;
        Ok(blob)
    }

    pub fn create_tree(&self, children: &[Handle]) -> Result<Tree, RepositoryError> {
        for child in children {
            self.lookup(child.as_bytes())?;
        }
        let tree = Tree::canonical(children)?;
        let bytes: Vec<u8> = children
            .iter()
            .flat_map(|child| child.as_bytes().iter().copied())
            .collect();
        self.publish(tree.into(), &bytes)?;
        Ok(tree)
    }

    pub fn lookup(&self, bytes: &[u8; 32]) -> Result<Handle, RepositoryError> {
        let view = Handle::parse_ref(bytes)?;
        if !view.is_canonical() {
            return Err(RepositoryError::NonCanonical);
        }
        let mut object_bytes = *bytes;
        object_bytes[31] &= 0xe0;
        let object_view = Handle::parse_ref(&object_bytes)?;
        let object = match object_view.name() {
            Name::Literal(contents) => Blob::literal(contents)?.object(),
            Name::Canonical(_) => {
                let key = hex::encode(object_bytes);
                let expected = format!("../data/{}", Self::data_key(&object_bytes));
                if fs::read_link(&format!("{}/objects/{key}", self.path))? != expected {
                    return Err(RepositoryError::InvalidRepository);
                }
                if !fs::exists(&format!("{}/objects/{key}", self.path))? {
                    return Err(ErrorKind::NotFound.into());
                }
                // The exact Object record establishes naming and type provenance.
                Object::try_from(unsafe { object_view.to_owned() })?
            }
            _ => return Err(RepositoryError::NonCanonical),
        };
        let value = if bytes[31] & 0x10 == 0 {
            object.into()
        } else {
            let reference = object.into_ref();
            let thunk = match bytes[31] & 12 {
                0 => return Ok(reference.into()),
                4 => TreeRef::try_from(Handle::from(reference))?.application(),
                8 => reference.identification(),
                12 => TreeRef::try_from(Handle::from(reference))?.selection(),
                _ => unreachable!(),
            };
            match bytes[31] & 3 {
                0 => thunk.into(),
                1 => thunk.strict().into(),
                2 => thunk.shallow().into(),
                _ => unreachable!(),
            }
        };
        Ok(value)
    }

    pub fn read_blob(&self, blob: Blob) -> Result<Box<[u8]>, RepositoryError> {
        self.lookup(blob.as_handle().as_bytes())?;
        if let Some(bytes) = blob.literal_bytes() {
            return Ok(bytes.into());
        }
        self.read_data(blob.into())
    }

    pub fn read_tree(&self, tree: Tree) -> Result<Box<[Handle]>, RepositoryError> {
        self.lookup(tree.as_handle().as_bytes())?;
        let bytes = self.read_data(tree.into())?;
        let mut children = Vec::with_capacity(tree.len());
        for child in bytes.as_chunks::<32>().0 {
            // Trusted Tree payloads contain authorized canonical child handles.
            let view = Handle::parse_ref(child)?;
            if !view.is_canonical() {
                return Err(RepositoryError::InvalidRepository);
            }
            children.push(unsafe { view.to_owned() });
        }
        Ok(children.into_boxed_slice())
    }

    pub fn read_label(&self, name: &str) -> Result<Handle, RepositoryError> {
        let path = self.label_path(name)?;
        let bytes = fs::read(&path).map_err(|error| match error {
            ErrorKind::NotFound => RepositoryError::MissingLabel {
                name: name.into(),
                repository: self.path.clone(),
            },
            error => error.into(),
        })?;
        let text = core::str::from_utf8(&bytes).map_err(|_| RepositoryError::InvalidRepository)?;
        let mut key = [0; 32];
        hex::decode_to_slice(text.trim(), &mut key)
            .map_err(|_| RepositoryError::InvalidRepository)?;
        self.lookup(&key)
    }

    pub fn labels(&self) -> Result<Vec<String>, RepositoryError> {
        let mut names = fs::read_dir(&format!("{}/labels", self.path))?;
        names.sort();
        Ok(names)
    }

    pub fn write_label(&self, name: &str, value: Handle) -> Result<(), RepositoryError> {
        let path = self.label_path(name)?;
        let value = self.lookup(value.as_bytes())?;
        self.write_atomic(&path, format!("{value}\n").as_bytes())
    }

    fn label_path(&self, name: &str) -> Result<String, RepositoryError> {
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.contains('/')
            || name.contains('\0')
        {
            return Err(RepositoryError::InvalidLabel);
        }
        Ok(format!("{}/labels/{name}", self.path))
    }

    fn data_key(bytes: &[u8; 32]) -> String {
        let mut bytes = *bytes;
        bytes[30..].fill(0xff);
        hex::encode(bytes)
    }

    fn read_data(&self, object: Handle) -> Result<Box<[u8]>, RepositoryError> {
        let path = format!("{}/objects/{object}", self.path);
        let bytes = fs::read(&path)?;
        if bytes.len() as u64 != object.byte_len() {
            return Err(RepositoryError::InvalidRepository);
        }
        Ok(bytes.into_boxed_slice())
    }

    pub(crate) fn publish(&self, object: Handle, bytes: &[u8]) -> Result<(), RepositoryError> {
        if matches!(object.name(), Name::Literal(_)) {
            return Ok(());
        }
        let key = Self::data_key(object.as_bytes());
        let path = format!("{}/data/{key}", self.path);
        if !fs::exists(&path)? {
            self.write_atomic(&path, bytes)?;
        }
        let target = format!("../data/{key}");
        let record = format!("{}/objects/{object}", self.path);
        match fs::symlink(&target, &record) {
            Ok(()) => Ok(()),
            Err(ErrorKind::AlreadyExists) if fs::read_link(&record)? == target => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn write_atomic(&self, path: &str, bytes: &[u8]) -> Result<(), RepositoryError> {
        let mut temporary = loop {
            let index = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let temporary = format!("{}/data/.tmp-{index}", self.path);
            match fs::write_with(
                &temporary,
                bytes,
                FileMode {
                    write: true,
                    create_new: true,
                    ..Default::default()
                },
            ) {
                Ok(()) => break Temporary(Some(temporary)),
                Err(ErrorKind::AlreadyExists) => continue,
                Err(error) => return Err(error.into()),
            }
        };
        fs::rename(temporary.0.as_ref().unwrap(), path)?;
        temporary.0.take();
        Ok(())
    }
}
