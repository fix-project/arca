extern crate alloc;
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

pub type Id = [u8; 16];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub id: Id,
    pub repository: String,
    pub workers: usize,
    pub memory: usize,
    pub scheduler: String,
    pub collector: String,
    pub memoize: String,
    pub arguments: Vec<String>,
    pub profile: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Create(Settings),
    Close,
    Join,
    Attach {
        session: Id,
        connection: Id,
    },
    Execute {
        request: u64,
        arguments: Vec<String>,
        stdio: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Opened {
        settings: Settings,
        connection: Id,
        node: Id,
    },
    Attached,
    Closing {
        last: bool,
    },
    Error(String),
    Finished {
        request: u64,
        status: u8,
        output: Vec<u8>,
    },
    Progress(super::control::EvaluationSnapshot),
    Step(super::control::EvaluationStep),
    Diagnostic(Vec<u8>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Command {
    pub connection: Id,
    pub request: u64,
    pub arguments: Vec<String>,
    pub stdio: Option<u64>,
}
