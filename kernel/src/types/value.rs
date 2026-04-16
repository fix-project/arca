use super::internal::*;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Value {
    Null(Null),
    Word(Word),
    Blob(Blob),
    Tuple(Tuple),
    Page(Page),
    Table(Table),
    Function(Function),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ValueRef<'a> {
    Null(&'a Null),
    Word(&'a Word),
    Blob(&'a Blob),
    Tuple(&'a Tuple),
    Page(&'a Page),
    Table(&'a Table),
    Function(&'a Function),
}

macro_rules! foreach_type_item {
    ($e:ident) => {
        $e! {Null}
        $e! {Word}
        $e! {Blob}
        $e! {Tuple}
        $e! {Page}
        $e! {Table}
        $e! {Function}
    };
}

macro_rules! impl_tryfrom_value {
    ($x:ident) => {
        impl TryFrom<Value> for $x {
            type Error = Value;

            fn try_from(value: Value) -> Result<Self, Self::Error> {
                if let Value::$x(x) = value {
                    Ok(x)
                } else {
                    Err(value)
                }
            }
        }
    };
}

macro_rules! impl_value_from {
    ($x:ident) => {
        impl From<$x> for Value {
            fn from(value: $x) -> Self {
                Value::$x(value)
            }
        }
    };
}

foreach_type_item! {impl_tryfrom_value}
foreach_type_item! {impl_value_from}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies Word -> Value -> Word conversion round-trips correctly.
    #[test]
    fn test_word_roundtrip() {
        let word = Word::new(99);
        let value: Value = word.clone().into();
        let roundtrip = Word::try_from(value).unwrap();
        assert_eq!(roundtrip, word);
    }

    /// Verifies Blob -> Value -> Blob conversion round-trips correctly.
    #[test]
    fn test_blob_roundtrip() {
        let blob = Blob::new(b"data".to_vec());
        let value: Value = blob.clone().into();
        let roundtrip = Blob::try_from(value).unwrap();
        assert_eq!(roundtrip, blob);
    }

    /// Ensures TryFrom fails when converting to the wrong variant type.
    #[test]
    fn test_mismatched_conversion_fails() {
        let value: Value = Word::new(1).into();
        assert!(Blob::try_from(value).is_err());
    }
}
