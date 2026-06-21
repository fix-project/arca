extern crate alloc;
use alloc::vec::Vec;

use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Send(Vec<u8>),
    Receive(usize),
    Close,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Length(usize),
    Bytes(Vec<u8>),
    Ack,
}
