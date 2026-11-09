use crate::{Any, Blob, Capture, Error, Handle, Object, Tree, Value, tree};

fn request(value: impl Value) -> Result<(u64, Handle<Any, Capture>), Error> {
    let reply = Handle::<Object<Tree>, _>::try_from(value.effect()?)?;
    Ok((reply.get(0).read_u64()?, reply.get(1).capture()?))
}

/// A worker-local byte stream. Close it explicitly when finished.
pub struct Stream(u64);

impl Stream {
    pub fn connect(host: &str, port: u16) -> Result<Option<Self>, Error> {
        let (status, value) = request(tree![
            Blob::create(b"tcp_connect"),
            Blob::create(host),
            port as u64
        ])?;
        Ok(if status == 0 {
            Some(Self(value.read_u64()?))
        } else {
            None
        })
    }

    pub fn open_uri(uri: &str) -> Result<Option<Self>, Error> {
        let (status, value) = request(tree![Blob::create(b"uri_open"), Blob::create(uri)])?;
        Ok(if status == 0 {
            Some(Self(value.read_u64()?))
        } else {
            None
        })
    }

    pub fn send(&self, bytes: &[u8]) -> Result<Option<usize>, Error> {
        let (status, value) = request(tree![
            Blob::create(b"stream_send"),
            self.0,
            Blob::create(bytes)
        ])?;
        Ok(if status == 0 {
            Some(usize::try_from(value.read_u64()?)?)
        } else {
            None
        })
    }

    pub fn receive(&self, length: usize) -> Result<Option<Handle<Any, Capture>>, Error> {
        let (status, value) = request(tree![
            Blob::create(b"stream_receive"),
            self.0,
            length as u64
        ])?;
        Ok(if status == 0 { Some(value) } else { None })
    }

    pub fn close(self) -> Result<(), Error> {
        let (status, _) = request(tree![Blob::create(b"stream_close"), self.0])?;
        assert_eq!(status, 0, "stream close failed");
        Ok(())
    }
}

#[cfg(feature = "std")]
mod standard {
    use super::*;
    use std::io;

    impl io::Read for Stream {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if out.is_empty() {
                return Ok(0);
            }
            let chunk = self
                .receive(out.len().min(65536))
                .map_err(io_error)?
                .ok_or(io::ErrorKind::ConnectionReset)?;
            let chunk = Handle::<Object<Blob>, _>::try_from(&chunk).map_err(io_error)?;
            let length =
                usize::try_from(chunk.len().map_err(io_error)?).map_err(io::Error::other)?;
            if length > out.len() {
                return Err(io::ErrorKind::InvalidData.into());
            }
            chunk.read_into(0, &mut out[..length]).map_err(io_error)?;
            Ok(length)
        }
    }

    impl io::Write for Stream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.send(bytes)
                .map_err(io_error)?
                .ok_or_else(|| io::ErrorKind::ConnectionReset.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn io_error(error: Error) -> io::Error {
        io::Error::other(alloc::format!("Fix stream error: {error:?}"))
    }
}
