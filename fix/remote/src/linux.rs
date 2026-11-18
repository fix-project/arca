use alloc::{format, string::String};
use fixrepository::Remote;
use std::{
    io,
    net::{TcpStream, ToSocketAddrs},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
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

pub enum RemoteConnection {
    Tcp(Connection),
    Ssh {
        child: Child,
        input: ChildStdin,
        output: ChildStdout,
    },
}

impl RemoteConnection {
    pub fn connect(remote: &Remote) -> io::Result<Self> {
        let address = remote.address.as_str();
        if let Some(address) = address.strip_prefix("tcp://") {
            return Connection::connect(address).map(Self::Tcp);
        }
        let ssh = if let Some(address) = address.strip_prefix("ssh://") {
            address
                .split_once('/')
                .map(|(host, _)| (host, &address[host.len()..]))
        } else if address.contains("://") {
            None
        } else {
            address.split_once(':')
        };
        let (host, repository) = ssh.ok_or(io::ErrorKind::InvalidInput)?;
        if host.is_empty() || host.starts_with('-') || repository.is_empty() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let path = format!("'{}'", repository.replace('\'', "'\\''"));
        let mut command = Command::new("ssh");
        command
            .arg("-T")
            .arg("--")
            .arg(host)
            .args(["fix", "-C", &path]);
        if let Some(scheduler) = &remote.scheduler {
            command
                .arg("--scheduler")
                .arg(format!("'{}'", scheduler.replace('\'', "'\\''")));
        }
        if let Some(collector) = &remote.collector {
            command
                .arg("--collector")
                .arg(format!("'{}'", collector.replace('\'', "'\\''")));
        }
        let mut child = command
            .args(["serve", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        common::progress::forward_stderr(child.stderr.take().unwrap());
        Ok(Self::Ssh {
            input: child.stdin.take().unwrap(),
            output: child.stdout.take().unwrap(),
            child,
        })
    }
}

impl Drop for RemoteConnection {
    fn drop(&mut self) {
        if let Self::Ssh { child, .. } = self {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

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
impl embedded_io::ErrorType for RemoteConnection {
    type Error = IoError;
}
impl embedded_io::Read for RemoteConnection {
    fn read(&mut self, bytes: &mut [u8]) -> Result<usize, Self::Error> {
        match self {
            Self::Tcp(stream) => embedded_io::Read::read(stream, bytes),
            Self::Ssh { output, .. } => io::Read::read(output, bytes).map_err(IoError),
        }
    }
}
impl embedded_io::Write for RemoteConnection {
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        match self {
            Self::Tcp(stream) => embedded_io::Write::write(stream, bytes),
            Self::Ssh { input, .. } => io::Write::write(input, bytes).map_err(IoError),
        }
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        match self {
            Self::Tcp(stream) => embedded_io::Write::flush(stream),
            Self::Ssh { input, .. } => io::Write::flush(input).map_err(IoError),
        }
    }
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
