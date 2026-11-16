extern crate alloc;

use super::control::PipeData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Accept,
    Close,
    LocalAddress,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Pipe(PipeData),
    Ack,
    Address([u8; 4], u16),
    Err(super::control::IoErrorKind),
}
