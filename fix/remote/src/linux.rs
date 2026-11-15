use alloc::string::String;
use std::{
    io,
    net::{TcpStream, ToSocketAddrs},
    path::Path,
};

pub fn sign(identity: &str, namespace: &str, message: &[u8]) -> io::Result<String> {
    sshagent::sign(identity, namespace, message)
}

pub fn sign_with_agent(
    socket: &Path,
    identity: &str,
    namespace: &str,
    message: &[u8],
) -> io::Result<String> {
    sshagent::sign_with_agent(socket, identity, namespace, message)
}

pub struct Connection(pub TcpStream);

impl Connection {
    pub fn connect(address: impl ToSocketAddrs) -> io::Result<Self> {
        TcpStream::connect(address).map(Self)
    }
}

#[derive(Debug)]
pub struct IoError(pub io::Error);

impl core::fmt::Display for IoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}
impl core::error::Error for IoError {}

impl embedded_io::Error for IoError {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

impl embedded_io::ErrorType for Connection {
    type Error = IoError;
}
impl embedded_io::Read for Connection {
    fn read(&mut self, bytes: &mut [u8]) -> Result<usize, Self::Error> {
        io::Read::read(&mut self.0, bytes).map_err(IoError)
    }
}
impl embedded_io::Write for Connection {
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        io::Write::write(&mut self.0, bytes).map_err(IoError)
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        io::Write::flush(&mut self.0).map_err(IoError)
    }
}
