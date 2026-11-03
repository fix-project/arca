#![no_std]

extern crate alloc;
#[cfg(target_os = "linux")]
extern crate std;

pub use fixhandle as handle;

#[cfg(target_os = "linux")]
mod fs;
#[cfg(not(target_os = "linux"))]
use kernel::host::fs;

use crate::handle::*;
use alloc::{boxed::Box, format, string::String, vec::Vec};
use common::protocol::control::{ErrorKind, FileMode};
use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    WrongStorage,
    UnsupportedName,
    NotFound,
    LengthMismatch,
    Cycle,
    NotComparable,
    InvalidComparisonBit,
    InvalidValueBit,
    InvalidSelection,
}

#[derive(Debug)]
pub enum RepositoryError {
    Io(ErrorKind),
    Handle(Error),
    Storage(StorageError),
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
const INDEX_FIELDS: usize = 8;

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

    pub fn object_paths(
        &self,
        object: Object,
        include_backing: bool,
    ) -> Result<Vec<String>, RepositoryError> {
        if object.is_empty() && object.as_handle().same_encoding(&Tree::empty().into()) {
            return Ok(Vec::new());
        }
        let record = if self.lookup(object.as_handle().as_bytes()).is_ok() {
            *object.as_handle()
        } else {
            let reference = object.into_ref();
            self.lookup(reference.as_handle().as_bytes())?
        };
        let mut paths = alloc::vec![format!(".fix/objects/{record}")];
        let todo = format!("todos/{}", object.as_handle());
        if fs::exists(&format!("{}/{todo}", self.path))? {
            paths.push(format!(".fix/{todo}"));
        }
        if !include_backing || !self.has_backing(object)? {
            return Ok(paths);
        }
        paths.push(format!(
            ".fix/data/{}",
            Self::data_key(object.as_handle().as_bytes())
        ));
        if let ObjectView::Tree(tree) = object.view()
            && tree.is_tag()
        {
            let fields = self.read_tree(*tree)?;
            let index = Self::tag_index(&fields);
            for index in [
                Some(index),
                self.claim_fields(&fields)?
                    .map(|claim| Self::index("claims", &claim)),
            ]
            .into_iter()
            .flatten()
            {
                let entry = format!("{index}/entries/{}", object.as_handle());
                if fs::exists(&format!("{}/{entry}", self.path))? {
                    paths.push(format!(".fix/{entry}"));
                }
            }
        }
        Ok(paths)
    }

