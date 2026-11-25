mod cluster;
mod network;
pub mod server;
use crate::io::Io;
use crate::{
    abi::{effect, request},
    shell::{Value, child, fields},
};
use alloc::{format, string::String, vec::Vec};
pub use cluster::{Cluster, Failure, PendingTransfer};
use fixwasm::{Blob, Error, Value as AsValue, tree};
pub use network::Worker;

pub use fixprotocol as protocol;
pub use fixtransport as framing;

pub struct Remote {
    pub name: String,
    pub address: String,
    pub scheduler: Option<String>,
    pub collector: Option<String>,
}

impl Remote {
    pub fn is_ssh(&self) -> bool {
        self.address.starts_with("ssh://")
            || (!self.address.contains("://") && self.address.contains(':'))
    }
}

pub fn configured() -> Result<Vec<Remote>, Error> {
    fields(&effect(b"remote_config", &[])?)?
        .iter()
        .map(|remote| {
            let name =
                String::from_utf8(bytes(&child(remote, 0)?)?).map_err(|_| Error::WrongType)?;
            let address =
                String::from_utf8(bytes(&child(remote, 1)?)?).map_err(|_| Error::WrongType)?;
            let scheduler =
                String::from_utf8(bytes(&child(remote, 2)?)?).map_err(|_| Error::WrongType)?;
            let collector =
                String::from_utf8(bytes(&child(remote, 3)?)?).map_err(|_| Error::WrongType)?;
            Ok(Remote {
                name,
                address,
                scheduler: (!scheduler.is_empty()).then_some(scheduler),
                collector: (!collector.is_empty()).then_some(collector),
            })
        })
        .collect()
}

pub(crate) fn report_status(
    remote: &str,
    status: &str,
    information: Option<&protocol::Information>,
    error: &str,
) {
    report_node_status(remote, status, information, error, None);
}

pub(crate) fn report_node_status(
    remote: &str,
    status: &str,
    information: Option<&protocol::Information>,
    error: &str,
    node: Option<protocol::topology::NodeId>,
) {
    let _ = (|| -> Result<(), Error> {
        let memory = match information.and_then(|info| info.memory_bytes) {
            Some(bytes) => crate::shell::capture(bytes)?,
            None => crate::shell::capture(Blob::create(b""))?,
        };
        effect(
            b"remote_status",
            &[
                crate::shell::capture(Blob::create(remote))?,
                crate::shell::capture(Blob::create(status))?,
                crate::shell::capture(information.map_or(0, |info| info.parallelism as u64))?,
                memory,
                crate::shell::capture(Blob::create(error))?,
                crate::shell::capture(Blob::create(
                    node.as_ref().map_or(&[][..], |node| &node[..]),
                ))?,
            ],
        )?;
        Ok(())
    })();
}

pub(crate) struct ProgressObserver(Value);

impl ProgressObserver {
    pub(crate) fn new() -> Result<Self, Error> {
        Ok(Self(effect(b"progress_subscribe", &[])?))
    }

    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, Error> {
        bytes(&effect(
            b"progress_snapshot",
            core::slice::from_ref(&self.0),
        )?)
    }
}

impl Drop for ProgressObserver {
    fn drop(&mut self) {
        let _ = effect(b"progress_unsubscribe", core::slice::from_ref(&self.0));
    }
}

pub(crate) fn report_progress(remote: &str, snapshot: &[u8], used: bool) -> Result<(), Error> {
    effect(
        b"remote_progress",
        &[
            crate::shell::capture(Blob::create(remote))?,
            crate::shell::capture(Blob::create(snapshot))?,
            crate::shell::capture(u64::from(used))?,
        ],
    )?;
    Ok(())
}

pub fn labels() -> Result<Vec<(String, protocol::Name)>, Error> {
    let value = effect(b"remote_labels", &[])?;
    postcard::from_bytes(&bytes(&value)?).map_err(|_| Error::WrongType)
}

