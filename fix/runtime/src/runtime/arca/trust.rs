use super::*;
use crate::storage::StorageError;
use hashbrown::HashSet;

#[derive(Debug, Default)]
pub struct Trust {
    authors: HashSet<[u8; 32]>,
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
        Ok(Self { authors })
    }

    pub fn runnable(&self, storage: &dyn Storage, tag: Tree) -> Result<Blob, TrustError> {
        if !tag.is_tag() || tag.len() != 3 {
            return Err(TrustError::InvalidRunnable);
        }
        let fields = storage.get_tree(tag)?;
        let authors = Tree::try_from(fields[0]).map_err(|_| TrustError::InvalidRunnable)?;
        let executable = Blob::try_from(fields[1]).map_err(|_| TrustError::InvalidRunnable)?;
        let claim = Blob::try_from(fields[2]).map_err(|_| TrustError::InvalidRunnable)?;
        if &*storage.get_blob(claim)? != b"Runnable" {
            return Err(TrustError::InvalidRunnable);
        }
        for author in storage.get_tree(authors)? {
            let canonical: Handle = storage.canonicalize(author)?.into();
            if !self.authors.contains(canonical.as_bytes()) {
                return Err(TrustError::UntrustedAuthority);
            }
        }
        Ok(executable)
    }
}
