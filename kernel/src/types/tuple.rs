use core::ops::{Deref, DerefMut};

use crate::prelude::*;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Tuple {
    contents: Box<[Value]>,
}

impl Tuple {
    pub fn new<T: Into<Box<[Value]>>>(x: T) -> Self {
        Tuple { contents: x.into() }
    }

    pub fn new_with_len(len: usize) -> Self {
        let v = vec![Value::default(); len];
        Tuple { contents: v.into() }
    }

    pub fn into_inner(self) -> Box<[Value]> {
        self.contents
    }
}

impl Deref for Tuple {
    type Target = [Value];

    fn deref(&self) -> &Self::Target {
        &self.contents
    }
}

impl DerefMut for Tuple {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.contents
    }
}

impl FromIterator<Value> for Tuple {
    fn from_iter<T: IntoIterator<Item = Value>>(iter: T) -> Self {
        let v: Box<[Value]> = iter.into_iter().collect();
        Tuple::new(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies new_with_len creates a tuple filled with Null values.
    #[test]
    fn test_new_with_len_defaults_to_null() {
        let tuple = Tuple::new_with_len(2);
        assert_eq!(tuple.len(), 2);
        assert!(matches!(tuple[0], Value::Null(_)));
        assert!(matches!(tuple[1], Value::Null(_)));
    }

    /// Verifies FromIterator collects values into a correctly sized tuple.
    #[test]
    fn test_from_iter() {
        let values: alloc::vec::Vec<Value> =
            alloc::vec![Value::Word(1u64.into()), Value::Blob("x".into()),];
        let tuple: Tuple = values.clone().into_iter().collect();
        assert_eq!(tuple.len(), 2);
        assert_eq!(tuple[0], values[0]);
        assert_eq!(tuple[1], values[1]);
    }
}
