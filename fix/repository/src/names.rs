use super::*;
use alloc::collections::BTreeSet;

impl Repository {
    /// Resolve a full handle, exact label, or unambiguous hexadecimal abbreviation.
    pub fn resolve_name(&self, name: &str) -> Result<[u8; 32], RepositoryError> {
        self.resolve_name_with_sources(name, core::iter::empty())
    }

    /// Include source repositories when expanding an abbreviation; labels remain local.
    pub fn resolve_name_with_sources<'a>(
        &'a self,
        name: &str,
        sources: impl IntoIterator<Item = &'a Repository>,
    ) -> Result<[u8; 32], RepositoryError> {
        if name.len() == 64 && name.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            let mut bytes = [0; 32];
            hex::decode_to_slice(name, &mut bytes).map_err(|_| Error::InvalidName)?;
            return Ok(bytes);
        }
        match self.label_name(name) {
            Ok(bytes) => return Ok(bytes),
            Err(RepositoryError::MissingLabel { .. }) => (),
            Err(error) => return Err(error),
        }
        let (prefix, kind) = match name.split_once('.') {
            Some((prefix, kind)) if kind.len() == 2 => {
                let mut byte = [0];
                if hex::decode_to_slice(kind, &mut byte).is_err() {
                    return self.label_name(name);
                }
                (prefix, Some(byte[0]))
            }
            Some(_) => return self.label_name(name),
            None => (name, None),
        };
        if prefix.is_empty()
            || prefix.len() > if kind.is_some() { 62 } else { 63 }
            || !prefix.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return self.label_name(name);
        }
        let prefix = prefix.to_ascii_lowercase();
        let mut matches = BTreeSet::new();
        for repository in core::iter::once(self).chain(sources) {
            let mut candidates = Vec::new();
            for entry in fs::read_dir(&format!("{}/objects", repository.path))? {
                let mut bytes = [0; 32];
                if hex::decode_to_slice(entry, &mut bytes).is_ok() {
                    candidates.push(bytes);
                }
            }
            for label in repository.labels()? {
                candidates.push(repository.label_name(&label)?);
            }
            for mut bytes in candidates {
                if !hex::encode(bytes).starts_with(&prefix) {
                    continue;
                }
                if let Some(kind) = kind {
                    bytes[31] = kind;
                }
                if Handle::parse_ref(&bytes).is_ok_and(|view| view.is_canonical()) {
                    matches.insert(bytes);
                }
            }
        }
        match matches.len() {
            0 => Err(RepositoryError::MissingObject {
                name: name.into(),
                repository: self.path.clone(),
            }),
            1 => Ok(*matches.first().unwrap()),
            _ => Err(RepositoryError::AmbiguousName {
                name: name.into(),
                matches: matches.into_iter().map(hex::encode).collect(),
            }),
        }
    }
}
