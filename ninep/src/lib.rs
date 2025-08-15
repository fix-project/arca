#![no_std]
#![feature(try_trait_v2)]
#![feature(iterator_try_collect)]
#![feature(try_blocks)]
#![feature(associated_type_defaults)]

extern crate alloc;

use alloc::{
    borrow::ToOwned as _,
    boxed::Box,
    string::{String, ToString as _},
    sync::Arc,
    vec,
    vec::Vec,
};
use derive_more::Display;
use serde::{Deserialize, Serialize};
use vfs::*;

pub mod client;
pub mod msg;
pub mod path;
pub mod server;
pub mod types;
pub mod wire;

pub use client::Client;
use msg::*;
pub use server::Server;
pub use types::*;
pub use wire::Error as WireError;
pub use wire::*;

pub use vfs::Error;
pub use vfs::Result;
