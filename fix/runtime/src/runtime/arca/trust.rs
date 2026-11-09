use super::*;
use crate::storage::StorageError;
use hashbrown::HashSet;

pub(super) fn is_kernel_author(author: Handle) -> bool {
    Tree::try_from(author).is_ok_and(|tree| tree.is_empty() && !tree.is_tag())
}

pub(super) fn is_author(
    storage: &dyn Storage,
    combination: Tree,
    author: Handle,
) -> Result<bool, StorageError> {
    if !author.is_comparable() {
        return Ok(false);
    }
    let identity = |value: Handle| match value.view() {
        HandleView::Ref(reference) => reference.object_descriptor().into(),
        _ => value,
    };
    let author = identity(author);
    let mut path = Vec::new();
    let mut current: Handle = combination.into();
    loop {
        let candidate = identity(current);
        if author.same_encoding(&candidate) {
            return Ok(true);
        }
        path.push(candidate);
        let Ok(tree) = Tree::try_from(current) else {
            break;
        };
        let fields = storage.get_tree(tree)?;
        let Some(next) = fields.get(1) else { break };
        current = *next;
    }
    for current in path {
        if !current.is_comparable()
            || (author.is_canonical() && current.is_canonical())
            || author.is_tree_data() != current.is_tree_data()
            || author.len() != current.len()
        {
            continue;
        }
        if storage.equals(author, current)? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(Debug, Default)]
pub struct Trust {
    authors: HashSet<[u8; 32]>,
    taggers: HashSet<[u8; 32]>,
}

#[derive(Debug)]
pub enum TrustError {
    InvalidAuthority,
    InvalidRunnable,
    UntrustedAuthority,
    Storage(StorageError),
}

impl From<StorageError> for TrustError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl core::fmt::Display for TrustError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidAuthority => {
                f.write_str("trust file requires canonical handle names, one per line")
            }
            Self::InvalidRunnable => f.write_str("expected a Runnable Tag containing an ELF Blob"),
            Self::UntrustedAuthority => f.write_str("Runnable certificate has an untrusted author"),
            Self::Storage(error) => write!(f, "Runnable certificate storage error: {error:?}"),
        }
    }
}

impl Trust {
    pub fn parse(text: &str) -> Result<Self, TrustError> {
        let mut authors = HashSet::new();
        for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let mut name = [0; 32];
            hex::decode_to_slice(line, &mut name).map_err(|_| TrustError::InvalidAuthority)?;
            let handle = Handle::parse_ref(&name).map_err(|_| TrustError::InvalidAuthority)?;
            if !handle.is_canonical() {
                return Err(TrustError::InvalidAuthority);
            }
            authors.insert(name);
        }
        Ok(Self {
            authors,
            taggers: HashSet::new(),
        })
    }

    pub fn add_tagger(&mut self, storage: &dyn Storage, callable: Tree) -> Result<(), TrustError> {
        self.runnable(storage, callable)?;
        let identity: Handle = storage.canonicalize(callable.into())?.into();
        self.taggers.insert(*identity.as_bytes());
        Ok(())
    }

    pub(super) fn is_tagger(
        &self,
        storage: &dyn Storage,
        combination: Tree,
    ) -> Result<bool, TrustError> {
        if self.taggers.is_empty() {
            return Ok(false);
        }
        let mut current = combination;
        loop {
            let fields = storage.get_tree(current)?;
            let next = *fields.get(1).ok_or(TrustError::InvalidRunnable)?;
            if Blob::try_from(next).is_ok() {
                self.runnable(storage, current)?;
                let identity: Handle = storage.canonicalize(current.into())?.into();
                return Ok(self.taggers.contains(identity.as_bytes()));
            }
            current = Tree::try_from(next).map_err(|_| TrustError::InvalidRunnable)?;
        }
    }

    pub fn runnable(&self, storage: &dyn Storage, tag: Tree) -> Result<Blob, TrustError> {
        if !tag.is_tag() || tag.len() != 3 {
            return Err(TrustError::InvalidRunnable);
        }
        let fields = storage.get_tree(tag)?;
        let author = fields[0];
        let executable = Blob::try_from(fields[1]).map_err(|_| TrustError::InvalidRunnable)?;
        let claim = Blob::try_from(fields[2]).map_err(|_| TrustError::InvalidRunnable)?;
        if &*storage.get_blob(claim)? != b"Runnable" {
            return Err(TrustError::InvalidRunnable);
        }
        if !is_kernel_author(author) {
            let canonical: Handle = storage.canonicalize(author)?.into();
            if !self.authors.contains(canonical.as_bytes()) {
                return Err(TrustError::UntrustedAuthority);
            }
        }
        Ok(executable)
    }
}
