#![no_std]
extern crate alloc;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

pub use fixtypes::{DataType, EncodeType, ValueType};
pub use macros::apply;
pub use value::{
    Any, Application, Blob, Cast, Choice, CreateBlob, CreateTree, Encode, Erased, Focus, Handle,
    Identification, Object, Ref, Reference, Select, Selection, Shallow, Strict, Thunk, Tree, Value,
    choose,
};
#[doc(hidden)]
pub use value::{Cons as __Cons, Nil as __Nil, Path};

pub mod files;
mod resource;
mod value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Error {
    GrowFailed = 1,
    OutOfBounds = 2,
    WrongType = 3,
    AllOccupied = 6,
    NotEq = 4,
}

#[doc(hidden)]
pub fn __apply(
    callback: impl for<'a> FnOnce(&'a mut Handle<Object<Tree>, Focus>) -> Result<(), Error>,
) -> Result<(), Error> {
    let resources = resource::Resources::new()?;
    let mut combination = Handle::<Object<Tree>, Focus>::combination();
    let result = callback(&mut combination);
    drop(resources);
    result
}

#[doc(hidden)]
pub use value::__finish;
