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
pub const VERSION: u32 = 9;
pub const OBJECT_CHUNK_SIZE: usize = 1 << 20;
pub const MAX_MESSAGE_SIZE: usize = 512 << 20;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub operation: u64,
    pub collector: Name,
    pub input: Name,
}
impl Task {
    pub fn stage_programs(collector: Name, scheduler: Name) -> Self {
        Self {
            operation: 0,
            collector,
            input: scheduler,
        }
    }

    pub fn is_stage_programs(&self) -> bool {
        self.operation == 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub name: Name,
    #[serde(with = "optional_bytes")]
    pub data: Option<Vec<u8>>,
    pub todo: Option<Name>,
    pub certificates: Vec<CertificateRef>,
}

mod optional_bytes {
    use super::*;
    use serde::{Deserializer, Serializer};

    struct Bytes<T>(T);
    impl<T: AsRef<[u8]>> Serialize for Bytes<T> {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_bytes(self.0.as_ref())
        }
    }
    pub fn serialize_bytes<S: Serializer>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(value)
    }
    pub fn deserialize_bytes<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<u8>, D::Error> {
        <&[u8]>::deserialize(deserializer).map(<[u8]>::to_vec)
    }
    pub fn serialize<S: Serializer>(
        value: &Option<Vec<u8>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.as_deref().map(Bytes).serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Vec<u8>>, D::Error> {
        Option::<&[u8]>::deserialize(deserializer).map(|value| value.map(<[u8]>::to_vec))
    }
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

pub fn prepare_transfer(
    mut transfer: Transfer,
    trusted_keys: &[String],
    mut certify: impl FnMut(Name, &[String]) -> Result<(CertificateRef, Record), String>,
) -> Result<Transfer, String> {
    for record in &mut transfer.records {
        if record.name[31] & 0x40 == 0
            || record
                .certificates
                .iter()
                .any(|certificate| trusted_keys.contains(&certificate.key))
        {
            continue;
        }
        let (certificate, blob) = certify(record.name, trusted_keys)?;
        if !trusted_keys.contains(&certificate.key) || certificate.blob != blob.name {
            return Err(String::from(
                "certificate does not match a trusted key or its blob",
            ));
        }
        record.certificates.push(certificate);
        transfer.certificates.push(blob);
    }
    Ok(transfer)
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
    RequestProgress,
    EvaluationProgress(
        #[serde(
            serialize_with = "optional_bytes::serialize_bytes",
            deserialize_with = "optional_bytes::deserialize_bytes"
        )]
        Vec<u8>,
    ),
    RequestLabels,
    Labels(Vec<(String, Name)>),
    RequestObject {
        name: Name,
        trusted_keys: Vec<String>,
        backing: bool,
    },
    ProposeObject {
        transfer: Transfer,
        backing: Vec<Name>,
    },
    AcceptObject(Vec<Name>),
    ObjectChunk {
        name: Name,
        offset: u64,
        #[serde(
            serialize_with = "optional_bytes::serialize_bytes",
            deserialize_with = "optional_bytes::deserialize_bytes"
        )]
        data: Vec<u8>,
    },
    RequestTrustedKeys,
    TrustedKeys(Vec<String>),
    SubmitObject(Transfer),
    ObjectReceived(Name),
    UpdateLabel {
        label: String,
        expected: Option<Name>,
        value: Name,
    },
    LabelUpdated,
    Rejected,
    Failure(String),
    Run(Task),
    Result {
        task: Task,
        coupon: Name,
    },
    RequestInfo {
        inventory: bool,
    },
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

pub fn encode(message: Message) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_allocvec(&Envelope {
        version: VERSION,
        message,
    })
}