    pub fn visit(
        &self,
        values: &[Handle],
        without_backing: &[Object],
        mut visitor: impl FnMut(Object, bool) -> Result<(), RepositoryError>,
    ) -> Result<(), RepositoryError> {
        let mut pending: Vec<_> = values.iter().rev().copied().collect();
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
            self.lookup(reference.as_handle().as_bytes())?;
            let include_backing = !without_backing
                .iter()
                .any(|excluded| excluded.as_handle().as_bytes() == object.as_handle().as_bytes());
            visitor(object, include_backing)?;
            if let Some(metadata) = self.get_todo(object.into_ref())? {
                pending.push(metadata);
            }
            if let ObjectView::Tree(tree) = object.view()
                && include_backing
                && self.has_backing(object)?
            {
                pending.extend(self.read_tree(*tree)?.iter().rev().copied());
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

    pub fn has_backing(&self, object: Object) -> Result<bool, RepositoryError> {
        if object.is_empty() && object.as_handle().same_encoding(&Tree::empty().into()) {
            return Ok(true);
        }
        match self.lookup(object.as_handle().as_bytes()) {
            Ok(_) => {}
            Err(RepositoryError::Io(ErrorKind::NotFound)) => return Ok(false),
            Err(error) => return Err(error),
        }
        if matches!(object.as_handle().name(), Name::Literal(_)) {
            return Ok(true);
        }
        Ok(fs::exists(&format!(
            "{}/objects/{}",
            self.path,
            object.as_handle()
        ))?)
    }

    pub fn get_todo(&self, reference: Ref) -> Result<Option<Handle>, RepositoryError> {
        let object = reference.object_descriptor();
        self.lookup(reference.as_handle().as_bytes())?;
        let path = format!("{}/todos/{}", self.path, object.as_handle());
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(ErrorKind::NotFound) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let text = core::str::from_utf8(&bytes).map_err(|_| RepositoryError::InvalidRepository)?;
        let mut key = [0; 32];
        hex::decode_to_slice(text.trim(), &mut key)
            .map_err(|_| RepositoryError::InvalidRepository)?;
        Ok(Some(self.lookup(&key)?))
    }

    pub fn set_todo(&self, reference: Ref, metadata: Handle) -> Result<(), RepositoryError> {
        let object = reference.object_descriptor();
        self.lookup(reference.as_handle().as_bytes())?;
        self.lookup(metadata.as_bytes())?;
        fs::mkdir(&format!("{}/todos", self.path))?;
        self.write_atomic(
            &format!("{}/todos/{}", self.path, object.as_handle()),
            format!("{metadata}\n").as_bytes(),
        )
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
        if !matches!(view.name(), Name::Literal(_)) {
            let mut object = *bytes;
            object[31] &= 0xe0;
            if view.byte_len() == 0 && &object == Tree::empty().as_handle().as_bytes() {
                return Ok(unsafe { view.to_owned() });
            }
            let expected = format!("../data/{}", Self::data_key(&object));
            let record = format!("{}/objects/{}", self.path, hex::encode(object));
            let admitted = match fs::read_link(&record) {
                Ok(target) => target,
                Err(ErrorKind::NotFound) if bytes[31] & 0x10 != 0 => {
                    object[31] |= 0x10;
                    fs::read_link(&format!("{}/objects/{}", self.path, hex::encode(object)))?
                }
                Err(error) => return Err(error.into()),
            };
            if admitted != expected {
                return Err(RepositoryError::InvalidRepository);
            }
        }
        // The exact Object or Ref record establishes the requested handle's provenance.
        Ok(unsafe { view.to_owned() })
    }

    pub fn read_blob(&self, blob: Blob) -> Result<Box<[u8]>, RepositoryError> {
        self.lookup(blob.as_handle().as_bytes())?;
        if let Some(bytes) = blob.literal_bytes() {
            return Ok(bytes.into());
        }
        self.read_data(blob.into())
    }

    pub fn read_tree(&self, tree: Tree) -> Result<Box<[Handle]>, RepositoryError> {
        if tree.is_empty() && tree.as_handle().same_encoding(&Tree::empty().into()) {
            return Ok(Box::default());
        }
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

    /// Finds stored Tags whose fields after the author start with `prefix`.
    /// Index entries are hints; the stored Tag establishes provenance and contents.
    pub fn tags(&self, prefix: &[Handle]) -> Result<Vec<Tree>, RepositoryError> {
        self.find_tags("tags", prefix)
    }

    /// Finds Ref-backed assertion Tags by the supplied assertion's field prefix.
    pub fn claims(&self, prefix: &[Handle]) -> Result<Vec<Tree>, RepositoryError> {
        self.find_tags("claims", prefix)
    }

    fn find_tags(&self, index: &str, prefix: &[Handle]) -> Result<Vec<Tree>, RepositoryError> {
        if prefix.iter().any(|field| !field.is_canonical()) {
            return Err(RepositoryError::NonCanonical);
        }
        let mut path = format!("{}/{index}", self.path);
        for field in prefix.iter().take(INDEX_FIELDS) {
            path.push('/');
            path.push_str(&format!("{field}"));
        }
        let mut pending = alloc::vec![(path, prefix.len().min(INDEX_FIELDS))];
        let mut tags = Vec::new();
        while let Some((path, depth)) = pending.pop() {
            let names = match fs::read_dir(&path) {
                Ok(names) => names,
                Err(ErrorKind::NotFound) => continue,
                Err(error) => return Err(error.into()),
            };
            for name in names {
                let child = format!("{path}/{name}");
                if name == "entries" {
                    for name in fs::read_dir(&child)? {
                        let mut bytes = [0; 32];
                        if hex::decode_to_slice(&name, &mut bytes).is_err() {
                            continue;
                        }
                        let Ok(handle) = self.lookup(&bytes) else {
                            continue;
                        };
                        let Ok(tag) = Tree::try_from(handle) else {
                            continue;
                        };
                        if tag.is_tag() {
                            let fields = self.read_tree(tag)?;
                            let fields = if index == "claims" {
                                self.claim_fields(&fields)?
                            } else {
                                Some(fields.get(1..).unwrap_or(&[]).into())
                            };
                            if fields.is_some_and(|fields| {
                                fields.len() >= prefix.len()
                                    && fields.iter().zip(prefix).all(|(a, b)| a.same_encoding(b))
                            }) {
                                tags.push(tag);
                            }
                        }
                    }
                } else if depth < INDEX_FIELDS
                    && name.len() == 64
                    && name.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    pending.push((child, depth + 1));
                }
            }
        }
        Ok(tags)
    }

    fn claim_fields(&self, fields: &[Handle]) -> Result<Option<Box<[Handle]>>, RepositoryError> {
        let [_, reference] = fields else {
            return Ok(None);
        };
        let Ok(reference) = TreeRef::try_from(*reference) else {
            return Ok(None);
        };
        match self.read_tree(reference.object_descriptor()) {
            Ok(fields) => Ok(Some(fields)),
            Err(RepositoryError::Io(ErrorKind::NotFound)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn index_tag(&self, tag: Tree) -> Result<(), RepositoryError> {
        let fields = self.read_tree(tag)?;
        self.index_entry(tag, "tags", fields.get(1..).unwrap_or(&[]))?;
        if let Some(claim) = self.claim_fields(&fields)? {
            self.index_entry(tag, "claims", &claim)?;
        }
        Ok(())
    }

    fn index_entry(
        &self,
        tag: Tree,
        index: &str,
        fields: &[Handle],
    ) -> Result<(), RepositoryError> {
        let count = fields.len().min(INDEX_FIELDS);
        let path = format!("{}/{}/entries", self.path, Self::index(index, fields));
        fs::mkdir(&path)?;
        let target = format!("{}objects/{}", "../".repeat(count + 2), tag.as_handle());
        let record = format!("{path}/{}", tag.as_handle());
        match fs::symlink(&target, &record) {
            Ok(()) => Ok(()),
            Err(ErrorKind::AlreadyExists) if fs::read_link(&record)? == target => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn tag_index(fields: &[Handle]) -> String {
        Self::index("tags", fields.get(1..).unwrap_or(&[]))
    }

    fn index(index: &str, fields: &[Handle]) -> String {
        let mut path = String::from(index);
        for field in fields.iter().take(INDEX_FIELDS) {
            path.push('/');
            path.push_str(&format!("{field}"));
        }
        path
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

    pub fn publish(&self, object: Handle, bytes: &[u8]) -> Result<(), RepositoryError> {
        if object.is_empty() && object.same_encoding(&Tree::empty().into()) {
            if !bytes.is_empty() {
                return Err(RepositoryError::InvalidRepository);
            }
            return Ok(());
        }
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
            Ok(()) => (),
            Err(ErrorKind::AlreadyExists) if fs::read_link(&record)? == target => (),
            Err(error) => return Err(error.into()),
        }
        if let Ok(tree) = Tree::try_from(object)
            && tree.is_tag()
        {
            self.index_tag(tree)?;
        }
        Ok(())
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

#[cfg(all(test, target_os = "linux"))]
mod tests;
