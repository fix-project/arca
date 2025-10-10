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
