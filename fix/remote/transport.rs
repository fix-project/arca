#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use embedded_io::{Read, Write};

#[derive(Debug)]
pub enum Error<E> {
    Io(E),
    Truncated,
    TooLarge { length: u64, limit: usize },
    WriteZero,
}

impl<E: core::fmt::Display> core::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Truncated => f.write_str("connection closed during a message"),
            Self::TooLarge { length, limit } => {
                write!(f, "message has {length} bytes; limit is {limit} bytes")
            }
            Self::WriteZero => f.write_str("connection closed while sending a message"),
        }
    }
}

pub trait AsyncRead: embedded_io::ErrorType {
    fn read(
        &mut self,
        bytes: &mut [u8],
    ) -> impl core::future::Future<Output = Result<usize, Self::Error>>;
}
pub trait AsyncWrite: embedded_io::ErrorType {
    fn write(
        &mut self,
        bytes: &[u8],
    ) -> impl core::future::Future<Output = Result<usize, Self::Error>>;
    fn flush(&mut self) -> impl core::future::Future<Output = Result<(), Self::Error>>;
}

pub struct Blocking<'a, S>(pub &'a mut S);
impl<S: embedded_io::ErrorType> embedded_io::ErrorType for Blocking<'_, S> {
    type Error = S::Error;
}
impl<S: Read> AsyncRead for Blocking<'_, S> {
    async fn read(&mut self, bytes: &mut [u8]) -> Result<usize, Self::Error> {
        self.0.read(bytes)
    }
}
impl<S: Write> AsyncWrite for Blocking<'_, S> {
    async fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        self.0.write(bytes)
    }
    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.0.flush()
    }
}

/// Finish a future whose I/O adapter completes every operation synchronously.
pub fn complete<F: core::future::Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    match future.as_mut().poll(&mut cx) {
        core::task::Poll::Ready(value) => value,
        core::task::Poll::Pending => panic!("blocking I/O returned pending"),
    }
}

pub fn receive<S: Read>(stream: &mut S, limit: usize) -> Result<Option<Vec<u8>>, Error<S::Error>> {
    complete(asynchronous::receive(&mut Blocking(stream), limit))
}
pub fn send<S: Write>(stream: &mut S, bytes: &[u8]) -> Result<(), Error<S::Error>> {
    complete(asynchronous::send(&mut Blocking(stream), bytes))
}

pub mod asynchronous {
    use super::*;
    pub async fn receive<S: AsyncRead>(
        stream: &mut S,
        limit: usize,
    ) -> Result<Option<Vec<u8>>, Error<S::Error>> {
        receive_observed(stream, limit, &mut |_, _| {}).await
    }

    pub async fn receive_observed<S: AsyncRead>(
        stream: &mut S,
        limit: usize,
        progress: &mut impl FnMut(u64, u64),
    ) -> Result<Option<Vec<u8>>, Error<S::Error>> {
        let mut header = [0; 8];
        if stream.read(&mut header[..1]).await.map_err(Error::Io)? == 0 {
            return Ok(None);
        }
        read_exact(stream, &mut header[1..]).await?;
        let announced = u64::from_be_bytes(header);
        let length = usize::try_from(announced).map_err(|_| Error::TooLarge {
            length: announced,
            limit,
        })?;
        if length > limit {
            return Err(Error::TooLarge {
                length: announced,
                limit,
            });
        }
        let mut bytes = alloc::vec![0; length];
        let mut received = 0;
        progress(0, announced);
        while received < length {
            let end = (received + 65536).min(length);
            let read = stream
                .read(&mut bytes[received..end])
                .await
                .map_err(Error::Io)?;
            if read == 0 {
                return Err(Error::Truncated);
            }
            received += read;
            progress(received as u64, announced);
        }
        Ok(Some(bytes))
    }

    pub async fn send<S: AsyncWrite>(stream: &mut S, bytes: &[u8]) -> Result<(), Error<S::Error>> {
        send_observed(stream, bytes, &mut |_, _| {}).await
    }

    pub async fn send_observed<S: AsyncWrite>(
        stream: &mut S,
        bytes: &[u8],
        progress: &mut impl FnMut(u64, u64),
    ) -> Result<(), Error<S::Error>> {
        write_all(stream, &(bytes.len() as u64).to_be_bytes()).await?;
        let mut written = 0;
        progress(0, bytes.len() as u64);
        while written < bytes.len() {
            let end = (written + 65536).min(bytes.len());
            let count = stream
                .write(&bytes[written..end])
                .await
                .map_err(Error::Io)?;
            if count == 0 {
                return Err(Error::WriteZero);
            }
            written += count;
            progress(written as u64, bytes.len() as u64);
        }
        stream.flush().await.map_err(Error::Io)
    }

    async fn read_exact<S: AsyncRead>(
        stream: &mut S,
        mut bytes: &mut [u8],
    ) -> Result<(), Error<S::Error>> {
        while !bytes.is_empty() {
            let length = stream.read(bytes).await.map_err(Error::Io)?;
            if length == 0 {
                return Err(Error::Truncated);
            }
            bytes = &mut bytes[length..];
        }
        Ok(())
    }

    async fn write_all<S: AsyncWrite>(
        stream: &mut S,
        mut bytes: &[u8],
    ) -> Result<(), Error<S::Error>> {
        while !bytes.is_empty() {
            let length = stream.write(bytes).await.map_err(Error::Io)?;
            if length == 0 {
                return Err(Error::WriteZero);
            }
            bytes = &bytes[length..];
        }
        Ok(())
    }
}
