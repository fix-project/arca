use serde::{Deserialize, Serialize};

pub mod control;
pub mod file;
pub mod listener;
pub mod stream;

#[derive(Debug, Serialize, Deserialize)]
pub enum Error {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileDescriptor(pub usize);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StreamDescriptor(pub usize);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListenerDescriptor(pub usize);
