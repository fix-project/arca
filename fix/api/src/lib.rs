#![no_std]
extern crate alloc;

mod memoization;
pub use fixtypes::{DataType, EncodeType, ValueType};
pub use memoization::Memoization;

/// The exit status of a Fix execution that failed with a Fix error: its output names the
/// `Fix.error` request instead of a result.
pub const FIX_ERROR_STATUS: u8 = 3;

pub trait Fix {
    type Value: Clone;
    type Error;

    /// Emits optional, non-normative advice. Implementations may discard it.
    fn advise(
        &mut self,
        _value: &Self::Value,
        _key: &str,
        _payload: &[u8],
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    fn blob(&mut self, bytes: &[u8]) -> Result<Self::Value, Self::Error>;
    fn tree(&mut self, children: &[Self::Value]) -> Result<Self::Value, Self::Error>;
    fn get(&mut self, tree: &Self::Value, index: usize) -> Result<Self::Value, Self::Error>;
    fn read(
        &mut self,
        blob: &Self::Value,
        offset: usize,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error>;
    fn len(&mut self, value: &Self::Value) -> Result<u64, Self::Error>;
    fn value_type(&mut self, value: &Self::Value) -> Result<ValueType, Self::Error>;
    fn data_type(&mut self, value: &Self::Value) -> Result<DataType, Self::Error>;
    fn encode_type(&mut self, value: &Self::Value) -> Result<EncodeType, Self::Error>;
    fn is_tag(&mut self, value: &Self::Value) -> Result<bool, Self::Error>;
    fn is_comparable(&mut self, value: &Self::Value) -> Result<bool, Self::Error>;
    fn equals(&mut self, left: &Self::Value, right: &Self::Value) -> Result<bool, Self::Error>;
    /// A conservative test for aliasing local values, including noncomparable values.
    fn aliases(&self, _left: &Self::Value, _right: &Self::Value) -> bool {
        false
    }
    fn reference(&mut self, object: &Self::Value) -> Result<Self::Value, Self::Error>;
    fn identification(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error>;
    fn application(&mut self, tree: &Self::Value) -> Result<Self::Value, Self::Error>;
    fn selection(&mut self, tree: &Self::Value) -> Result<Self::Value, Self::Error>;
    fn strict(&mut self, thunk: &Self::Value) -> Result<Self::Value, Self::Error>;
    fn shallow(&mut self, thunk: &Self::Value) -> Result<Self::Value, Self::Error>;
    fn eval(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error>;
}

/// Operations requiring a currently executing Fix combination.
pub trait Invocation: Fix {
    fn combination(&mut self) -> Result<Self::Value, Self::Error>;
    fn tag(&mut self, fields: &[Self::Value]) -> Result<Self::Value, Self::Error>;
    fn effect(&mut self, combination: &Self::Value) -> Result<Self::Value, Self::Error>;
}
