extern crate alloc;

use serde::{Serialize, Deserialize};
use super::control::PipeData;

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
