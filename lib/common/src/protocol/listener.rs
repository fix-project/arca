extern crate alloc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Accept,
    Close,
    LocalAddress,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Stream(u64),
    Ack,
    Address([u8; 4], u16),
    Err(super::control::IoErrorKind),
}
