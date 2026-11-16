mod cluster;
pub mod server;
use crate::io::Io;
use crate::{
    abi::{effect, request},
    shell::{Value, child, fields},
};
use alloc::{string::String, vec::Vec};
pub use cluster::{Cluster, Failure, PendingEvaluation};
use fixwasm::{Blob, Error, Tree, Value as AsValue, tree};

pub use fixprotocol as protocol;
pub use fixtransport as framing;

pub struct Remote {
    pub name: String,
    pub address: String,
}

pub fn configured() -> Result<Vec<Remote>, Error> {
    fields(&effect(b"remote_config", &[])?)?
        .iter()
        .map(|remote| {
            let name =
                String::from_utf8(bytes(&child(remote, 0)?)?).map_err(|_| Error::WrongType)?;
            let address =
                String::from_utf8(bytes(&child(remote, 1)?)?).map_err(|_| Error::WrongType)?;
            Ok(Remote { name, address })
        })
        .collect()
}

pub fn export(values: &[Value], without_backing: &[Value]) -> Result<Option<Vec<u8>>, Error> {
    let (status, value) = request(tree![
        Blob::create(b"storage_export"),
        Tree::from_iter(values),
        Tree::from_iter(without_backing)
    ])?;
    if status == 0 {
        Ok(Some(bytes(&value)?))
    } else {
        Ok(None)
    }
}

pub fn import(bytes: &[u8]) -> Result<Option<Vec<Value>>, Error> {
    let (status, value) = request(tree![Blob::create(b"storage_import"), Blob::create(bytes)])?;
    if status == 0 {
        Ok(Some(fields(&value)?))
    } else {
        Ok(None)
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

pub fn trusted_keys() -> Result<Vec<String>, Error> {
    let value = effect(b"remote_trusted_keys", &[])?;
    postcard::from_bytes(&bytes(&value)?).map_err(|_| Error::WrongType)
}

pub fn certify(
    name: protocol::Name,
    keys: &[String],
) -> Result<Option<(protocol::CertificateRef, protocol::Record)>, Error> {
    let encoded = postcard::to_allocvec(&(name, keys)).map_err(|_| Error::WrongType)?;
    let (status, value) = request(tree![
        Blob::create(b"storage_certify"),
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

/// An asynchronous TCP connection owned by one task at a time.
pub struct Connection<'a>(u64, &'a Io);
impl embedded_io::ErrorType for Connection<'_> {
    type Error = IoError;
}
impl fixtransport::AsyncRead for Connection<'_> {
    async fn read(&mut self, bytes: &mut [u8]) -> Result<usize, Self::Error> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let (status, value) = self
            .1
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
impl fixtransport::AsyncWrite for Connection<'_> {
    async fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        let bytes = &bytes[..bytes.len().min(65536)];
        let (status, value) = self
            .1
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
impl Drop for Connection<'_> {
    fn drop(&mut self) {
        let _ = crate::abi::request(tree![Blob::create(b"stream_close"), self.0]);
    }
}

impl<'a> Connection<'a> {
    pub async fn connect_tcp(io: &'a Io, remote: &Remote) -> Result<Option<Self>, Error> {
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
            Some(Self(stream.read_u64()?, io))
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
