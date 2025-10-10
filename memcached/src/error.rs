use core::{fmt::Display, str::Utf8Error};

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use user::prelude::Value;

pub struct Error(String);

impl From<user::io::Error> for Error {
    fn from(_: user::io::Error) -> Self {
        Error("I/O".to_string())
    }
}

impl From<core::fmt::Error> for Error {
    fn from(e: core::fmt::Error) -> Self {
        Error(e.to_string())
    }
}

impl From<Value> for Error {
    fn from(v: Value) -> Self {
        Error(format!("conversion of {v:?}"))
    }
}

impl<T: core::fmt::Debug> From<Vec<T>> for Error {
    fn from(v: Vec<T>) -> Self {
        Error(format!("conversion of {v:?}"))
    }
}

impl From<Utf8Error> for Error {
    fn from(e: Utf8Error) -> Self {
        Error(e.to_string())
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Error({})", self.0)
    }
}
