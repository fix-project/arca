#![no_std]
extern crate alloc;

use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
use embedded_io::{Read, Write};
use serde::{Deserialize, Serialize};

pub type Name = [u8; 32];
pub const VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub operation: u64,
    pub collector: Name,
    pub input: Name,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub name: Name,
    pub data: Option<Vec<u8>>,
    pub todo: Option<Name>,
    pub certificates: Vec<CertificateRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificateRef {
    pub key: String,
    pub blob: Name,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub name: Name,
    pub backing: bool,
    pub todo: Option<Name>,
}

impl Record {
    pub fn offer(&self) -> Offer {
        Offer {
            name: self.name,
            backing: self.data.is_some(),
            todo: self.todo,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transfer {
    pub roots: Vec<Name>,
    pub records: Vec<Record>,
    pub certificates: Vec<Record>,
}
impl Transfer {
    pub fn encode(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, postcard::Error> {
        let (value, trailing) = postcard::take_from_bytes(bytes)?;
        if !trailing.is_empty() {
            return Err(postcard::Error::DeserializeBadEncoding);
        }
        Ok(value)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Information {
    pub parallelism: u32,
    pub memory_bytes: Option<u64>,
    pub link_speed: f64,
    pub data: Vec<Name>,
    pub results: Vec<Task>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Message {
    Run(Task),
    Result {
        task: Task,
        coupon: Name,
    },
    RequestInfo,
    Info(Information),
    RequestTree(Name),
    RequestShallowTree(Name),
    RequestBlob(Name),
    BlobData(Record),
    TreeData(Record),
    LoadBlob(Name),
    LoadTree(Name),
    ShallowTreeData(Record),
    ProposeTransfer {
        task: Task,
        result: Option<Name>,
        objects: Vec<Offer>,
    },
    AcceptTransfer {
        task: Task,
        result: Option<Name>,
        objects: Vec<Name>,
        trusted_keys: Vec<String>,
    },
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    message: Message,
}

#[derive(Debug)]
pub enum Error<E> {
    Transport(fixtransport::Error<E>),
    Invalid,
    Rejected,
}
impl<E> From<fixtransport::Error<E>> for Error<E> {
    fn from(error: fixtransport::Error<E>) -> Self {
        Self::Transport(error)
    }
}

pub fn send<S: Write>(stream: &mut S, message: Message) -> Result<(), Error<S::Error>> {
    let bytes = postcard::to_allocvec(&Envelope {
        version: VERSION,
        message,
    })
    .map_err(|_| Error::Invalid)?;
    fixtransport::send(stream, &bytes)?;
    Ok(())
}

pub fn receive<S: Read>(stream: &mut S, limit: usize) -> Result<Option<Message>, Error<S::Error>> {
    let Some(bytes) = fixtransport::receive(stream, limit)? else {
        return Ok(None);
    };
    let (envelope, trailing): (Envelope, _) =
        postcard::take_from_bytes(&bytes).map_err(|_| Error::Invalid)?;
    if envelope.version != VERSION || !trailing.is_empty() {
        return Err(Error::Invalid);
    }
    Ok(Some(envelope.message))
}

fn roots(task: &Task, result: Option<Name>) -> Vec<Name> {
    match result {
        Some(coupon) => alloc::vec![coupon],
        None => alloc::vec![task.collector, task.input],
    }
}

pub fn send_transfer<S: Read + Write>(
    stream: &mut S,
    limit: usize,
    task: &Task,
    result: Option<Name>,
    transfer: Transfer,
    mut certify: impl FnMut(Name, &[String]) -> Option<(CertificateRef, Record)>,
) -> Result<(), Error<S::Error>> {
    if transfer.roots != roots(task, result) {
        return Err(Error::Invalid);
    }
    let offered: BTreeSet<_> = transfer.records.iter().map(|record| record.name).collect();
    if offered.len() != transfer.records.len() {
        return Err(Error::Invalid);
    }
    send(
        stream,
        Message::ProposeTransfer {
            task: task.clone(),
            result,
            objects: transfer.records.iter().map(Record::offer).collect(),
        },
    )?;
    let Some(Message::AcceptTransfer {
        task: accepted,
        result: accepted_result,
        objects,
        trusted_keys,
    }) = receive(stream, limit)?
    else {
        return Err(Error::Invalid);
    };
    let selected: BTreeSet<_> = objects.iter().copied().collect();
    if accepted != *task
        || accepted_result != result
        || selected.len() != objects.len()
        || !selected.is_subset(&offered)
    {
        return Err(Error::Invalid);
    }
    let certificates: BTreeMap<_, _> = transfer
        .certificates
        .into_iter()
        .map(|record| (record.name, record))
        .collect();
    let mut sent = BTreeSet::new();
    for record in &transfer.records {
        if !selected.contains(&record.name) || !sent.insert(record.name) {
            continue;
        }
        let mut record = record.clone();
        if record.name[31] & 0x40 != 0 {
            let existing = record
                .certificates
                .iter()
                .find(|certificate| trusted_keys.contains(&certificate.key))
                .cloned();
            let (certificate, blob) = if let Some(certificate) = existing {
                let blob = certificates
                    .get(&certificate.blob)
                    .or_else(|| {
                        transfer
                            .records
                            .iter()
                            .find(|record| record.name == certificate.blob)
                    })
                    .ok_or(Error::Invalid)?
                    .clone();
                (certificate, blob)
            } else {
                certify(record.name, &trusted_keys).ok_or(Error::Rejected)?
            };
            if !trusted_keys.contains(&certificate.key) || blob.name != certificate.blob {
                return Err(Error::Invalid);
            }
            if sent.insert(blob.name) {
                send(stream, Message::BlobData(blob))?;
            }
            record.certificates = alloc::vec![certificate];
        }
        let message = if record.name[31] & 0x20 != 0 {
            Message::TreeData(record)
        } else {
            Message::BlobData(record)
        };
        send(stream, message)?;
    }
    send(
        stream,
        match result {
            Some(coupon) => Message::Result {
                task: task.clone(),
                coupon,
            },
            None => Message::Run(task.clone()),
        },
    )
}

pub struct Incoming {
    pub task: Task,
    pub result: Option<Name>,
    pub transfer: Transfer,
}

struct Pending {
    task: Task,
    result: Option<Name>,
    offers: BTreeMap<Name, Offer>,
    selected: BTreeSet<Name>,
    requested: BTreeSet<Name>,
    records: BTreeMap<Name, Record>,
}

pub fn receive_transfer<S: Read + Write>(
    stream: &mut S,
    limit: usize,
    parallelism: u32,
    trusted_keys: &[String],
    missing: impl FnMut(&[Offer]) -> Option<Vec<Name>>,
) -> Result<Option<Incoming>, Error<S::Error>> {
    receive_transfer_with_info(
        stream,
        limit,
        &Information {
            parallelism,
            ..Information::default()
        },
        trusted_keys,
        missing,
    )
}

pub fn receive_transfer_with_info<S: Read + Write>(
    stream: &mut S,
    limit: usize,
    information: &Information,
    trusted_keys: &[String],
    mut missing: impl FnMut(&[Offer]) -> Option<Vec<Name>>,
) -> Result<Option<Incoming>, Error<S::Error>> {
    let mut pending: Option<Pending> = None;
    let mut total = 0usize;
    loop {
        let Some(message) = receive(stream, limit)? else {
            return if pending.is_none() {
                Ok(None)
            } else {
                Err(Error::Invalid)
            };
        };
        match message {
            Message::RequestInfo => send(stream, Message::Info(information.clone()))?,
            Message::ProposeTransfer {
                task,
                result,
                objects,
            } if pending.is_none() => {
                let offered: BTreeSet<_> = objects.iter().map(|offer| offer.name).collect();
                if offered.len() != objects.len() {
                    return Err(Error::Invalid);
                }
                let requested = missing(&objects).ok_or(Error::Rejected)?;
                let selected: BTreeSet<_> = requested.iter().copied().collect();
                if selected.len() != requested.len() || !selected.is_subset(&offered) {
                    return Err(Error::Invalid);
                }
                send(
                    stream,
                    Message::AcceptTransfer {
                        task: task.clone(),
                        result,
                        objects: requested,
                        trusted_keys: if selected.iter().any(|name| name[31] & 0x40 != 0) {
                            trusted_keys.to_vec()
                        } else {
                            Vec::new()
                        },
                    },
                )?;
                pending = Some(Pending {
                    task,
                    result,
                    offers: objects
                        .into_iter()
                        .map(|offer| (offer.name, offer))
                        .collect(),
                    requested: selected.clone(),
                    selected,
                    records: BTreeMap::new(),
                });
            }
            Message::BlobData(record)
            | Message::TreeData(record)
            | Message::ShallowTreeData(record) => {
                let Some(pending) = &mut pending else {
                    return Err(Error::Invalid);
                };
                if pending.records.contains_key(&record.name) {
                    return Err(Error::Invalid);
                }
                if pending.selected.remove(&record.name) {
                    if pending.offers.get(&record.name) != Some(&record.offer()) {
                        return Err(Error::Invalid);
                    }
                } else if record.name[31] & 0x20 != 0
                    || record.todo.is_some()
                    || !record.certificates.is_empty()
                {
                    return Err(Error::Invalid);
                }
                if record.name[31] & 0x40 != 0
                    && (record.certificates.len() != 1
                        || !trusted_keys.contains(&record.certificates[0].key))
                {
                    return Err(Error::Rejected);
                }
                total = total
                    .checked_add(record.data.as_ref().map_or(0, Vec::len))
                    .ok_or(Error::Invalid)?;
                if total > limit {
                    return Err(Error::Invalid);
                }
                pending.records.insert(record.name, record);
            }
            Message::Run(_) | Message::Result { .. } => {
                let (task, result) = match message {
                    Message::Run(task) => (task, None),
                    Message::Result { task, coupon } => (task, Some(coupon)),
                    _ => unreachable!(),
                };
                let Some(mut pending) = pending else {
                    return Err(Error::Invalid);
                };
                if pending.task != task || pending.result != result || !pending.selected.is_empty()
                {
                    return Err(Error::Invalid);
                }
                let certificates: BTreeSet<_> = pending
                    .records
                    .values()
                    .flat_map(|record| {
                        record
                            .certificates
                            .iter()
                            .map(|certificate| certificate.blob)
                    })
                    .collect();
                if pending
                    .records
                    .keys()
                    .any(|name| !pending.requested.contains(name) && !certificates.contains(name))
                {
                    return Err(Error::Invalid);
                }
                for offer in pending.offers.into_values() {
                    pending.records.entry(offer.name).or_insert(Record {
                        name: offer.name,
                        data: None,
                        todo: offer.todo,
                        certificates: Vec::new(),
                    });
                }
                return Ok(Some(Incoming {
                    transfer: Transfer {
                        roots: roots(&task, result),
                        records: pending.records.into_values().collect(),
                        certificates: Vec::new(),
                    },
                    task,
                    result,
                }));
            }
            _ => return Err(Error::Invalid),
        }
    }
}
