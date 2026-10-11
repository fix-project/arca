use crate::buffer::Region;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request<B = Region> {
    Read(B),
    Write(B),
    Seek(Whence),
    Close,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Length(usize),
    Offset(u64),
    Ack,
    Err(super::control::IoErrorKind),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Whence {
    Start(u64),
    Current(i64),
    End(i64),
}
