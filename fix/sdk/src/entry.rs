#[cfg(feature = "alloc")]
use crate::Choice;
use crate::value::FixType;
use crate::{Any, Blob, Error, Evaluation, Handle, Object, Tree, Value, ValueType};
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub trait Argument: Sized {
    fn from_argument(value: Handle<Any>) -> Result<Self, Error>;
}
impl<T: FixType> Argument for Handle<T> {
    fn from_argument(value: Handle<Any>) -> Result<Self, Error> {
        value.checked()
    }
}
impl Argument for fix::Value<crate::backend::Shell> {
    fn from_argument(value: Handle<Any>) -> Result<Self, Error> {
        Ok(Self::from_inner(value))
    }
}
macro_rules! typed_argument {
    ($($kind:ident),*) => {$(
        impl Argument for fix::$kind<crate::backend::Shell> {
            fn from_argument(value: Handle<Any>) -> Result<Self, Error> {
                Self::try_from(fix::Value::from_inner(value)).map_err(Error::from)
            }
        }
    )*};
}
typed_argument!(Blob, Tree, Object, Reference, Thunk, Encode);

fn read_argument<const N: usize>(value: Handle<Any>) -> Result<[u8; N], Error> {
    let value = Handle::<Object<Blob>>::from_argument(value)?;
    if value.len()? != N as u64 {
        return Err(Error::WrongType);
    }
    let mut bytes = [0; N];
    value.read_into(0, &mut bytes)?;
    Ok(bytes)
}
macro_rules! scalar_argument {
    ($($ty:ty),+ $(,)?) => {$ (
        impl Argument for $ty {
            fn from_argument(value: Handle<Any>) -> Result<Self, Error> {
                Ok(Self::from_le_bytes(read_argument(value)?))
            }
        }
    )*};
}
scalar_argument!(u8, u16, u32, u64, i8, i16, i32, i64, f32, f64);
impl Argument for bool {
    fn from_argument(value: Handle<Any>) -> Result<Self, Error> {
        match read_argument::<1>(value)? {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(Error::WrongType),
        }
    }
}

/// Visits curried arguments in application order without evaluating them.
pub fn visit_arguments(
    combination: &impl Value<Type = Object<Tree>>,
    visit: &mut impl FnMut(Handle<Any>) -> Result<(), Error>,
) -> Result<(), Error> {
    let function = Handle::<Object<Tree>>::try_from(combination.get(1).capture()?)?;
    if !function.is_tag()? {
        visit_arguments(&function, visit)?;
    }
    for index in 2..usize::try_from(combination.len()?)? {
        visit(combination.get(index).capture()?)?;
    }
    Ok(())
}

type CapturedArguments<const N: usize> = ([Handle<Any>; N], usize);

#[doc(hidden)]
pub fn fixed_arguments<const N: usize>(
    combination: &Handle<Object<Tree>>,
) -> Result<Option<CapturedArguments<N>>, Error> {
    let mut arguments = [const { None }; N];
    let mut count = 0;
    visit_arguments(combination, &mut |argument| {
        if count < N {
            arguments[count] = Some(argument);
        }
        count = count.checked_add(1).ok_or(Error::OutOfBounds)?;
        Ok(())
    })?;
    Ok((count >= N).then(|| (arguments.map(Option::unwrap), count)))
}

/// Flattens curried arguments without evaluating them.
#[cfg(feature = "alloc")]
pub fn arguments(combination: &Handle<Object<Tree>>) -> Result<Vec<Handle<Any>>, Error> {
    let mut arguments = Vec::new();
    visit_arguments(combination, &mut |argument| {
        arguments.push(argument);
        Ok(())
    })?;
    Ok(arguments)
}

#[doc(hidden)]
#[cfg(feature = "alloc")]
pub fn normalize_arguments(
    combination: &Handle<Object<Tree>>,
    arguments: &mut [Handle<Any>],
    evaluations: &[Evaluation],
) -> Result<Option<Handle<Any>>, Error> {
    let mut changed = false;
    for (argument, evaluation) in arguments.iter_mut().zip(evaluations) {
        if let Some(prepared) = normalize_argument(&*argument, *evaluation)? {
            *argument = prepared;
            changed = true;
        }
    }
    if !changed {
        return Ok(None);
    }
    Ok(Some(reapply(combination, arguments)?))
}

/// The Encode that evaluates `value` as far as `evaluation` requires, or
/// `None` if `value` needs no evaluation.
pub fn normalize_argument(
    value: &impl Value,
    evaluation: Evaluation,
) -> Result<Option<Handle<Any>>, Error> {
    let thunk = || Handle::<crate::Thunk>::try_from(value.capture()?);
    Ok(Some(match (evaluation, value.value_type()?) {
        (Evaluation::Shallow, ValueType::Object) => Handle::<Object>::try_from(value.capture()?)?
            .reference()
            .erase()
            .capture()?,
        (Evaluation::Lazy, _) | (_, ValueType::Object) => return Ok(None),
        (_, ValueType::Encode) => value.capture()?,
        (Evaluation::Shallow, ValueType::Thunk) => thunk()?.shallow().erase().capture()?,
        (_, ValueType::Thunk) => thunk()?.strict().erase().capture()?,
        (Evaluation::Strict, ValueType::Ref) => {
            value.identification().strict().erase().capture()?
        }
        (_, ValueType::Ref) => return Ok(None),
    }))
}

