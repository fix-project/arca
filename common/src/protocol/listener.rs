extern crate alloc;

use super::control::PipeData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Accept,
    Close,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Pipe(PipeData),
    Ack,
}
