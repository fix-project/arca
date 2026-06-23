extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    GetArgs,
    Exit(i32),
    Open(String, FileMode),
    Mkdir(String),
    Listen { ip: [u8; 4], port: u16 },
    Connect { host: String, port: u16 },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Args(Vec<String>),
    Pipe(PipeData),
    Ack,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PipeData {
    pub rx_ptr: usize,
    pub rx_len: usize,
    pub tx_ptr: usize,
    pub tx_len: usize,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FileMode {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub append: bool,
    pub truncate: bool,
}
