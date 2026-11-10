#![no_std]
#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

pub use fixcoupon as coupon;
pub use fixtypes::{DataType, EncodeType, ThunkType, ValueType};
pub use macros::apply;
pub use macros::main;
mod entry;
#[cfg(feature = "alloc")]
pub use entry::arguments;
pub use entry::{Argument, apply_remaining, error, normalize_argument, visit_arguments};
#[doc(hidden)]
pub use entry::{apply_excess, fixed_arguments};
#[cfg(feature = "alloc")]
pub use entry::{call, normalize_arguments, reapply};
pub use value::{
    Any, Blob, Choice, Encode, Evaluated, Evaluation, Handle, Object, RawHandle, Ref, Thunk, Tree,
    Value, choose,
};
pub mod ffi;
mod memory;
pub use memory::{Memory, ReadOnly, ReadWrite, Unmapped};

pub mod backend;
pub mod files;
#[cfg(feature = "alloc")]
pub mod importers;
pub mod map;
pub mod stream;
mod value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Error {
    GrowFailed = 1,
    OutOfBounds = 2,
    WrongType = 3,
    Busy = 6,
    NotComparable = 4,
}

impl From<core::num::TryFromIntError> for Error {
    fn from(_: core::num::TryFromIntError) -> Self {
        Self::OutOfBounds
    }
}

#[doc(hidden)]
pub trait Combination: Sized {
    fn from_combination(value: Handle<Any>) -> Result<Self, Error>;
}
impl Combination for Handle<Object<Tree>> {
    fn from_combination(value: Handle<Any>) -> Result<Self, Error> {
        value.checked()
    }
}
impl Combination for fix::Tree<backend::Shell> {
    fn from_combination(value: Handle<Any>) -> Result<Self, Error> {
        Self::try_from(fix::Value::from_inner(value)).map_err(Error::from)
    }
}
impl From<fix::TypeError> for Error {
    fn from(error: fix::TypeError) -> Self {
        match error {
            fix::TypeError::WrongType => Error::WrongType,
        }
    }
}

#[doc(hidden)]
pub fn __apply<C: Combination>(
    index: u32,
    callback: impl FnOnce(&mut C) -> Result<Handle<Any>, Error>,
) -> u32 {
    let _invocation = backend::InvocationGuard::new(index);
    let mut combination =
        C::from_combination(unsafe { Handle::from_raw(index) }).expect("invalid combination");
    callback(&mut combination)
        .expect("Fix procedure failed")
        .into_raw()
}

/// Emits advice about a value without evaluating it or receiving a reply.
/// Advice does not form part of the value or its identity.
pub fn advise(value: &impl Value, key: &str, payload: &[u8]) -> Result<(), Error> {
    let handle = value.to_handle();
    unsafe {
        ffi::advise(
            handle.as_raw(),
            key.as_ptr() as u32,
            key.len().try_into()?,
            payload.as_ptr() as u32,
            payload.len().try_into()?,
        )
    };
    Ok(())
}

pub mod advice {
    pub const TITLE: &str = "fix::title";
    pub const PROGRESS: &str = "fix::progress";
    pub const CANONICALIZE: &str = "fix::canonicalize";
}
