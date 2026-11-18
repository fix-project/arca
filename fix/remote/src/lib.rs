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
use fixhandle::{Blob, Handle, HandleView, Name, Object, ObjectView};
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
    Transfer(String),
    Untrusted,
    Signing(String),
    Repository(RepositoryError),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Invalid => f.write_str("invalid object transfer"),
            Self::Transfer(message) => f.write_str(message),
            Self::Untrusted => f.write_str("untrusted remote runtime"),
            Self::Signing(error) => write!(f, "could not sign object transfer: {error}"),
            Self::Repository(error) => write!(f, "repository error: {error}"),
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
    export_plan(repository, roots, without_backing, true, &[])?
        .0
        .encode()
        .map_err(|_| Error::Invalid)
}

fn export_plan(
    repository: &Repository,
    roots: &[Handle],
    without_backing: &[Object],
    load: bool,
    definitions: &[Handle],
) -> Result<(Transfer, Vec<[u8; 32]>), Error> {
    let source = Repository::open(repository.path())?.unsigned();
    let repository = &source;
    for root in roots {
        repository.lookup(root.as_bytes())?;
    }
    let mut objects = BTreeMap::new();
    let mut order = Vec::new();
    let mut ordered = BTreeSet::new();
    let mut pending = roots.to_vec();
    let mut seen = BTreeSet::new();
    while let Some(root) = pending.pop() {
        export_metadata(repository, root, &mut seen, &mut pending, &mut objects)?;
        repository.visit(
            root,
            without_backing,
            definitions.iter().any(|value| value.same_encoding(&root)),
            |object| {
                let include = !without_backing
                    .iter()
                    .any(|excluded| excluded.as_handle().same_encoding(object.as_handle()));
                let key = *object.as_handle().as_bytes();
                if ordered.insert(key) {
                    order.push(key);
                }
                objects
                    .entry(key)
                    .and_modify(|(_, old)| *old |= include)
                    .or_insert((object, include));
                export_metadata(
                    repository,
                    object.into(),
                    &mut seen,
                    &mut pending,
                    &mut objects,
                )?;
                if let ObjectView::Tree(tree) = object.view()
                    && include
                    && repository.has_backing(object)?
                {
                    for (index, child) in repository.read_tree(*tree)?.iter().enumerate() {
                        if !tree.is_tag() || index != 0 {
                            export_metadata(
                                repository,
                                *child,
                                &mut seen,
                                &mut pending,
                                &mut objects,
                            )?;
                        }
                    }
                }
                Ok::<_, Error>(())
            },
        )?;
    }
    let mut records = BTreeMap::new();
    let mut backing = Vec::new();
    let mut certificate_blobs = BTreeMap::new();
    for (object, include) in objects.into_values() {
        let available = include && repository.has_backing(object)?;
        if available {
            backing.push(*object.as_handle().as_bytes());
        }
        let data = if available && load {
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
            let available = if !available
                && repository
                    .certificate_blobs(object.as_handle().as_bytes())?
                    .is_empty()
            {
                Vec::new()
            } else {
                repository.tag_certificates(object.into())?
            };
            let available: BTreeMap<_, _> = available
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
        let todo = repository
            .get_todo(object.into_ref())?
            .map(|value| *value.as_bytes());
        if !available && certificates.is_empty() && todo.is_none() {
            continue;
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
        record.todo = todo;
    }
    let certificates = certificate_blobs
        .into_iter()
        .filter(|(name, _)| !records.contains_key(name))
        .map(|(_, record)| record)
        .collect();
    let mut ordered = Vec::new();
    for name in order {
        if let Some(record) = records.remove(&name) {
            ordered.push(record);
        }
    }
    ordered.extend(records.into_values());
    let transfer = Transfer {
        roots: roots.iter().map(|value| *value.as_bytes()).collect(),
        certificates,
        records: ordered,
    };
    backing.sort_unstable();
    backing.dedup();
    Ok((transfer, backing))
}

fn export_metadata(
    repository: &Repository,
    value: Handle,
    seen: &mut BTreeSet<[u8; 32]>,
    pending: &mut Vec<Handle>,
    objects: &mut BTreeMap<[u8; 32], (Object, bool)>,
) -> Result<(), Error> {
    let object = value.object_descriptor();
    let key = *object.as_handle().as_bytes();
    if !seen.insert(key) {
        return Ok(());
    }
    if let Some(metadata) = repository.get_todo(object.into_ref())? {
        objects.entry(key).or_insert((object, false));
        pending.push(metadata);
    }
    if !matches!(value.view(), HandleView::Object(_)) {
        objects.entry(key).or_insert((object, false));
    }
    Ok(())
}

pub fn propose_export(
    repository: &Repository,
    root: Handle,
    backing: bool,
) -> Result<(Transfer, Vec<[u8; 32]>), Error> {
    let omitted = if backing {
        Vec::new()
    } else {
        match root.view() {
            HandleView::Object(object) => alloc::vec![*object],
            _ => Vec::new(),
        }
    };
    let roots = if backing {
        coupon_roots(repository, &[root], &omitted)?
    } else {
        alloc::vec![root]
    };
    let (mut transfer, objects) = export_plan(repository, &roots, &omitted, false, &[])?;
    transfer.roots = alloc::vec![*root.as_bytes()];
    Ok((transfer, objects))
}

/// Propose a complete computation's accessible backing rather than only its minimum repository.
pub fn propose_export_full(
    repository: &Repository,
    root: Handle,
) -> Result<(Transfer, Vec<[u8; 32]>), Error> {
    let roots = coupon_roots(repository, &[root], &[])?;
    let (mut transfer, objects) = export_plan(repository, &roots, &[], false, &[root])?;
    transfer.roots = alloc::vec![*root.as_bytes()];
    Ok((transfer, objects))
}

pub fn export_with_coupons(
    repository: &Repository,
    roots: &[Handle],
    without_backing: &[Object],
) -> Result<Vec<u8>, Error> {
    let selected = coupon_roots(repository, roots, without_backing)?;
    let (mut transfer, _) = export_plan(repository, &selected, without_backing, true, &[])?;
    transfer.roots = roots.iter().map(|root| *root.as_bytes()).collect();
    transfer.encode().map_err(|_| Error::Invalid)
}

fn coupon_roots(
    repository: &Repository,
    roots: &[Handle],
    without_backing: &[Object],
) -> Result<Vec<Handle>, Error> {
    let mut inputs: BTreeMap<[u8; 32], Handle> =
        roots.iter().map(|root| (*root.as_bytes(), *root)).collect();
    for root in roots {
        repository.visit(*root, without_backing, false, |object| {
            if let ObjectView::Tree(tree) = object.view()
                && !without_backing
                    .iter()
                    .any(|excluded| excluded.as_handle().same_encoding(object.as_handle()))
                && repository.has_backing(object)?
            {
                for (index, child) in repository.read_tree(*tree)?.iter().enumerate() {
                    if !tree.is_tag() || index != 0 {
                        inputs.insert(*child.as_bytes(), *child);
                    }
                }
            }
            Ok::<_, Error>(())
        })?;
    }
    let mut selected: BTreeMap<[u8; 32], Handle> =
        roots.iter().map(|root| (*root.as_bytes(), *root)).collect();
    for input in inputs.values() {
        for operation in [1u64, 2, 3, 4, 6] {
            let operation = Blob::literal(&operation.to_le_bytes()).map_err(|_| Error::Invalid)?;
            for coupon in repository.claims(&[operation.into(), *input])? {
                selected.insert(*coupon.as_handle().as_bytes(), coupon.into());
                let fields = repository.read_tree(coupon)?;
                let claim = fixhandle::TreeRef::try_from(fields[1])
                    .map_err(|_| Error::Invalid)?
                    .object_descriptor();
                selected.insert(*claim.as_handle().as_bytes(), claim.into());
            }
        }
    }
    Ok(selected.into_values().collect())
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
    let mut claims = repository.claim_results(collector)?;
    claims.extend(repository.claim_results(fixhandle::Tree::empty().into())?);
    claims.sort_by_key(|(operation, input)| (*operation, *input.as_bytes()));
    claims.dedup_by(|left, right| left.0 == right.0 && left.1.same_encoding(&right.1));
    let results = claims
        .into_iter()
        .map(|(operation, input)| protocol::Task {
            operation,
            collector: *collector.as_bytes(),
            input: *input.as_bytes(),
        })
        .collect();
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
    let mut bundle = Transfer::decode(bytes)
        .map_err(|error| Error::Transfer(alloc::format!("cannot decode transfer: {error}")))?;
    bundle.records.append(&mut bundle.certificates);
    let records: BTreeMap<_, _> = bundle
        .records
        .iter()
        .map(|record| (record.name, record))
        .collect();
    if records.len() != bundle.records.len() {
        return Err(Error::Transfer(String::from(
            "transfer contains duplicate object records",
        )));
    }
    let certificate_bytes = |name: &[u8; 32]| -> Result<Vec<u8>, Error> {
        let certificate_name = display_name(name);
        let blob = fixhandle::Blob::try_from(canonical(name)?).map_err(|error| {
            Error::Transfer(alloc::format!(
                "certificate {} is not a Blob: {error:?}",
                display_name(name)
            ))
        })?;
        let data = if let Some(data) = records.get(name).and_then(|record| record.data.as_ref()) {
            data.clone()
        } else {
            repository.read_blob(blob)?.into_vec()
        };
        blob.verify(&data).map_err(|error| {
            Error::Transfer(alloc::format!(
                "certificate Blob {certificate_name} failed validation: {error:?}"
            ))
        })?;
        Ok(data)
    };
    let mut certificates = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut backed = BTreeSet::new();
    for record in &bundle.records {
        let record_name = display_name(&record.name);
        let object = Object::try_from(canonical(&record.name)?).map_err(|error| {
            Error::Transfer(alloc::format!(
                "record {} is not an Object: {error:?}",
                display_name(&record.name)
            ))
        })?;
        if matches!(object.view(), ObjectView::Tree(tree) if tree.is_tag())
            && (record.data.is_some() || !record.certificates.is_empty())
        {
            let available = if record.certificates.is_empty() && config.policy.unsigned {
                Vec::new()
            } else if record.certificates.is_empty() {
                repository.available_certificates(object.as_handle().as_bytes())?
            } else {
                record
                    .certificates
                    .iter()
                    .map(|reference| {
                        let certificate_name = display_name(&reference.blob);
                        let bytes = certificate_bytes(&reference.blob)?;
                        let certificate = Certificate::decode(&bytes).map_err(|error| {
                            Error::Transfer(alloc::format!(
                                "cannot decode certificate {certificate_name} for Tag {record_name}: {error}"
                            ))
                        })?;
                        let signer = certificate.signer().map_err(|error| {
                            Error::Transfer(alloc::format!(
                                "cannot read signer of certificate {certificate_name}: {error}"
                            ))
                        })?;
                        if certificate.name != record.name || signer != reference.key {
                            return Err(Error::Transfer(alloc::format!(
                                "certificate {certificate_name} does not match Tag {record_name} or advertised signing key {}",
                                reference.key
                            )));
                        }
                        certificate
                            .verify(
                                &record.name,
                                &Policy {
                                    authorized_keys: alloc::vec![reference.key.clone()],
                                    ..Policy::default()
                                },
                            )
                            .map_err(|error| {
                                Error::Transfer(alloc::format!(
                                    "invalid signature on Tag {record_name} by {}: {error}",
                                    reference.key
                                ))
                            })?;
                        Ok(certificate)
                    })
                    .collect::<Result<Vec<_>, Error>>()?
            };
            if !config.policy.unsigned
                && !available
                    .iter()
                    .any(|certificate| certificate.verify(&record.name, &config.policy).is_ok())
            {
                return Err(Error::Untrusted);
            }
            certificates.insert(record.name, available);
        } else if !record.certificates.is_empty() {
            return Err(Error::Transfer(alloc::format!(
                "non-Tag record {record_name} carries certificates"
            )));
        }
        if matches!(object.as_handle().name(), Name::Literal(_)) || !names.insert(record.name) {
            return Err(Error::Transfer(alloc::format!(
                "record {record_name} is literal or duplicates another record"
            )));
        }
        if let Some(bytes) = &record.data {
            backed.insert(record.name);
            match object.view() {
                ObjectView::Blob(blob) => blob.verify(bytes).map_err(|error| {
                    Error::Transfer(alloc::format!(
                        "Blob {record_name} backing failed validation: {error:?}"
                    ))
                })?,
                ObjectView::Tree(tree) => {
                    tree.verify_bytes(bytes).map_err(|error| {
                        Error::Transfer(alloc::format!(
                            "Tree {record_name} backing failed validation: {error:?}"
                        ))
                    })?;
                }
            }
        }
    }
    let admitted = |bytes: &[u8; 32], require_backing: bool| -> Result<(), Error> {
        let handle = canonical(bytes)?;
        if !require_backing || !matches!(handle.view(), HandleView::Object(_)) {
            return Ok(());
        }
        let mut object = *bytes;
        object[31] &= 0xe0;
        let supplied = if require_backing && matches!(handle.view(), HandleView::Object(_)) {
            backed.contains(&object)
        } else {
            names.contains(&object)
        };
        let existing = if require_backing && matches!(handle.view(), HandleView::Object(_)) {
            let value = Object::try_from(handle).map_err(|error| {
                Error::Transfer(alloc::format!("expected Object {handle}: {error:?}"))
            })?;
            repository.has_backing(value)?
        } else {
            repository.lookup(handle.as_bytes()).is_ok()
        };
        if supplied || existing {
            Ok(())
        } else {
            Err(Error::Transfer(alloc::format!(
                "{} {handle} is neither supplied by the transfer nor available in repository {:?}",
                if require_backing && matches!(handle.view(), HandleView::Object(_)) {
                    "backing for Object"
                } else {
                    "descriptor for value"
                },
                repository.path()
            )))
        }
    };
    for root in &bundle.roots {
        admitted(root, true)?;
    }
    for record in &bundle.records {
        if let Some(todo) = &record.todo {
            admitted(todo, true)?;
        }
        for certificate in &record.certificates {
            admitted(&certificate.blob, true)?;
        }
        if matches!(canonical(&record.name)?.view(), HandleView::Object(object)
            if matches!(object.view(), ObjectView::Tree(_)))
            && let Some(bytes) = &record.data
        {
            let is_tag = record.name[31] & 0x40 != 0;
            for (index, child) in bytes.as_chunks::<32>().0.iter().enumerate() {
                admitted(child, !is_tag || index != 0)?;
            }
        }
    }

    for record in &bundle.records {
        let object = Object::try_from(canonical(&record.name)?).map_err(|error| {
            Error::Transfer(alloc::format!(
                "record {} is not an Object: {error:?}",
                display_name(&record.name)
            ))
        })?;
        if let Some(certificate) = certificates
            .get(&record.name)
            .filter(|certificates| !certificates.is_empty())
        {
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
                    .map_err(|error| {
                        Error::Transfer(alloc::format!(
                            "cannot encode certificate for {}: {error}",
                            display_name(&record.name)
                        ))
                    })?
                    .as_deref(),
            )?;
        }
    }
    for record in &bundle.records {
        let object = Object::try_from(canonical(&record.name)?).map_err(|error| {
            Error::Transfer(alloc::format!(
                "record {} is not an Object: {error:?}",
                display_name(&record.name)
            ))
        })?;
        if record.data.is_some()
            && let Some(certificates) = certificates.get(&record.name)
            && !certificates.is_empty()
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
    let view = Handle::parse_ref(bytes).map_err(|error| {
        Error::Transfer(alloc::format!(
            "invalid Fix name {}: {error:?}",
            display_name(bytes)
        ))
    })?;
    if !view.is_canonical() {
        return Err(Error::Transfer(alloc::format!(
            "Fix name {} is not canonical",
            display_name(bytes)
        )));
    }
    // Only canonical machine names enter admission; backing and Tag certificates are checked before publication.
    Ok(unsafe { view.to_owned() })
}

#[cfg(test)]
mod tests;

fn display_name(bytes: &[u8; 32]) -> String {
    use core::fmt::Write;
    let mut name = String::with_capacity(64);
    for byte in bytes {
        write!(&mut name, "{byte:02x}").unwrap();
    }
    name
}
