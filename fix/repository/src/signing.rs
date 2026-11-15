use super::*;
use alloc::collections::{BTreeMap, BTreeSet};

impl Repository {
    fn tag_records(&self) -> Result<Vec<[u8; 32]>, RepositoryError> {
        let mut records = Vec::new();
        for entry in fs::read_dir(&format!("{}/objects", self.path))? {
            let mut name = [0; 32];
            if hex::decode_to_slice(&entry, &mut name).is_ok() && name[31] & 0x40 != 0 {
                records.push(name);
            }
        }
        Ok(records)
    }

    pub(super) fn stored_certificates(
        &self,
        name: [u8; 32],
    ) -> Result<Option<Vec<Certificate>>, RepositoryError> {
        let view = Handle::parse_ref(&name)?;
        if !view.is_canonical() {
            return Err(RepositoryError::NonCanonical);
        }
        if !view.is_tree_data() || name[31] & 0x40 == 0 || name[31] & 0x0f != 0 {
            return Err(Error::WrongType.into());
        }
        let mut object = name;
        object[31] &= 0xe0;
        let path = format!("{}/objects/{}", self.path, hex::encode(name));
        match fs::read_link(&path) {
            Ok(target) if target == format!("../data/{}", Self::data_key(&object)) => (),
            Ok(_) => return Err(RepositoryError::InvalidRepository),
            Err(ErrorKind::NotFound) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let certificate = self
            .certificate_blobs(&name)?
            .iter()
            .map(|blob| Ok(Certificate::decode(&self.read_blob(*blob)?)?))
            .collect::<Result<Vec<_>, RepositoryError>>()?;
        if certificate
            .iter()
            .any(|certificate| certificate.name != object)
        {
            return Err(RepositoryError::InvalidRepository);
        }
        // Explicit signing authorizes this name; its available backing must still match.
        let tree = Tree::try_from(unsafe { Handle::parse(object)? })?;
        match self.read_data(tree.into()) {
            Ok(bytes) => tree.verify_bytes(&bytes)?,
            Err(RepositoryError::Io(ErrorKind::NotFound)) => (),
            Err(error) => return Err(error),
        }
        Ok(Some(certificate))
    }

    pub fn inspect_tags(&self, names: &[[u8; 32]]) -> Result<(), RepositoryError> {
        for name in names {
            self.stored_certificates(*name)?
                .ok_or(RepositoryError::Io(ErrorKind::NotFound))?;
        }
        Ok(())
    }

    pub fn sign_tag(&self, value: &str) -> Result<(), RepositoryError> {
        let text = if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            value.into()
        } else {
            String::from_utf8(fs::read(&self.label_path(value)?)?)
                .map_err(|_| RepositoryError::InvalidRepository)?
        };
        let mut name = [0; 32];
        hex::decode_to_slice(text.trim(), &mut name)
            .map_err(|_| RepositoryError::InvalidRepository)?;
        self.sign_tags(&[name])
    }

    pub fn sign_all_tags(&self) -> Result<(), RepositoryError> {
        self.sign_tags(&self.tag_records()?)
    }

    pub fn sign_tags(&self, names: &[[u8; 32]]) -> Result<(), RepositoryError> {
        let mut objects = BTreeSet::new();
        for name in names {
            let view = Handle::parse_ref(name)?;
            if !view.is_canonical()
                || !view.is_tree_data()
                || name[31] & 0x40 == 0
                || name[31] & 0x0f != 0
            {
                return Err(Error::WrongType.into());
            }
            let mut object = *name;
            object[31] &= 0xe0;
            objects.insert(object);
        }
        let mut certificates = BTreeMap::new();
        let mut records = Vec::new();
        for object in objects {
            let mut combined = BTreeMap::new();
            let mut found = false;
            for flags in [0, 0x10] {
                let mut name = object;
                name[31] |= flags;
                if let Some(certificate) = self.stored_certificates(name)? {
                    found = true;
                    records.push(name);
                    for certificate in certificate {
                        combined.insert(certificate.signer()?, certificate);
                    }
                }
            }
            if !found {
                return Err(RepositoryError::Io(ErrorKind::NotFound));
            }
            certificates.insert(object, combined);
        }
        let policy = self.policy()?;
        for (name, existing) in &mut certificates {
            for certificate in Self::issue_certificates(*name, &policy)? {
                existing.insert(certificate.signer()?, certificate);
            }
        }
        for name in records {
            let mut object = name;
            object[31] &= 0xe0;
            let certificates: Vec<_> = certificates[&object].values().cloned().collect();
            self.write_certificates(&name, &certificates)?;
        }
        self.index_trusted_tags()
    }

    pub fn index_trusted_tags(&self) -> Result<(), RepositoryError> {
        for name in self.tag_records()? {
            if name[31] & 0x10 != 0 {
                continue;
            }
            let Ok(handle) = self.lookup(&name) else {
                continue;
            };
            let tag = Tree::try_from(handle)?;
            if self.read_tree(tag).is_ok() {
                self.index_tag(tag)?;
            }
        }
        Ok(())
    }
}
