#![no_std]
#[cfg(feature = "alloc")]
extern crate alloc;
mod memoization;
mod value;
pub use fixtypes::{DataType, EncodeType, ThunkType, ValueType};
pub use memoization::Memoization;
pub use value::{Blob, Encode, Iter, Object, Reference, Thunk, Tree, TypeError, Value};

/// The exit status of a Fix execution that failed with a Fix error: its output names the
/// `Fix.error` request instead of a result.
pub const FIX_ERROR_STATUS: u8 = 3;

/// The Fix shell available to the current execution.
pub trait FixShell {
    type Value: Clone;
    type Error;

    /// Emits optional, non-normative advice. Implementations may discard it.
    fn advise(_value: &Self::Value, _key: &str, _payload: &[u8]) {}
    fn blob(bytes: &[u8]) -> Self::Value;
    fn tree(children: &[Self::Value]) -> Self::Value;
    fn get(tree: &Self::Value, index: usize) -> Result<Self::Value, Self::Error>;
    fn read(blob: &Self::Value, offset: usize, bytes: &mut [u8]) -> Result<(), Self::Error>;
    fn len(value: &Self::Value) -> u64;
    fn value_type(value: &Self::Value) -> ValueType;
    fn data_type(value: &Self::Value) -> DataType;
    fn encode_type(value: &Self::Value) -> EncodeType;
    fn is_tag(value: &Self::Value) -> bool;
    fn is_comparable(value: &Self::Value) -> bool;
    fn equals(left: &Self::Value, right: &Self::Value) -> Result<bool, Self::Error>;
    /// A conservative test for aliasing local values, including noncomparable values.
    fn aliases(_left: &Self::Value, _right: &Self::Value) -> bool {
        false
    }
    fn reference(object: &Self::Value) -> Self::Value;
    fn identification(value: &Self::Value) -> Self::Value;
    fn application(tree: &Self::Value) -> Self::Value;
    fn selection(tree: &Self::Value) -> Self::Value;
    fn strict(thunk: &Self::Value) -> Self::Value;
    fn shallow(thunk: &Self::Value) -> Self::Value;
    fn eval(value: &Self::Value) -> Result<Self::Value, Self::Error>;
}

/// Operations requiring a currently executing Fix combination.
pub trait Invocation: FixShell {
    fn combination() -> Self::Value;
    fn tag(fields: &[Self::Value]) -> Result<Self::Value, Self::Error>;
    fn effect(combination: &Self::Value) -> Self::Value;
}
