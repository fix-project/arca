#![no_std]

use fixwasm::{Blob, Error, Handle, Object, Tree, Value, ValueType, tree};

fn check(value: &impl Value) -> Result<(), Error> {
    let mut bytes = [0; 64];
    value.read_into(0, &mut bytes)?;
    assert_eq!(bytes, [42; 64]);
    Ok(())
}

#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<impl Value<Type = Object<Blob>>, Error> {
    let argument = Handle::<Object<Blob>>::try_from(combination.get(2))?;
    let mode = combination.get(4).read_u64()?;
    let count = combination.get(5).read_u64()?;
    if mode == 4 {
        let saved = argument.capture()?;
        for _ in 0..count {
            let reply = tree![Blob::create(b"sdk_test"), &saved].effect()?;
            check(&reply)?;
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
                check(&third)?;
                check(&reused)?;
                let children = Handle::<Object<Tree>>::try_from(second)?;
                check(&children.get(0))?;
                assert_eq!(children.get(1).read_u64()?, 42);
                drop(third);
                Handle::<Object<Blob>>::try_from(children.get(0))?
                    .reference()
                    .identification()
                    .strict()
                    .eval()?
            }
        };
        if mode == 2 {
            let result = Handle::<Object<Tree>>::try_from(result)?;
            check(&result.get(2))?;
        } else {
            check(&result)?;
        }
        check(&argument)?;
    }
    assert!(tree![argument, Blob::create([42; 64])].is_comparable()?);
    assert!(argument.equals(Blob::create([42; 64]))?);
    let result = argument.eval()?;
    check(&result)?;
    Ok(Handle::<Object<Blob>>::try_from(result)?)
}