/// Replaces leading arguments while preserving the curried combination's frames.
#[cfg(feature = "alloc")]
pub fn reapply(
    combination: &Handle<Object<Tree>>,
    arguments: &[Handle<Any>],
) -> Result<Handle<Any>, Error> {
    fn rebuild(
        frame: &impl Value<Type = Object<Tree>>,
        arguments: &[Handle<Any>],
        offset: &mut usize,
    ) -> Result<Handle<Any>, Error> {
        let callee = Handle::<Object<Tree>>::try_from(frame.get(1).capture()?)?;
        let function = if callee.is_tag()? {
            callee.erase().capture()?
        } else {
            rebuild(&callee, arguments, offset)?
        };
        let count = usize::try_from(frame.len()?)?
            .checked_sub(2)
            .ok_or(Error::WrongType)?;
        let start = *offset;
        *offset = start.checked_add(count).ok_or(Error::OutOfBounds)?;
        let prefix = [Choice::Left(frame.get(0)), Choice::Right(&function)];
        let fields = prefix
            .into_iter()
            .map(Choice::Left)
            .chain((0..count).map(|index| {
                Choice::Right(match arguments.get(start + index) {
                    Some(argument) => Choice::Left(argument),
                    None => Choice::Right(frame.get(index + 2)),
                })
            }));
        Tree::from_iter(fields).erase().capture()
    }
    let function = rebuild(combination, arguments, &mut 0)?;
    Handle::<Object<Tree>>::try_from(function)?
        .application()
        .erase()
        .capture()
}

/// Constructs an SDK call; the function position is strict and arguments are lazy.
#[cfg(feature = "alloc")]
pub fn call(
    metadata: impl Value,
    function: impl Value,
    arguments: impl IntoIterator<Item: Value, IntoIter: Clone>,
) -> Result<Handle<Any>, Error> {
    let function = function.erase().capture()?;
    let function = match function.value_type()? {
        ValueType::Object => function,
        ValueType::Thunk => Handle::<crate::Thunk>::try_from(function)?
            .strict()
            .erase()
            .capture()?,
        ValueType::Ref => function.identification().strict().erase().capture()?,
        ValueType::Encode if function.encode_type()? == crate::EncodeType::Strict => function,
        ValueType::Encode => crate::tree![crate::tree![function], 0u64]
            .selection()
            .strict()
            .erase()
            .capture()?,
    };
    let metadata = metadata.erase();
    let prefix = [Choice::Left(&metadata), Choice::Right(&function)];
    let fields = prefix
        .into_iter()
        .map(Choice::Left)
        .chain(arguments.into_iter().map(|arg| Choice::Right(arg.erase())));
    Tree::from_iter(fields).application().erase().capture()
}

/// Applies extra SDK arguments to a function result.
pub fn apply_remaining(
    metadata: impl Value,
    result: impl Value,
    remaining: &[Handle<Any>],
) -> Result<Handle<Any>, Error> {
    let mut result = result.erase();
    for argument in remaining {
        result = apply_one(&metadata, result, argument)?;
    }
    Ok(result)
}

fn apply_one(
    metadata: impl Value,
    function: impl Value,
    argument: impl Value,
) -> Result<Handle<Any>, Error> {
    let function = function.erase();
    let function = match function.value_type()? {
        ValueType::Object => function,
        ValueType::Thunk => function.strict().erase(),
        ValueType::Ref => function.identification().strict().erase(),
        ValueType::Encode if function.encode_type()? == crate::EncodeType::Strict => function,
        ValueType::Encode => crate::tree![crate::tree![function], 0u64]
            .selection()
            .strict()
            .erase(),
    };
    Ok(crate::tree![metadata, function, argument]
        .application()
        .erase())
}

pub fn apply_excess(
    combination: &Handle<Object<Tree>>,
    result: impl Value,
    arity: usize,
) -> Result<Handle<Any>, Error> {
    let mut result = result.erase();
    let mut count = 0;
    visit_arguments(combination, &mut |_| {
        count += 1;
        Ok(())
    })?;
    for index in arity..count {
        result = apply_one(combination.get(0), result, argument(combination, index)?)?;
    }
    Ok(result)
}

/// The flattened argument at `index`, as `visit_arguments` would visit it.
fn argument(
    combination: &impl Value<Type = Object<Tree>>,
    index: usize,
) -> Result<Handle<Any>, Error> {
    let function = Handle::<Object<Tree>>::try_from(combination.get(1).capture()?)?;
    let mut inner = 0;
    if !function.is_tag()? {
        visit_arguments(&function, &mut |_| {
            inner += 1;
            Ok(())
        })?;
        if index < inner {
            return argument(&function, index);
        }
    }
    combination.get(2 + index - inner).capture()
}

/// Constructs the SDK's recoverable `Fix.error` effect.
pub fn error(error: Error) -> Result<Handle<Any>, Error> {
    let message = match error {
        Error::GrowFailed => "GrowFailed",
        Error::OutOfBounds => "OutOfBounds",
        Error::WrongType => "WrongType",
        Error::Busy => "Busy",
        Error::NotComparable => "NotComparable",
    };
    crate::tree![Blob::create(b"Fix.error"), Blob::create(message.as_bytes())].effect()
}
