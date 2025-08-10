#![no_std]
#![feature(try_trait_v2)]
#![feature(iterator_try_collect)]

extern crate alloc;

use alloc::{
    borrow::ToOwned as _,
    boxed::Box,
    string::{String, ToString as _},
    sync::Arc,
    vec,
    vec::Vec,
};
use async_trait::async_trait;
use derive_more::{Display, From};
use enumflags2::BitFlags;
use serde::{Deserialize, Serialize};

pub mod client;
pub mod dir;
pub mod file;
pub mod msg;
pub mod node;
pub mod path;
pub mod types;
pub mod wire;

pub use client::*;
pub use dir::*;
pub use file::*;
pub use msg::*;
pub use node::*;
pub use path::*;
pub use types::*;
pub use wire::*;

#[derive(Clone, Debug, Eq, PartialEq, From, Display)]
pub enum Error {
    Message(String),
    InputOutputError,
    PathTooLong,
    NoSuchFileOrDirectory,
    PermissionDenied,
    BadFileDescriptor,
    NotADirectory,
    IsADirectory,
    FileExists,
    OperationNotPermitted,
}

pub type Result<T> = core::result::Result<T, Error>;

pub trait NodeType: Send + Sync {}
impl NodeType for OpenFile {}
impl NodeType for ClosedFile {}
impl NodeType for OpenDir {}
impl NodeType for ClosedDir {}
impl NodeType for OpenNode {}
impl NodeType for ClosedNode {}

pub trait FileType: NodeType {}
impl FileType for OpenFile {}
impl FileType for ClosedFile {}

pub trait DirType: NodeType {}
impl DirType for OpenDir {}
impl DirType for ClosedDir {}

pub trait OpenType: NodeType {}
impl OpenType for OpenFile {}
impl OpenType for OpenDir {}

pub trait ClosedType: NodeType {}
impl ClosedType for ClosedFile {}
impl ClosedType for ClosedDir {}
