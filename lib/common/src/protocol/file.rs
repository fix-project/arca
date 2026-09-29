extern crate alloc;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Read(usize),
    Write(Vec<u8>),
    Seek(Whence),
    Close,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Bytes(Vec<u8>),
    Length(usize),
    Offset(u64),
    Ack,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Whence {
    Start(u64),
    Current(i64),
    End(i64),
}
