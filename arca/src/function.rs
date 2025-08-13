use super::prelude::*;

impl<R: Runtime> Function<R> {
    pub fn new(data: impl Into<Value<R>>) -> Result<Self, R::Error> {
        R::create_function(data.into())
    }

    pub fn arcane(data: impl Into<Value<R>>) -> Result<Self, R::Error> {
        Self::new(Tuple::from((
            Value::Blob(Blob::from("Arcane")),
            data.into(),
        )))
    }

    pub fn symbolic(value: impl Into<Value<R>>) -> Self {
        Self::new(Tuple::from((
            Value::Blob(Blob::from("Symbolic")),
            value.into(),
        )))
        .unwrap()
    }

    pub fn apply(self, argument: impl Into<Value<R>>) -> Self {
        R::apply_function(self, argument.into())
    }

    pub fn force(self) -> Value<R> {
        R::force_function(self)
    }

    pub fn is_arcane(&self) -> bool {
        R::is_function_arcane(self)
    }

    pub fn is_symbolic(&self) -> bool {
        !self.is_arcane()
    }

    pub fn call_with_current_continuation(self) -> Value<R> {
        R::call_with_current_continuation(self)
    }

    pub fn read(self) -> Value<R> {
        R::read_function(self)
    }

    pub fn read_cloned(&self) -> Value<R> {
        R::read_function(self.clone())
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Continuation;

impl<R: Runtime> FnOnce<(Continuation,)> for Function<R> {
    type Output = Value<R>;

    extern "rust-call" fn call_once(self, _: (Continuation,)) -> Self::Output {
        self.call_with_current_continuation()
    }
}

impl<R: Runtime, A: Into<Value<R>>> FnOnce<(A,)> for Function<R> {
    type Output = Function<R>;

    extern "rust-call" fn call_once(self, args: (A,)) -> Self::Output {
        self.apply(args.0.into())
    }
}

macro_rules! fn_impl {
    ($(($head:ident, $($rest:ident),+) => ($headf:tt, $($restf:tt),+)),+) => {
        $(
        impl<R: Runtime, $head, $($rest),+> FnOnce<($head, $($rest),+)> for Function<R>
        where
            Function<R>: FnOnce<($head,), Output = Function<R>>,
            Function<R>: FnOnce<($($rest),+,)>,
        {
            type Output = <Function<R> as FnOnce<($($rest),+,)>>::Output;

            extern "rust-call" fn call_once(self, args: ($head, $($rest),+)) -> <Function<R> as FnOnce<($($rest),+,)>>::Output {
                self(args.$headf)($(args.$restf),*)
            }
        }
        )*
    };
}

fn_impl! {
    (A, B) => (0, 1),
    (A, B, C) => (0, 1, 2),
    (A, B, C, D) => (0, 1, 2, 3),
    (A, B, C, D, E) => (0, 1, 2, 3, 4),
    (A, B, C, D, E, F) => (0, 1, 2, 3, 4, 5),
    (A, B, C, D, E, F, G) => (0, 1, 2, 3, 4, 5, 6)
}
