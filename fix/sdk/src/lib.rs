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
pub use entry::{
    Argument, apply_remaining, call, error, normalize_argument, reapply, visit_arguments,
};
#[doc(hidden)]
pub use entry::{apply_excess, fixed_arguments, normalize_arguments};
pub use value::{
    Any, Application, AttachedBlob, AttachedElement, AttachedTree, Blob, Capture, Cast, Choice,
    CreateBlob, CreateTag, CreateTree, Encode, Erased, Evaluated, Evaluation, Focus, Handle,
    Identification, Memory, Object, Ref, Reference, Select, Selection, Shallow, Strict, Table,
    Thunk, Tree, Value, choose,
};
#[doc(hidden)]
pub use value::{Cons as __Cons, Nil as __Nil, Path};

#[cfg(feature = "alloc")]
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
pub fn __apply(
    callback: impl for<'a> FnOnce(&'a mut Handle<Object<Tree>, Focus>) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut combination = Handle::<Object<Tree>, Focus>::combination();
    callback(&mut combination)
}

#[doc(hidden)]
pub use value::__finish;

/// Reclaims local object backing. Live references, Rust paths, and pending values remain valid.
pub fn invalidate_externrefs() -> Result<(), Error> {
    value::invalidate_externrefs()
}

/// Emits advice about a value without evaluating it or receiving a reply.
/// Advice does not form part of the value or its identity.
pub fn advise(value: &impl Value, key: &str, payload: &[u8]) -> Result<(), Error> {
    value::advise(value, key, payload)
}

pub mod advice {
    pub const TITLE: &str = "fix::title";
    pub const PROGRESS: &str = "fix::progress";
    pub const CANONICALIZE: &str = "fix::canonicalize";
}