#[derive(Debug)]
pub enum Error<E> {
    Transport(fixtransport::Error<E>),
    Invalid,
    Decode {
        length: usize,
        error: postcard::Error,
    },
    Version {
        expected: u32,
        received: u32,
    },
    Rejected,
    Remote(String),
}
impl<E: core::fmt::Display> core::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Transport(error) => error.fmt(f),
            Self::Invalid => f.write_str("invalid remote message"),
            Self::Decode { length, error } => write!(
                f,
                "could not decode protocol {VERSION} message ({length} bytes): {error}"
            ),
            Self::Version { expected, received } => {
                write!(f, "protocol version {received}; expected {expected}")
            }
            Self::Rejected => f.write_str("remote rejected the request"),
            Self::Remote(message) => f.write_str(message),
        }
    }
}
impl<E> From<fixtransport::Error<E>> for Error<E> {
    fn from(error: fixtransport::Error<E>) -> Self {
        Self::Transport(error)
    }
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

pub fn send<S: Write>(stream: &mut S, message: Message) -> Result<(), Error<S::Error>> {
    fixtransport::complete(asynchronous::send(
        &mut fixtransport::Blocking(stream),
        message,
    ))
}
pub fn receive_observed<S: Read>(
    stream: &mut S,
    limit: usize,
    mut progress: impl FnMut(u64, u64),
) -> Result<Option<Message>, Error<S::Error>> {
    fixtransport::complete(asynchronous::receive_observed(
        &mut fixtransport::Blocking(stream),
        limit,
        &mut progress,
    ))
}
pub fn receive<S: Read>(stream: &mut S, limit: usize) -> Result<Option<Message>, Error<S::Error>> {
    fixtransport::complete(asynchronous::receive(
        &mut fixtransport::Blocking(stream),
        limit,
    ))
}
pub fn send_transfer<S: Read + Write>(
    stream: &mut S,
    limit: usize,
    task: &Task,
    result: Option<Name>,
    transfer: Transfer,
    certify: impl FnMut(Name, &[String]) -> Option<(CertificateRef, Record)>,
) -> Result<(), Error<S::Error>> {
    fixtransport::complete(asynchronous::send_transfer(
        &mut fixtransport::Blocking(stream),
        limit,
        task,
        result,
        transfer,
        certify,
    ))
}
pub fn receive_transfer<S: Read + Write>(
    stream: &mut S,
    limit: usize,
    parallelism: u32,
    trusted_keys: &[String],
    missing: impl FnMut(&[Offer]) -> Option<Vec<Name>>,
) -> Result<Option<Incoming>, Error<S::Error>> {
    fixtransport::complete(asynchronous::receive_transfer(
        &mut fixtransport::Blocking(stream),
        limit,
        parallelism,
        trusted_keys,
        missing,
    ))
}
pub fn receive_transfer_with_info<S: Read + Write>(
    stream: &mut S,
    limit: usize,
    information: &Information,
    trusted_keys: &[String],
    missing: impl FnMut(&[Offer]) -> Option<Vec<Name>>,
) -> Result<Option<Incoming>, Error<S::Error>> {
    fixtransport::complete(asynchronous::receive_transfer_with_info(
        &mut fixtransport::Blocking(stream),
        limit,
        information,
        trusted_keys,
        missing,
    ))
}

pub mod asynchronous {
    use super::*;
    use fixtransport::{AsyncRead, AsyncWrite};
    pub async fn send<S: AsyncWrite>(
        stream: &mut S,
        message: Message,
    ) -> Result<(), Error<S::Error>> {
        let bytes = postcard::to_allocvec(&Envelope {
            version: VERSION,
            message,
        })
        .map_err(|_| Error::Invalid)?;
        fixtransport::asynchronous::send(stream, &bytes).await?;
        Ok(())
    }

    pub async fn receive<S: AsyncRead>(
        stream: &mut S,
        limit: usize,
    ) -> Result<Option<Message>, Error<S::Error>> {
        receive_observed(stream, limit, &mut |_, _| {}).await
    }