fn request_failure(status: u64, value: &Value) -> String {
    if status == 5
        && let Ok(message) =
            bytes(value).and_then(|bytes| String::from_utf8(bytes).map_err(|_| Error::WrongType))
        && !message.is_empty()
        && !message.contains('\0')
    {
        return message;
    }
    format!("scheduler operation failed with status {status}")
}

pub fn propose_object(name: protocol::Name, backing: bool) -> Result<protocol::Proposal, String> {
    object_proposal(name, backing, false)
}

fn object_proposal(
    name: protocol::Name,
    backing: bool,
    full: bool,
) -> Result<protocol::Proposal, String> {
    let (status, value) = request(tree![
        Blob::create(b"remote_export"),
        Blob::create(name),
        backing as u64,
        full as u64
    ])
    .map_err(|error| format!("{error:?}"))?;
    decode_proposal(status, value)
}

fn decode_proposal(status: u64, value: Value) -> Result<protocol::Proposal, String> {
    if status != 0 {
        return Err(request_failure(status, &value));
    }
    let encoded = bytes(&value).map_err(|error| format!("{error:?}"))?;
    postcard::from_bytes(&encoded).map_err(|error| format!("invalid export proposal: {error}"))
}

/// Propose the minimum repositories of the supplied roots without reading Blob backing.
pub fn propose(values: &[Value]) -> Result<protocol::Proposal, String> {
    let mut proposals = Vec::new();
    for value in values {
        let name = value
            .raw_handle_bytes()
            .map_err(|error| format!("{error:?}"))?;
        proposals.push((name, object_proposal(name, true, false)?));
    }
    Ok(merge_proposals(proposals))
}

pub(crate) async fn propose_task(
    collector: &Value,
    input: &Value,
    backing: bool,
    full: bool,
    inventory: Option<&Value>,
) -> Result<protocol::Proposal, String> {
    let empty = crate::shell::capture(Blob::create(b"")).map_err(|error| format!("{error:?}"))?;
    let inventory = inventory.unwrap_or(&empty);
    let mut proposals = Vec::new();
    for (value, backing, full) in [(collector, true, false), (input, backing, full)] {
        let name = value
            .raw_handle_bytes()
            .map_err(|error| format!("{error:?}"))?;
        let (status, value) = crate::io::Io
            .request(tree![
                Blob::create(b"remote_export"),
                Blob::create(name),
                backing as u64,
                full as u64,
                inventory,
            ])
            .await
            .map_err(|error| format!("{error:?}"))?;
        proposals.push((name, decode_proposal(status, value)?));
    }
    Ok(merge_proposals(proposals))
}

fn merge_proposals(proposals: Vec<(protocol::Name, protocol::Proposal)>) -> protocol::Proposal {
    let mut roots = Vec::new();
    let mut records = alloc::collections::BTreeMap::new();
    let mut order = Vec::new();
    let mut certificates = alloc::collections::BTreeMap::new();
    let mut backing = alloc::collections::BTreeSet::new();
    let mut refs = alloc::collections::BTreeSet::new();
    let mut coupons = alloc::collections::BTreeSet::new();
    for (name, proposal) in proposals {
        roots.push(name);
        let transfer = proposal.transfer;
        for record in transfer.records {
            order.push(record.name);
            records.insert(record.name, record);
        }
        certificates.extend(
            transfer
                .certificates
                .into_iter()
                .map(|record| (record.name, record)),
        );
        backing.extend(proposal.backing);
        refs.extend(proposal.refs);
        coupons.extend(proposal.coupons);
    }
    let certificates = certificates
        .into_iter()
        .filter(|(name, _)| !records.contains_key(name))
        .map(|(_, record)| record)
        .collect();
    let mut records: Vec<_> = order
        .into_iter()
        .rev()
        .filter_map(|name| records.remove(&name))
        .collect();
    records.reverse();
    protocol::Proposal {
        transfer: protocol::Transfer {
            roots,
            certificates,
            records,
        },
        backing: backing.into_iter().collect(),
        refs: refs.into_iter().collect(),
        coupons: coupons.into_iter().collect(),
    }
}

