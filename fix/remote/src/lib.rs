#![no_std]

extern crate alloc;
#[cfg(target_os = "linux")]
extern crate std;

use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
use fixcertificates::{Certificate, Policy};
use fixhandle::{Handle, HandleView, Name, Object, ObjectView};
pub use fixrepository::Remote;
use fixrepository::{Repository, RepositoryError};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "linux")]
pub mod linux;
pub use fixtransport as transport;

pub use fixprotocol as protocol;
use protocol::{CertificateRef, Offer, Record, Transfer};

#[derive(Debug)]
pub enum Error {
    Invalid,
    Untrusted,
    Signing(String),
    Repository(RepositoryError),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Invalid => f.write_str("invalid object transfer"),
            Self::Untrusted => f.write_str("untrusted remote runtime"),
            Self::Signing(error) => write!(f, "could not sign object transfer: {error}"),
            Self::Repository(error) => write!(f, "repository error: {error:?}"),
        }
    }
}
impl core::error::Error for Error {}

impl From<RepositoryError> for Error {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub policy: Policy,
    #[serde(default)]
    pub remotes: Vec<Remote>,
}

impl Config {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let config: Self = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), Error> {
        for key in self
            .policy
            .authorized_keys
            .iter()
            .chain(&self.policy.identities)
        {
            fixcertificates::public_key(key).map_err(|_| Error::Invalid)?;
        }
        Remote::validate_all(&self.remotes).map_err(|_| Error::Invalid)?;
        Ok(())
    }
}

pub fn export(
    repository: &Repository,
    roots: &[Handle],
    without_backing: &[Object],
) -> Result<Vec<u8>, Error> {
    let source = Repository::open(repository.path())?.unsigned();
    let repository = &source;
    for root in roots {
        repository.lookup(root.as_bytes())?;
    }
    let mut objects = Vec::new();
    repository.visit(roots, without_backing, |object, include| {
        objects.push((object, include));
        Ok(())
    })?;
    let mut records = BTreeMap::new();
    let mut certificate_blobs = BTreeMap::new();
    for (object, include) in objects {
        let data = if include && repository.has_backing(object)? {
            Some(match object.view() {
                ObjectView::Blob(blob) => repository.read_blob(*blob)?.into_vec(),
                ObjectView::Tree(tree) => repository
                    .read_tree(*tree)?
                    .iter()
                    .flat_map(|child| *child.as_bytes())
                    .collect(),
            })
        } else {
            None
        };
        let mut certificates = Vec::new();
        if matches!(object.view(), ObjectView::Tree(tree) if tree.is_tag()) {
            let available: BTreeMap<_, _> = repository
                .tag_certificates(object.into())?
                .into_iter()
                .map(|certificate| {
                    Ok((
                        certificate.signer().map_err(|_| Error::Invalid)?,
                        certificate,
                    ))
                })
                .collect::<Result<_, Error>>()?;
            for (key, certificate) in available {
                let bytes = certificate.encode().map_err(|_| Error::Invalid)?;
                let blob = fixhandle::Blob::canonical(&bytes).map_err(|_| Error::Invalid)?;
                let name = *blob.as_handle().as_bytes();
                certificate_blobs.insert(
                    name,
                    Record {
                        name,
                        data: Some(bytes),
                        todo: None,
                        certificates: Vec::new(),
                    },
                );
                certificates.push(CertificateRef { key, blob: name });
            }
        }
        let name = *object.as_handle().as_bytes();
        let record = records.entry(name).or_insert(Record {
            name,
            data: None,
            todo: None,
            certificates,
        });
        if data.is_some() {
            record.data = data;
        }
        record.todo = repository
            .get_todo(object.into_ref())?
            .map(|value| *value.as_bytes());
    }
    Transfer {
        roots: roots.iter().map(|value| *value.as_bytes()).collect(),
        certificates: certificate_blobs
            .into_iter()
            .filter(|(name, _)| !records.contains_key(name))
            .map(|(_, record)| record)
            .collect(),
        records: records.into_values().collect(),
    }
    .encode()
    .map_err(|_| Error::Invalid)
}

