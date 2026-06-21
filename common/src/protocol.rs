extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    GetArgs,
    Exit(i32),
    // TODO: most of these should be on dedicated File pipes
    Open(String, FileMode),
    Close(FileDescriptor),
    Read(FileDescriptor, usize),
    Write(FileDescriptor, Vec<u8>),
    Seek(FileDescriptor, Whence),
    // TODO: most of these should be on dedicated TCP pipes
    Connect {ip: u32, port: u16},
    Disconnect(StreamDescriptor),
    Send(StreamDescriptor, Vec<u8>),
    Recv(StreamDescriptor, Vec<u8>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Ack,
    Args(Vec<String>),
    File(FileDescriptor),
    Stream(StreamDescriptor),
    Offset(u64),
    Bytes(Vec<u8>),
    Length(usize),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Error {
    FileDoesNotExist,
    InvalidArgument,
    InsufficientPermissions,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileDescriptor(pub usize);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StreamDescriptor(pub usize);

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FileMode {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub append: bool,
    pub truncate: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Whence {
    Start(u64),
    Current(i64),
    End(i64),
}