pub fn read_backing(name: protocol::Name, offset: u64, length: usize) -> Result<Vec<u8>, String> {
    let (status, value) = request(tree![
        Blob::create(b"remote_read_backing"),
        Blob::create(name),
        offset,
        length as u64
    ])
    .map_err(|error| format!("{error:?}"))?;
    if status != 0 {
        return Err(request_failure(status, &value));
    }
    bytes(&value).map_err(|error| format!("{error:?}"))
}

pub fn update_label(
    label: &str,
    expected: Option<protocol::Name>,
    value: protocol::Name,
) -> Result<(), String> {
    let encoded = postcard::to_allocvec(&(label, expected, value))
        .map_err(|error| format!("could not encode label update: {error}"))?;
    let (status, response) = request(tree![
        Blob::create(b"remote_label_update"),
        Blob::create(&encoded)
    ])
    .map_err(|error| format!("{error:?}"))?;
    if status == 0 {
        Ok(())
    } else {
        Err(request_failure(status, &response))
    }
}

pub(super) fn transfer_limit() -> Result<usize, Error> {
    let available = crate::kernel::worker::available_memory()?;
    Ok(usize::try_from(available).unwrap_or(usize::MAX))
}

pub(super) fn import_transfer(mut transfer: protocol::Transfer) -> Result<Vec<Value>, String> {
    let available = transfer_limit().map_err(|error| format!("{error:?}"))?;
    let size = transfer.encoded_len().map_err(|error| format!("{error}"))?;
    if size > available / 4 {
        for record in &mut transfer.records {
            if record.name[31] & 0xe0 != 0x80 || !record.certificates.is_empty() {
                continue;
            }
            if let Some(data) = record.data.take() {
                import_buffered_transfer(protocol::Transfer {
                    roots: alloc::vec![record.name],
                    records: alloc::vec![protocol::Record {
                        name: record.name,
                        data: Some(data),
                        todo: None,
                        certificates: Vec::new(),
                    }],
                    certificates: Vec::new(),
                })?;
            }
        }
    }
    import_buffered_transfer(transfer)
}

fn import_buffered_transfer(transfer: protocol::Transfer) -> Result<Vec<Value>, String> {
    let size = transfer.encoded_len().map_err(|error| format!("{error}"))?;
    let available = transfer_limit().map_err(|error| format!("{error:?}"))?;
    if size > available {
        return Err(format!(
            "insufficient free memory to encode the transfer ({size} required, {available} available)"
        ));
    }
    let bytes = transfer.encode().map_err(|error| format!("{error}"))?;
    drop(transfer);
    let available = transfer_limit().map_err(|error| format!("{error:?}"))?;
    // Import can hold a Blob backing, a host copy, and decoded records together.
    let required = bytes.len().saturating_mul(3);
    if required > available {
        return Err(format!(
            "insufficient free memory to import the transfer ({required} required, {available} available)"
        ));
    }
    import(&bytes)
}

pub fn import(transfer_bytes: &[u8]) -> Result<Vec<Value>, String> {
    let (status, value) = request(tree![
        Blob::create(b"storage_import"),
        Blob::create(transfer_bytes)
    ])
    .map_err(|error| format!("{error:?}"))?;
    if status == 0 {
        fields(&value).map_err(|error| format!("{error:?}"))
    } else {
        Err(request_failure(status, &value))
    }
}

/// Advertise available objects, coupons, and this VM's capacity.
pub fn information(memory_bytes: u64) -> Result<protocol::Information, Error> {
    let value = effect(
        b"remote_information",
        &[
            crate::collector::callable()?,
            crate::shell::capture(memory_bytes)?,
        ],
    )?;
    postcard::from_bytes(&bytes(&value)?).map_err(|_| Error::WrongType)
}