pub fn certify(
    repository: &Repository,
    name: [u8; 32],
    trusted_keys: &[String],
    identities: &[String],
    mut sign: impl FnMut(&str, &str, &[u8]) -> Result<String, fixcertificates::Error>,
) -> Result<(CertificateRef, Record), Error> {
    let mut certificates = repository.available_certificates(&name)?;
    let certificate = if let Some(certificate) = certificates.iter().find(|certificate| {
        certificate.signer().is_ok_and(|key| {
            trusted_keys.contains(&key)
                && certificate
                    .verify(
                        &name,
                        &Policy {
                            authorized_keys: alloc::vec![key],
                            ..Policy::default()
                        },
                    )
                    .is_ok()
        })
    }) {
        certificate.clone()
    } else {
        let key = identities
            .iter()
            .find(|identity| {
                fixcertificates::public_key(identity)
                    .and_then(|key| fixcertificates::canonical_key(&key))
                    .is_ok_and(|key| trusted_keys.contains(&key))
            })
            .ok_or(Error::Untrusted)?;
        let signature =
            fixcertificates::sign(fixcertificates::TAG_NAMESPACE, &name, key, &mut sign)
                .map_err(|error| Error::Signing(alloc::format!("{error}")))?;
        let certificate = Certificate { name, signature };
        certificates.push(certificate.clone());
        repository.retain_certificates(&name, &certificates)?;
        certificate
    };
    let key = certificate.signer().map_err(|_| Error::Invalid)?;
    let bytes = certificate.encode().map_err(|_| Error::Invalid)?;
    let blob = fixhandle::Blob::canonical(&bytes).map_err(|_| Error::Invalid)?;
    let name = *blob.as_handle().as_bytes();
    Ok((
        CertificateRef { key, blob: name },
        Record {
            name,
            data: Some(bytes),
            todo: None,
            certificates: Vec::new(),
        },
    ))
}

pub fn information(
    repository: &Repository,
    collector: Handle,
    parallelism: u32,
    memory_bytes: Option<u64>,
) -> Result<protocol::Information, Error> {
    let inventory = repository.inventory()?;
    let mut results = Vec::new();
    for handle in &inventory {
        if let Ok(tag) = fixhandle::Tree::try_from(*handle)
            && tag.is_tag()
            && tag.len() == 2
        {
            let fields = repository.read_tree(tag)?;
            if !fields[0].same_encoding(&collector)
                && !fields[0].same_encoding(&fixhandle::Tree::empty().into())
            {
                continue;
            }
            let Ok(reference) = fixhandle::Ref::try_from(fields[1]) else {
                continue;
            };
            let Ok(tree) = fixhandle::Tree::try_from(Handle::from(reference.object_descriptor()))
            else {
                continue;
            };
            let Ok(claim) = repository.read_tree(tree) else {
                continue;
            };
            if claim.len() != 3 {
                continue;
            }
            let Ok(kind) = fixhandle::Blob::try_from(claim[0]) else {
                continue;
            };
            let bytes = repository.read_blob(kind)?;
            let Ok(bytes) = bytes.as_ref().try_into() else {
                continue;
            };
            let operation = u64::from_le_bytes(bytes);
            if matches!(operation, 1 | 2 | 3 | 4 | 6) {
                results.push(protocol::Task {
                    operation,
                    collector: *collector.as_bytes(),
                    input: *claim[1].as_bytes(),
                });
            }
        }
    }
    Ok(protocol::Information {
        parallelism,
        memory_bytes,
        link_speed: 0.0,
        data: inventory.iter().map(|handle| *handle.as_bytes()).collect(),
        results,
    })
}

pub fn trusted_keys(config: &Config) -> Result<Vec<String>, Error> {
    config
        .policy
        .authorized_keys
        .iter()
        .map(|key| {
            let key = fixcertificates::public_key(key).map_err(|_| Error::Invalid)?;
            fixcertificates::canonical_key(&key).map_err(|_| Error::Invalid)
        })
        .collect()
}

pub fn missing(repository: &Repository, offers: &[Offer]) -> Result<Vec<[u8; 32]>, Error> {
    let mut names = Vec::new();
    for offer in offers {
        let handle = canonical(&offer.name)?;
        let object = Object::try_from(handle).map_err(|_| Error::Invalid)?;
        let available = if offer.backing {
            repository.has_backing(object).unwrap_or(false)
        } else {
            repository
                .lookup(object.into_ref().as_handle().as_bytes())
                .is_ok()
        };
        if !available {
            names.push(offer.name);
        }
    }
    Ok(names)
}

