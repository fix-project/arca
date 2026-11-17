#![no_std]

extern crate alloc;
#[cfg(target_os = "linux")]
extern crate std;

pub use fixcertificates::{Certificate, Policy};
pub use fixhandle as handle;

mod config;
mod signing;
pub use config::Remote;

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
    LabelConflict,
    Certificate(fixcertificates::Error),
    MissingLabel { name: String, repository: String },
    MissingObject { name: String, repository: String },
    MissingBacking { name: String, repository: String },
}

impl core::fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl core::error::Error for RepositoryError {}

impl From<fixcertificates::Error> for RepositoryError {
    fn from(error: fixcertificates::Error) -> Self {
        Self::Certificate(error)
    }
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
    policy: Option<Policy>,
    identities: Vec<String>,
    unsigned_override: bool,
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
    pub fn with_overrides(mut self, identities: Vec<String>, unsigned: bool) -> Self {
        self.identities = identities;
        self.unsigned_override = unsigned;
        self
    }
    pub fn with_policy(mut self, policy: Policy) -> Self {
        self.policy = Some(policy);
        self
    }

    pub fn unsigned(self) -> Self {
        self.with_policy(Policy {
            unsigned: true,
            ..Policy::default()
        })
    }

    pub fn policy(&self) -> Result<Policy, RepositoryError> {
        let mut policy = match &self.policy {
            Some(policy) => policy.clone(),
            None => {
                #[cfg(target_os = "linux")]
                {
                    fixcertificates::linux::load()?
                }
                #[cfg(not(target_os = "linux"))]
                {
                    Policy::default()
                }
            }
        };
        if !self.identities.is_empty() {
            policy.identities = self.identities.clone();
        }
        policy.unsigned |= self.unsigned_override;
        Ok(policy)
    }
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
        if object.as_handle().as_bytes()[31] & 0x40 != 0 {
            let directory = self.certificate_directory(object.as_handle().as_bytes());
            match fs::read_dir(&directory) {
                Ok(entries) => {
                    for entry in entries {
                        paths.push(format!(
                            ".fix/tags/signatures/{}/{entry}",
                            object.as_handle()
                        ));
                    }
                }
                Err(ErrorKind::NotFound) => (),
                Err(error) => return Err(error.into()),
            }
            for certificate in self.certificate_blobs(record.as_bytes())? {
                paths.extend(self.object_paths(certificate.object(), true)?);
            }
        }
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
        Ok(paths)
    }

    pub fn visit(
        &self,
        values: &[Handle],
        without_backing: &[Object],
        mut visitor: impl FnMut(Object, bool) -> Result<(), RepositoryError>,
    ) -> Result<(), RepositoryError> {
        let mut pending: Vec<_> = values
            .iter()
            .rev()
            .copied()
            .map(|value| (value, false))
            .collect();
        let mut seen = hashbrown::HashMap::new();
        while let Some((value, thin)) = pending.pop() {
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
                    pending.push((thunk.into(), thin));
                    continue;
                }
            };
            let object = reference.object_descriptor();
            if matches!(object.as_handle().name(), Name::Literal(_)) {
                continue;
            }
            let include_backing = !thin
                && !without_backing.iter().any(|excluded| {
                    excluded.as_handle().as_bytes() == object.as_handle().as_bytes()
                });
            let key = *object.as_handle().as_bytes();
            if seen
                .get(&key)
                .is_some_and(|previous| *previous || !include_backing)
            {
                continue;
            }
            seen.insert(key, include_backing);
            self.lookup(reference.as_handle().as_bytes())?;
            visitor(object, include_backing)?;
            if let Some(metadata) = self.get_todo(object.into_ref())? {
                pending.push((metadata, false));
            }
            if let ObjectView::Tree(tree) = object.view()
                && include_backing
                && self.has_backing(object)?
            {
                let children = self.read_tree(*tree)?;
                pending.extend(
                    children
                        .iter()
                        .enumerate()
                        .rev()
                        .map(|(index, child)| (*child, tree.is_tag() && index == 0)),
                );
            }
        }
        Ok(())
    }

    pub fn init(path: &str) -> Result<Self, RepositoryError> {
        for directory in [
            "data",
            "objects",
            "labels",
            "tags/signatures",
            "tags/by-subject",
            "tags/by-author",
        ] {
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
        for directory in [
            "data",
            "objects",
            "labels",
            "tags/signatures",
            "tags/by-subject",
            "tags/by-author",
        ] {
            match fs::is_directory(&format!("{path}/{directory}")) {
                Ok(true) => (),
                Ok(false) | Err(ErrorKind::NotFound) => {
                    return Err(RepositoryError::NotInitialized);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(Self {
            path: path.into(),
            policy: None,
            identities: Vec::new(),
            unsigned_override: false,
        })
    }

    pub fn inventory(&self) -> Result<Vec<Handle>, RepositoryError> {
        let backing: hashbrown::HashSet<_> = fs::read_dir(&format!("{}/data", self.path))?
            .into_iter()
            .collect();
        let mut objects = Vec::new();
        for entry in fs::read_dir(&format!("{}/objects", self.path))? {
            let mut name = [0; 32];
            if hex::decode_to_slice(&entry, &mut name).is_err() || name[31] & 0x1f != 0 {
                continue;
            }
            if let Ok(handle) = unsafe { Handle::parse(name) }
                && handle.is_canonical()
                && Object::try_from(handle).is_ok()
                && backing.contains(&Self::data_key(&name))
            {
                objects.push(handle);
            }
        }
        objects.sort_by_key(|handle| *handle.as_bytes());
        Ok(objects)
    }

    pub fn has_backing(&self, object: Object) -> Result<bool, RepositoryError> {
        if object.is_empty() && object.as_handle().same_encoding(&Tree::empty().into()) {
            return Ok(true);
        }
        match self.lookup(object.as_handle().as_bytes()) {
            Ok(_) => {}
            Err(RepositoryError::MissingObject { .. }) => return Ok(false),
            Err(error) => return Err(error),
        }
        if matches!(object.as_handle().name(), Name::Literal(_)) {
            return Ok(true);
        }
        Ok(fs::exists(&format!(
            "{}/data/{}",
            self.path,
            Self::data_key(object.as_handle().as_bytes())
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
            if object[31] & 0x40 != 0 {
                let policy = self.policy()?;
                let verify = |name: [u8; 32]| -> Result<(), RepositoryError> {
                    let certificate = self.stored_certificates(name)?.ok_or_else(|| {
                        RepositoryError::MissingObject {
                            name: hex::encode(name),
                            repository: self.path.clone(),
                        }
                    })?;
                    if policy.unsigned
                        || certificate
                            .iter()
                            .any(|certificate| certificate.verify(&object, &policy).is_ok())
                    {
                        Ok(())
                    } else {
                        Err(fixcertificates::Error::Untrusted.into())
                    }
                };
                let primary = verify(object);
                if primary.is_err() && bytes[31] & 0x10 != 0 {
                    let mut reference = object;
                    reference[31] |= 0x10;
                    verify(reference).or(primary)?;
                } else {
                    primary?;
                }
                return Ok(unsafe { view.to_owned() });
            }
            let expected = format!("../data/{}", Self::data_key(&object));
            let record = format!("{}/objects/{}", self.path, hex::encode(object));
            let admitted = match fs::read_link(&record) {
                Ok(target) => target,
                Err(ErrorKind::NotFound) if bytes[31] & 0x10 != 0 => {
                    object[31] |= 0x10;
                    fs::read_link(&format!("{}/objects/{}", self.path, hex::encode(object)))
                        .map_err(|error| match error {
                            ErrorKind::NotFound => RepositoryError::MissingObject {
                                name: hex::encode(object),
                                repository: self.path.clone(),
                            },
                            error => error.into(),
                        })?
                }
                Err(ErrorKind::NotFound) => {
                    return Err(RepositoryError::MissingObject {
                        name: hex::encode(object),
                        repository: self.path.clone(),
                    });
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

    /// Reads unverified backing bytes. The receiver must verify the complete object.
    pub fn read_backing_range(
        &self,
        object: Object,
        offset: u64,
        length: usize,
    ) -> Result<Vec<u8>, RepositoryError> {
        self.lookup(object.as_handle().as_bytes())?;
        if offset
            .checked_add(length as u64)
            .is_none_or(|end| end > object.byte_len())
        {
            return Err(RepositoryError::InvalidRepository);
        }
        let path = format!(
            "{}/data/{}",
            self.path,
            Self::data_key(object.as_handle().as_bytes())
        );
        fs::read_range(&path, offset, length).map_err(|error| match error {
            ErrorKind::NotFound => RepositoryError::MissingBacking {
                name: hex::encode(object.as_handle().as_bytes()),
                repository: self.path.clone(),
            },
            error => error.into(),
        })
    }

    pub fn read_blob(&self, blob: Blob) -> Result<Box<[u8]>, RepositoryError> {
        self.lookup(blob.as_handle().as_bytes())?;
        if let Some(bytes) = blob.literal_bytes() {
            return Ok(bytes.into());
        }
        let bytes = self.read_data(blob.into())?;
        #[cfg(target_os = "linux")]
        let progress = (bytes.len() >= 1024 * 1024)
            .then(|| common::progress::Progress::activity("Verifying repository Blob data"));
        blob.verify(&bytes)?;
        #[cfg(target_os = "linux")]
        if let Some(progress) = progress {
            progress.finish();
        }
        Ok(bytes)
    }

    pub fn read_tree(&self, tree: Tree) -> Result<Box<[Handle]>, RepositoryError> {
        if tree.is_empty() && tree.as_handle().same_encoding(&Tree::empty().into()) {
            return Ok(Box::default());
        }
        self.lookup(tree.as_handle().as_bytes())?;
        let bytes = self.read_data(tree.into())?;
        #[cfg(target_os = "linux")]
        let progress = (bytes.len() >= 1024 * 1024).then(|| {
            common::progress::Progress::activity("Verifying and decoding repository Tree data")
        });
        tree.verify_bytes(&bytes)?;
        let mut children = Vec::with_capacity(tree.len());
        for (index, child) in bytes.as_chunks::<32>().0.iter().enumerate() {
            if tree.is_tag() && index == 0 {
                let author = Handle::parse_ref(child)?;
                if let Ok(object) = Object::try_from(unsafe { author.to_owned() }) {
                    self.lookup(object.into_ref().as_handle().as_bytes())?;
                    children.push(object.into());
                    continue;
                }
            }
            children.push(self.lookup(child)?);
        }
        #[cfg(target_os = "linux")]
        if let Some(progress) = progress {
            progress.finish();
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

    /// Finds Ref-backed assertion Tags by the supplied assertion's field prefix.
    pub fn claims(&self, prefix: &[Handle]) -> Result<Vec<Tree>, RepositoryError> {
        if prefix.iter().any(|field| !field.is_canonical()) {
            return Err(RepositoryError::NonCanonical);
        }
        let mut path = format!("{}/tags/by-subject", self.path);
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
                            let fields = self.claim_fields(&fields)?;
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

    pub fn claim_results(&self, author: Handle) -> Result<Vec<(u64, Handle)>, RepositoryError> {
        let root = format!("{}/tags/by-author/{author}", self.path);
        let operations = match fs::read_dir(&root) {
            Ok(operations) => operations,
            Err(ErrorKind::NotFound) => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut results = Vec::new();
        for operation in operations {
            let mut name = [0; 32];
            if hex::decode_to_slice(&operation, &mut name).is_err() {
                continue;
            }
            let Ok(handle) = (unsafe { Handle::parse(name) }) else {
                continue;
            };
            let Ok(kind) = Blob::try_from(handle) else {
                continue;
            };
            let Some(bytes) = kind.literal_bytes() else {
                continue;
            };
            let Ok(bytes) = <[u8; 8]>::try_from(bytes) else {
                continue;
            };
            let code = u64::from_le_bytes(bytes);
            if !matches!(code, 1 | 2 | 3 | 4 | 6) {
                continue;
            }
            for input in fs::read_dir(&format!("{root}/{operation}"))? {
                let mut name = [0; 32];
                if hex::decode_to_slice(&input, &mut name).is_err() {
                    continue;
                }
                if let Ok(handle) = unsafe { Handle::parse(name) }
                    && handle.is_canonical()
                {
                    results.push((code, handle));
                }
            }
        }
        Ok(results)
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
        if let Some(claim) = self.claim_fields(&fields)? {
            self.index_entry(tag, "tags/by-subject", &claim, 2)?;
            self.index_entry(tag, &format!("tags/by-author/{}", fields[0]), &claim, 3)?;
        }
        Ok(())
    }

    fn index_entry(
        &self,
        tag: Tree,
        base: &str,
        fields: &[Handle],
        base_depth: usize,
    ) -> Result<(), RepositoryError> {
        let count = fields.len().min(INDEX_FIELDS);
        let path = format!("{}/{}/entries", self.path, Self::index(base, fields));
        fs::mkdir(&path)?;
        let target = format!(
            "{}objects/{}",
            "../".repeat(count + base_depth + 1),
            tag.as_handle()
        );
        let record = format!("{path}/{}", tag.as_handle());
        match fs::symlink(&target, &record) {
            Ok(()) => Ok(()),
            Err(ErrorKind::AlreadyExists) if fs::read_link(&record)? == target => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn index(base: &str, fields: &[Handle]) -> String {
        let mut path = String::from(base);
        for field in fields.iter().take(INDEX_FIELDS) {
            path.push('/');
            path.push_str(&format!("{field}"));
        }
        path
    }

    pub fn write_label(&self, name: &str, value: Handle) -> Result<(), RepositoryError> {
        let path = self.label_path(name)?;
        let value = self.lookup(value.as_bytes())?;
        self.check_label_backing(value)?;
        self.write_atomic(&path, format!("{value}\n").as_bytes())
    }

    pub fn remove_label(&self, name: &str) -> Result<(), RepositoryError> {
        fs::remove_file(&self.label_path(name)?).map_err(|error| match error {
            ErrorKind::NotFound => RepositoryError::MissingLabel {
                name: name.into(),
                repository: self.path.clone(),
            },
            error => error.into(),
        })
    }

    fn check_label_backing(&self, root: Handle) -> Result<(), RepositoryError> {
        let mut pending = alloc::vec![(root, false)];
        let mut seen = hashbrown::HashSet::new();
        while let Some((value, author)) = pending.pop() {
            if !seen.insert((*value.as_bytes(), author)) {
                continue;
            }
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
                    pending.push((thunk.into(), author));
                    continue;
                }
            };
            let object = reference.object_descriptor();
            self.lookup(reference.as_handle().as_bytes())?;
            if !author && matches!(value.view(), HandleView::Object(_)) {
                if !self.has_backing(object)? {
                    return Err(RepositoryError::Io(ErrorKind::NotFound));
                }
                if let ObjectView::Tree(tree) = object.view() {
                    pending.extend(
                        self.read_tree(*tree)?
                            .iter()
                            .enumerate()
                            .map(|(index, child)| (*child, tree.is_tag() && index == 0)),
                    );
                }
            }
            if let Some(metadata) = self.get_todo(object.into_ref())? {
                pending.push((metadata, false));
            }
        }
        Ok(())
    }

    pub fn compare_and_write_label(
        &self,
        name: &str,
        expected: Option<[u8; 32]>,
        value: Handle,
    ) -> Result<(), RepositoryError> {
        self.label_path(name)?;
        let lock = format!("{}/data/.label-lock-{}", self.path, hex::encode(name));
        fs::write_with(
            &lock,
            b"",
            FileMode {
                write: true,
                create_new: true,
                ..Default::default()
            },
        )?;
        let _lock = Temporary(Some(lock));
        let current = match self.read_label(name) {
            Ok(handle) => Some(*handle.as_bytes()),
            Err(RepositoryError::MissingLabel { .. }) => None,
            Err(error) => return Err(error),
        };
        if current != expected {
            return Err(RepositoryError::LabelConflict);
        }
        self.write_label(name, value)
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
        let path = format!("{}/data/{}", self.path, Self::data_key(object.as_bytes()));
        let bytes = fs::read(&path).map_err(|error| match error {
            ErrorKind::NotFound => RepositoryError::MissingBacking {
                name: hex::encode(object.as_bytes()),
                repository: self.path.clone(),
            },
            error => error.into(),
        })?;
        if bytes.len() as u64 != object.byte_len() {
            return Err(RepositoryError::InvalidRepository);
        }
        Ok(bytes.into_boxed_slice())
    }

    pub fn publish(&self, object: Handle, bytes: &[u8]) -> Result<(), RepositoryError> {
        self.publish_certified(object, bytes, None)?;
        if let Ok(tree) = Tree::try_from(object)
            && tree.is_tag()
        {
            self.index_tag(tree)?;
        }
        Ok(())
    }

    pub fn publish_certified(
        &self,
        object: Handle,
        bytes: &[u8],
        certificate: Option<&[u8]>,
    ) -> Result<(), RepositoryError> {
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
        self.publish_record(object, certificate)?;
        Ok(())
    }

    pub fn publish_reference(&self, reference: Ref) -> Result<(), RepositoryError> {
        let handle = reference.as_handle();
        if !handle.is_canonical() {
            return Err(RepositoryError::NonCanonical);
        }
        if matches!(handle.name(), Name::Literal(_)) {
            return Ok(());
        }
        self.publish_record(*handle, None)
    }

    pub fn publish_certified_reference(
        &self,
        reference: Ref,
        certificate: &[u8],
    ) -> Result<(), RepositoryError> {
        self.publish_record(reference.into(), Some(certificate))
    }

    fn publish_record(
        &self,
        object: Handle,
        certificate: Option<&[u8]>,
    ) -> Result<(), RepositoryError> {
        if object.as_bytes()[31] & 0x40 == 0 {
            return self.publish_link(object);
        }
        let policy = self.policy()?;
        if policy.unsigned {
            return self.publish_link(object);
        }
        let mut name = *object.as_bytes();
        name[31] &= 0xe0;
        let certificate = match certificate {
            Some(bytes) => alloc::vec![Certificate::decode(bytes)?],
            None => Self::issue_certificates(name, &policy)?,
        };
        if !certificate
            .iter()
            .any(|certificate| certificate.verify(&name, &policy).is_ok())
        {
            return Err(fixcertificates::Error::Untrusted.into());
        }
        self.write_certificates(object.as_bytes(), &certificate)
    }

    fn issue_certificates(
        name: [u8; 32],
        policy: &Policy,
    ) -> Result<Vec<Certificate>, RepositoryError> {
        if policy.identities.is_empty() {
            return Err(
                fixcertificates::Error::Signing("no signing identity configured".into()).into(),
            );
        }
        policy
            .identities
            .iter()
            .map(|identity| {
                let signature = fixcertificates::sign(
                    fixcertificates::TAG_NAMESPACE,
                    &name,
                    identity,
                    |identity, namespace, message| {
                        #[cfg(target_os = "linux")]
                        {
                            sshagent::sign(identity, namespace, message).map_err(|error| {
                                fixcertificates::Error::Signing(alloc::format!("{error}"))
                            })
                        }
                        #[cfg(not(target_os = "linux"))]
                        {
                            kernel::host::os::ssh_sign(identity, namespace, message)
                                .map_err(fixcertificates::Error::Signing)
                        }
                    },
                )?;
                Ok(Certificate { name, signature })
            })
            .collect()
    }

    pub fn tag_certificates(&self, object: Handle) -> Result<Vec<Certificate>, RepositoryError> {
        let mut name = *object.as_bytes();
        name[31] = (name[31] & 0xe0) | 0x10;
        self.lookup(&name)?;
        if name[31] & 0x40 == 0 {
            return Err(Error::WrongType.into());
        }
        self.available_certificates(&name)
    }

    pub fn available_certificates(
        &self,
        name: &[u8; 32],
    ) -> Result<Vec<Certificate>, RepositoryError> {
        let mut object = *name;
        object[31] &= 0xe0;
        let mut reference = object;
        reference[31] |= 0x10;
        let mut combined = alloc::collections::BTreeMap::new();
        let mut found = false;
        for name in [object, reference] {
            if let Some(certificates) = self.stored_certificates(name)? {
                found = true;
                for certificate in certificates {
                    combined.insert(certificate.signer()?, certificate);
                }
            }
        }
        if !found {
            return Err(RepositoryError::Io(ErrorKind::NotFound));
        }
        Ok(combined.into_values().collect())
    }

    pub fn retain_certificates(
        &self,
        name: &[u8; 32],
        certificates: &[Certificate],
    ) -> Result<(), RepositoryError> {
        self.available_certificates(name)?;
        for certificate in certificates {
            let key = certificate.signer()?;
            certificate.verify(
                name,
                &Policy {
                    authorized_keys: alloc::vec![key],
                    ..Policy::default()
                },
            )?;
        }
        self.write_certificates(name, certificates)
    }

    pub fn export_certificates(&self, object: Handle) -> Result<Vec<Certificate>, RepositoryError> {
        let mut certificates = self.tag_certificates(object)?;
        let policy = self.policy()?;
        if !policy.identities.is_empty() {
            for certificate in Self::issue_certificates(
                certificates.first().map_or_else(
                    || {
                        let mut name = *object.as_bytes();
                        name[31] &= 0xe0;
                        name
                    },
                    |certificate| certificate.name,
                ),
                &policy,
            )? {
                let signer = certificate.signer()?;
                certificates.retain(|previous| previous.signer().ok().as_ref() != Some(&signer));
                certificates.push(certificate);
            }
        }
        Ok(certificates)
    }

    pub fn store_certificates(
        &self,
        name: &[u8; 32],
        certificates: &[Certificate],
    ) -> Result<(), RepositoryError> {
        let policy = self.policy()?;
        let mut object = *name;
        object[31] &= 0xe0;
        if !certificates
            .iter()
            .any(|certificate| certificate.verify(&object, &policy).is_ok())
        {
            return Err(fixcertificates::Error::Untrusted.into());
        }
        self.write_certificates(name, certificates)
    }

    fn certificate_directory(&self, name: &[u8; 32]) -> String {
        let mut object = *name;
        object[31] &= 0xe0;
        format!("{}/tags/signatures/{}", self.path, hex::encode(object))
    }

    fn write_certificates(
        &self,
        name: &[u8; 32],
        certificates: &[Certificate],
    ) -> Result<(), RepositoryError> {
        let mut object = *name;
        object[31] &= 0xe0;
        let directory = self.certificate_directory(name);
        fs::mkdir(&directory)?;
        for certificate in certificates {
            if certificate.name != object {
                return Err(RepositoryError::InvalidRepository);
            }
            let signer = Blob::canonical(certificate.signer()?.as_bytes())?;
            let blob = self.create_blob(&certificate.encode()?)?;
            self.write_atomic(
                &format!("{directory}/{}", signer.as_handle()),
                blob.as_handle().as_bytes(),
            )?;
        }
        self.publish_link(unsafe { Handle::parse(*name)? })
    }

    pub fn certificate_blobs(&self, name: &[u8; 32]) -> Result<Vec<Blob>, RepositoryError> {
        let directory = self.certificate_directory(name);
        let mut entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(ErrorKind::NotFound) => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        entries.sort();
        entries
            .iter()
            .map(|entry| {
                let bytes = fs::read(&format!("{directory}/{entry}"))?;
                let name = bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| RepositoryError::InvalidRepository)?;
                let view = Handle::parse_ref(name)?;
                if !view.is_canonical() || view.is_tree_data() || name[31] & 0x1f != 0 {
                    return Err(RepositoryError::InvalidRepository);
                }
                Ok(Blob::try_from(self.lookup(name)?)?)
            })
            .collect()
    }

    fn publish_link(&self, object: Handle) -> Result<(), RepositoryError> {
        let key = Self::data_key(object.as_bytes());
        let target = format!("../data/{key}");
        let record = format!("{}/objects/{object}", self.path);
        match fs::symlink(&target, &record) {
            Ok(()) => (),
            Err(ErrorKind::AlreadyExists) if fs::read_link(&record)? == target => (),
            Err(error) => return Err(error.into()),
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
