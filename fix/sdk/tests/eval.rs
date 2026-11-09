#![no_std]

use fix::{Blob, Error, Handle, Object, Path, Tree, Value, ValueType, tree};

struct Callback;
impl Value for Callback {
    type Type = Object<Blob>;
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        let evaluated = 7u64.eval()?;
        fix::invalidate_externrefs()?;
        assert_eq!(evaluated.read_u64()?, 7);
        Blob::create([42; 64]).build_at(path)
    }
}

fn check(value: &impl Value) -> Result<(), Error> {
    let mut bytes = [0; 64];
    value.read_into(0, &mut bytes)?;
    assert_eq!(bytes, [42; 64]);
    Ok(())
}

#[fix::apply]
fn apply(
    combination: &Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Object<Blob>>, Error> {
    let argument = Handle::<Object<Blob>, _>::try_from(combination.get(2))?;
    let mode = combination.get(4).read_u64()?;
    let count = combination.get(5).read_u64()?;
    if mode == 4 {
        let saved = argument.capture()?;
        for _ in 0..count {
            let reply = tree![Blob::create(b"sdk_test"), &saved].effect()?;
            check(&reply)?;
            fix::invalidate_externrefs()?;
            check(&saved)?;
        }
        return saved.unsound_handle_bytes();
    }
    let empty = Blob::create([]).eval()?;
    assert!(empty.is_empty()?);
    drop(empty);
    let opaque = argument.reference().identification().eval()?;
    assert_eq!(opaque.value_type()?, ValueType::Thunk);
    drop(opaque);
    let reference = argument.reference().eval()?;
    assert_eq!(reference.value_type()?, ValueType::Ref);
    drop(reference);
    for _ in 0..count {
        let result = match mode {
            0 => argument.eval()?,
            1 => argument.reference().identification().strict().eval()?,
            2 => tree![0u64, combination.get(3), argument]
                .application()
                .strict()
                .eval()?,
            _ => {
                let first = argument.eval()?;
                let second = tree![&first, 42u64].eval()?;
                let third = argument.eval()?;
                drop(first);
                let reused = argument.eval()?;
                fix::invalidate_externrefs()?;
                check(&third)?;
                check(&reused)?;
                let children = Handle::<Object<Tree>, _>::try_from(second)?;
                check(&children.get(0))?;
                assert_eq!(children.get(1).read_u64()?, 42);
                drop(third);
                Handle::<Object<Blob>, _>::try_from(children.get(0))?
                    .reference()
                    .identification()
                    .strict()
                    .eval()?
            }
        };
        fix::invalidate_externrefs()?;
        if mode == 2 {
            let result = Handle::<Object<Tree>, _>::try_from(result)?;
            check(&result.get(2))?;
        } else {
            check(&result)?;
        }
        check(&argument)?;
    }
    assert!(tree![argument, Callback].is_comparable()?);
    assert!(argument.equals(Callback)?);
    let result = argument.eval()?;
    check(&result)?;
    Ok(Handle::<Object<Blob>, _>::try_from(result)?)
}
