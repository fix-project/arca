use super::*;
#[derive(Clone, Copy)]
pub struct Erased<V>(pub(super) V);
impl<V: Value> Value for Erased<V> {
    type Type = Any;
    delegate_value!(this, &this.0);
}
#[derive(Clone, Copy)]
pub enum Choice<L, R> {
    Left(L),
    Right(R),
}
pub fn choose<L: Value, R: Value<Type = L::Type>>(
    condition: bool,
    left: L,
    right: R,
) -> Choice<L, R> {
    if condition {
        Choice::Left(left)
    } else {
        Choice::Right(right)
    }
}
pub(super) fn at_root<T>(path: Option<&Path<'_>>, value: T) -> Result<T, Error> {
    if path.is_some() {
        Err(Error::WrongType)
    } else {
        Ok(value)
    }
}

impl<L: Value, R: Value<Type = L::Type>> Value for Choice<L, R> {
    type Type = L::Type;
    delegate_value!(
        this,
        match this {
            Self::Left(left) => left,
            Self::Right(right) => right,
        }
    );
}
