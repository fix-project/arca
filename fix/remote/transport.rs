#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use embedded_io::{Read, Write};

#[derive(Debug)]
pub enum Error<E> {
    Io(E),
    Truncated,
    TooLarge,
    WriteZero,
}

pub fn receive<S: Read>(stream: &mut S, limit: usize) -> Result<Option<Vec<u8>>, Error<S::Error>> {
    let mut header = [0; 8];
    if stream.read(&mut header[..1]).map_err(Error::Io)? == 0 {
        return Ok(None);
    }
    read_exact(stream, &mut header[1..])?;
    let length = usize::try_from(u64::from_be_bytes(header)).map_err(|_| Error::TooLarge)?;
    if length > limit {
        return Err(Error::TooLarge);
    }
    let mut bytes = alloc::vec![0; length];
    read_exact(stream, &mut bytes)?;
    Ok(Some(bytes))
}

pub fn send<S: Write>(stream: &mut S, bytes: &[u8]) -> Result<(), Error<S::Error>> {
    write_all(stream, &(bytes.len() as u64).to_be_bytes())?;
    write_all(stream, bytes)?;
    stream.flush().map_err(Error::Io)
}

fn read_exact<S: Read>(stream: &mut S, mut bytes: &mut [u8]) -> Result<(), Error<S::Error>> {
    while !bytes.is_empty() {
        let length = stream.read(bytes).map_err(Error::Io)?;
        if length == 0 {
            return Err(Error::Truncated);
        }
        bytes = &mut bytes[length..];
    }
    Ok(())
}

fn write_all<S: Write>(stream: &mut S, mut bytes: &[u8]) -> Result<(), Error<S::Error>> {
    while !bytes.is_empty() {
        let length = stream.write(bytes).map_err(Error::Io)?;
        if length == 0 {
            return Err(Error::WriteZero);
        }
        bytes = &bytes[length..];
    }
    Ok(())
}