pub fn file_information(address: &str) -> Result<protocol::Information, String> {
    let (status, value) = request(tree![
        Blob::create(b"remote_file_information"),
        Blob::create(address),
        crate::collector::callable().map_err(|error| format!("{error:?}"))?,
    ])
    .map_err(|error| format!("{error:?}"))?;
    if status != 0 {
        return Err(request_failure(status, &value));
    }
    postcard::from_bytes(&bytes(&value).map_err(|error| format!("{error:?}"))?)
        .map_err(|error| format!("invalid file remote inventory: {error}"))
}

pub fn trusted_keys() -> Result<Vec<String>, Error> {
    let value = effect(b"remote_trusted_keys", &[])?;
    postcard::from_bytes(&bytes(&value)?).map_err(|_| Error::WrongType)
}

pub fn trust_collector(collector: &Value) -> Result<(), String> {
    let (status, value) = request(tree![Blob::create(b"remote_trust_collector"), collector])
        .map_err(|error| format!("{error:?}"))?;
    if status == 0 {
        Ok(())
    } else {
        Err(request_failure(status, &value))
    }
}

pub fn certify(
    name: protocol::Name,
    keys: &[String],
) -> Result<(protocol::CertificateRef, protocol::Record), String> {
    let encoded = postcard::to_allocvec(&(name, keys))
        .map_err(|error| format!("could not encode certificate request: {error}"))?;
    let (status, value) = request(tree![
        Blob::create(b"storage_certify"),
        Blob::create(encoded)
    ])
    .map_err(|error| format!("{error:?}"))?;
    if status == 0 {
        postcard::from_bytes(&bytes(&value).map_err(|error| format!("{error:?}"))?)
            .map_err(|error| format!("invalid certificate response: {error}"))
    } else {
        Err(request_failure(status, &value))
    }
}

pub fn missing(offers: &[protocol::Offer]) -> Result<Option<Vec<protocol::Name>>, Error> {
    let encoded = postcard::to_allocvec(offers).map_err(|_| Error::WrongType)?;
    let (status, value) = request(tree![
        Blob::create(b"storage_missing"),
        Blob::create(encoded)
    ])?;
    if status == 0 {
        Ok(Some(
            postcard::from_bytes(&bytes(&value)?).map_err(|_| Error::WrongType)?,
        ))
    } else {
        Ok(None)
    }
}

pub fn bytes(value: &Value) -> Result<Vec<u8>, Error> {
    let mut bytes = alloc::vec![0; usize::try_from(value.len()?)?];
    value.read_into(0, &mut bytes)?;
    Ok(bytes)
}

/// An asynchronous host byte stream.
pub struct RawStream(u64);
impl embedded_io::ErrorType for &RawStream {
    type Error = IoError;
}
impl fixtransport::AsyncRead for &RawStream {
    async fn read(&mut self, bytes: &mut [u8]) -> Result<usize, Self::Error> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let (status, value) = Io
            .request(tree![
                Blob::create(b"stream_receive"),
                self.0,
                bytes.len().min(65536) as u64
            ])
            .await
            .map_err(|_| IoError)?;
        if status != 0 {
            return Err(IoError);
        }
        let length = usize::try_from(value.len().map_err(|_| IoError)?).map_err(|_| IoError)?;
        if length > bytes.len() {
            return Err(IoError);
        }
        value
            .read_into(0, &mut bytes[..length])
            .map_err(|_| IoError)?;
        Ok(length)
    }
}
impl fixtransport::AsyncWrite for &RawStream {
    async fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        let bytes = &bytes[..bytes.len().min(65536)];
        let (status, value) = Io
            .request(tree![
                Blob::create(b"stream_send"),
                self.0,
                Blob::create(bytes)
            ])
            .await
            .map_err(|_| IoError)?;
        if status != 0 {
            return Err(IoError);
        }
        usize::try_from(value.read_u64().map_err(|_| IoError)?).map_err(|_| IoError)
    }
    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl RawStream {
    fn close(&self) {
        let _ = crate::abi::request(tree![Blob::create(b"stream_close"), self.0]);
    }
}
impl Drop for RawStream {
    fn drop(&mut self) {
        let _ = crate::abi::request(tree![Blob::create(b"stream_close"), self.0]);
    }
}