pub fn import(
    repository: &Repository,
    bytes: &[u8],
    config: &Config,
) -> Result<Vec<Handle>, Error> {
    let mut bundle = Transfer::decode(bytes).map_err(|_| Error::Invalid)?;
    bundle.records.append(&mut bundle.certificates);
    let records: BTreeMap<_, _> = bundle
        .records
        .iter()
        .map(|record| (record.name, record))
        .collect();
    if records.len() != bundle.records.len() {
        return Err(Error::Invalid);
    }
    let certificate_bytes = |name: &[u8; 32]| -> Result<Vec<u8>, Error> {
        let blob = fixhandle::Blob::try_from(canonical(name)?).map_err(|_| Error::Invalid)?;
        let data = if let Some(data) = records.get(name).and_then(|record| record.data.as_ref()) {
            data.clone()
        } else {
            repository.read_blob(blob)?.into_vec()
        };
        blob.verify(&data).map_err(|_| Error::Invalid)?;
        Ok(data)
    };
    let mut certificates = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut backed = BTreeSet::new();
    for record in &bundle.records {
        let object = Object::try_from(canonical(&record.name)?).map_err(|_| Error::Invalid)?;
        if matches!(object.view(), ObjectView::Tree(tree) if tree.is_tag()) {
            let available = if record.certificates.is_empty() {
                repository.available_certificates(object.as_handle().as_bytes())?
            } else {
                record
                    .certificates
                    .iter()
                    .map(|reference| {
                        let bytes = certificate_bytes(&reference.blob)?;
                        let certificate =
                            Certificate::decode(&bytes).map_err(|_| Error::Invalid)?;
                        if certificate.name != record.name
                            || certificate.signer().map_err(|_| Error::Invalid)? != reference.key
                        {
                            return Err(Error::Invalid);
                        }
                        certificate
                            .verify(
                                &record.name,
                                &Policy {
                                    authorized_keys: alloc::vec![reference.key.clone()],
                                    ..Policy::default()
                                },
                            )
                            .map_err(|_| Error::Invalid)?;
                        Ok(certificate)
                    })
                    .collect::<Result<Vec<_>, Error>>()?
            };
            if !available
                .iter()
                .any(|certificate| certificate.verify(&record.name, &config.policy).is_ok())
            {
                return Err(Error::Untrusted);
            }
            certificates.insert(record.name, available);
        } else if !record.certificates.is_empty() {
            return Err(Error::Invalid);
        }
        if matches!(object.as_handle().name(), Name::Literal(_)) || !names.insert(record.name) {
            return Err(Error::Invalid);
        }
        if let Some(bytes) = &record.data {
            backed.insert(record.name);
            match object.view() {
                ObjectView::Blob(blob) => blob.verify(bytes).map_err(|_| Error::Invalid)?,
                ObjectView::Tree(tree) => {
                    tree.verify_bytes(bytes).map_err(|_| Error::Invalid)?;
                }
            }
        }
    }
    let admitted = |bytes: &[u8; 32]| -> Result<(), Error> {
        let handle = canonical(bytes)?;
        let mut object = *bytes;
        object[31] &= 0xe0;
        let supplied = if matches!(handle.view(), HandleView::Object(_)) {
            backed.contains(&object)
        } else {
            names.contains(&object)
        };
        let existing = if matches!(handle.view(), HandleView::Object(_)) {
            let value = Object::try_from(handle).map_err(|_| Error::Invalid)?;
            repository.has_backing(value).unwrap_or(false)
        } else {
            repository.lookup(handle.as_bytes()).is_ok()
        };
        if supplied || existing {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    };
    for root in &bundle.roots {
        admitted(root)?;
    }
    for record in &bundle.records {
        if let Some(todo) = &record.todo {
            admitted(todo)?;
        }
        for certificate in &record.certificates {
            admitted(&certificate.blob)?;
        }
        if matches!(canonical(&record.name)?.view(), HandleView::Object(object)
            if matches!(object.view(), ObjectView::Tree(_)))
            && let Some(bytes) = &record.data
        {
            for child in bytes.as_chunks::<32>().0 {
                admitted(child)?;
            }
        }
    }

    for record in &bundle.records {
        let object = Object::try_from(canonical(&record.name)?).map_err(|_| Error::Invalid)?;
        if let Some(certificate) = certificates.get(&record.name) {
            repository.store_certificates(object.into_ref().as_handle().as_bytes(), certificate)?;
        } else {
            repository.publish_reference(object.into_ref())?;
        }
    }
    for record in bundle.records.iter().rev() {
        if let Some(data) = &record.data {
            repository.publish_certified(
                canonical(&record.name)?,
                data,
                certificates
                    .get(&record.name)
                    .and_then(|certificates| {
                        certificates.iter().find(|certificate| {
                            certificate.verify(&record.name, &config.policy).is_ok()
                        })
                    })
                    .map(Certificate::encode)
                    .transpose()
                    .map_err(|_| Error::Invalid)?
                    .as_deref(),
            )?;
        }
    }
    for record in &bundle.records {
        let object = Object::try_from(canonical(&record.name)?).map_err(|_| Error::Invalid)?;
        if record.data.is_some()
            && let Some(certificates) = certificates.get(&record.name)
        {
            repository.store_certificates(&record.name, certificates)?;
        }
        if let ObjectView::Tree(tree) = object.view()
            && tree.is_tag()
            && record.data.is_some()
        {
            repository.index_tag(*tree)?;
        }
        if let Some(todo) = &record.todo {
            repository.set_todo(object.into_ref(), canonical(todo)?)?;
        }
    }
    bundle
        .roots
        .iter()
        .map(|name| repository.lookup(name).map_err(Error::from))
        .collect()
}

fn canonical(bytes: &[u8; 32]) -> Result<Handle, Error> {
    let view = Handle::parse_ref(bytes).map_err(|_| Error::Invalid)?;
    if !view.is_canonical() {
        return Err(Error::Invalid);
    }
    // Only canonical machine names enter admission; backing and Tag certificates are checked before publication.
    Ok(unsafe { view.to_owned() })
}

#[cfg(test)]
mod tests;
