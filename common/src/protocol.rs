extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub enum ControlRequest {
    GetArgs,
}

#[derive(Serialize, Deserialize)]
pub enum ControlResponse {
    Args(Vec<String>),
}