pub enum Connection {
    Raw(alloc::rc::Rc<RawStream>),
    Request(network::Request),
}
impl Connection {
    fn raw(id: u64) -> Self {
        Self::Raw(alloc::rc::Rc::new(RawStream(id)))
    }

    fn finish_request(&mut self) {
        if let Self::Request(request) = self {
            request.finish();
        }
    }
}
impl embedded_io::ErrorType for Connection {
    type Error = IoError;
}
impl protocol::asynchronous::ReadMessage for Connection {
    async fn read_message(
        &mut self,
        limit: usize,
    ) -> Result<Option<protocol::Message>, protocol::Error<IoError>> {
        match self {
            Self::Raw(stream) => {
                protocol::asynchronous::receive_with_payload_limit(
                    &mut stream.as_ref(),
                    limit,
                    || transfer_limit().unwrap_or(0),
                )
                .await
            }
            Self::Request(request) => request.read().await,
        }
    }
}
impl protocol::asynchronous::WriteMessage for Connection {
    async fn write_message(
        &mut self,
        message: protocol::Message,
    ) -> Result<(), protocol::Error<IoError>> {
        match self {
            Self::Raw(stream) => protocol::asynchronous::send(&mut stream.as_ref(), message).await,
            Self::Request(request) => request
                .write(message)
                .map_err(|error| protocol::Error::Transport(fixtransport::Error::Io(error))),
        }
    }
}

impl Connection {
    pub async fn connect(io: &Io, remote: &Remote) -> Result<Option<Self>, Error> {
        let ssh = if let Some(address) = remote.address.strip_prefix("ssh://") {
            address
                .split_once('/')
                .map(|(host, _)| (host, &address[host.len()..]))
        } else if remote.address.contains("://") {
            None
        } else {
            remote.address.split_once(':')
        };
        if let Some((host, repository)) = ssh {
            if host.is_empty() || repository.is_empty() {
                return Ok(None);
            }
            let (status, stream) = io
                .request(tree![
                    Blob::create(b"ssh_connect"),
                    Blob::create(host),
                    Blob::create(repository),
                    Blob::create(remote.scheduler.as_deref().unwrap_or("")),
                    Blob::create(remote.collector.as_deref().unwrap_or(""))
                ])
                .await?;
            return if status == 0 {
                Ok(Some(Self::raw(stream.read_u64()?)))
            } else {
                Ok(None)
            };
        }
        Self::connect_tcp(io, remote).await
    }

    pub async fn connect_tcp(io: &Io, remote: &Remote) -> Result<Option<Self>, Error> {
        let Some(address) = remote.address.strip_prefix("tcp://") else {
            return Ok(None);
        };
        let Some((host, port)) = address.rsplit_once(':') else {
            return Ok(None);
        };
        let Ok(port) = port.parse::<u16>() else {
            return Ok(None);
        };
        let (status, stream) = io
            .request(tree![
                Blob::create(b"tcp_connect"),
                Blob::create(host.trim_matches(['[', ']'])),
                u64::from(port)
            ])
            .await?;
        Ok(if status == 0 {
            Some(Self::raw(stream.read_u64()?))
        } else {
            None
        })
    }
}

#[derive(Debug)]
pub struct IoError;
impl core::fmt::Display for IoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Fix stream failed")
    }
}
impl core::error::Error for IoError {}
impl embedded_io::Error for IoError {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

/// Read one offered object's backing after the receiver has selected it.
pub fn read_object(name: protocol::Name) -> Result<Vec<u8>, String> {
    let length = fixhandle::Handle::parse_ref(&name)
        .map_err(|error| format!("invalid backing name: {error:?}"))?
        .byte_len();
    let length =
        usize::try_from(length).map_err(|_| String::from("backing exceeds address space"))?;
    read_backing(name, 0, length)
}

mod local;