    pub async fn receive_observed<S: AsyncRead>(
        stream: &mut S,
        limit: usize,
        progress: &mut impl FnMut(u64, u64),
    ) -> Result<Option<Message>, Error<S::Error>> {
        let Some(bytes) =
            fixtransport::asynchronous::receive_observed(stream, limit, progress).await?
        else {
            return Ok(None);
        };
        let decode = |error| Error::Decode {
            length: bytes.len(),
            error,
        };
        let (version, body): (u32, _) = postcard::take_from_bytes(&bytes).map_err(decode)?;
        if version != VERSION {
            return Err(Error::Version {
                expected: VERSION,
                received: version,
            });
        }
        let (message, trailing): (Message, _) = postcard::take_from_bytes(body).map_err(decode)?;
        if !trailing.is_empty() {
            return Err(Error::Invalid);
        }
        Ok(Some(message))
    }

    fn roots(task: &Task, result: Option<Name>) -> Vec<Name> {
        match result {
            Some(coupon) => alloc::vec![coupon],
            None => alloc::vec![task.collector, task.input],
        }
    }

    pub async fn send_transfer<S: AsyncRead + AsyncWrite>(
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
        )
        .await?;
        let Some(Message::AcceptTransfer {
            task: accepted,
            result: accepted_result,
            objects,
            trusted_keys,
        }) = receive(stream, limit).await?
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
                    send(stream, Message::BlobData(blob)).await?;
                }
                record.certificates = alloc::vec![certificate];
            }
            let message = if record.name[31] & 0x20 != 0 {
                Message::TreeData(record)
            } else {
                Message::BlobData(record)
            };
            send(stream, message).await?;
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
        .await
    }

    pub async fn receive_transfer<S: AsyncRead + AsyncWrite>(
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
        .await
    }

    pub async fn receive_transfer_with_info<S: AsyncRead + AsyncWrite>(
        stream: &mut S,
        limit: usize,
        information: &Information,
        trusted_keys: &[String],
        missing: impl FnMut(&[Offer]) -> Option<Vec<Name>>,
    ) -> Result<Option<Incoming>, Error<S::Error>> {
        receive_transfer_with_info_from(stream, limit, information, trusted_keys, missing, None)
            .await
    }

    pub async fn receive_transfer_with_info_from<S: AsyncRead + AsyncWrite>(
        stream: &mut S,
        limit: usize,
        information: &Information,
        trusted_keys: &[String],
        missing: impl FnMut(&[Offer]) -> Option<Vec<Name>>,
        first: Option<Message>,
    ) -> Result<Option<Incoming>, Error<S::Error>> {
        receive_transfer_observed(
            stream,
            limit,
            information,
            trusted_keys,
            missing,
            first,
            |_| {},
        )
        .await
    }

    pub async fn receive_transfer_observed<S: AsyncRead + AsyncWrite>(
        stream: &mut S,
        limit: usize,
        information: &Information,
        trusted_keys: &[String],
        mut missing: impl FnMut(&[Offer]) -> Option<Vec<Name>>,
        first: Option<Message>,
        mut progress: impl FnMut(&Message),
    ) -> Result<Option<Incoming>, Error<S::Error>> {
        let mut pending: Option<Pending> = None;
        let mut total = 0usize;
        let mut first = first;
        loop {
            let Some(message) = (if first.is_some() {
                first.take()
            } else {
                receive(stream, limit).await?
            }) else {
                return if pending.is_none() {
                    Ok(None)
                } else {
                    Err(Error::Invalid)
                };
            };
            progress(&message);
            match message {
                Message::EvaluationProgress(_) => (),
                Message::Failure(message) => return Err(Error::Remote(message)),
                Message::RequestInfo { .. } => {
                    send(stream, Message::Info(information.clone())).await?
                }
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
                    )
                    .await?;
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
                    if pending.task != task
                        || pending.result != result
                        || !pending.selected.is_empty()
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
                    if pending.records.keys().any(|name| {
                        !pending.requested.contains(name) && !certificates.contains(name)
                    }) {
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
}
