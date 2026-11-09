#![no_std]
use fix::{Any, Blob, Error, Handle, Memory, Object, Table, Tree, Value, tree};

#[fix::apply]
fn apply(combination: &Handle<Object<Tree>, fix::Focus>) -> Result<impl Value, Error> {
    let input = Handle::<Object<Tree>, _>::try_from(combination.get(2))?;
    let mut first = unsafe { Table::from_index(3) };
    let mut second = unsafe { Table::from_index(4) };
    let mut memory = unsafe { Memory::from_index(3) };
    let mut other_memory = unsafe { Memory::from_index(4) };
    let input = first.attach(&input)?;
    let made = tree![input.get(1), input.get(0)].capture()?;
    let made = Handle::<Object<Tree>, _>::try_from(made)?;
    let made = second.attach(&made)?;
    let bytes = memory.attach(&Blob::create([7; 65]))?;
    let other = other_memory.attach(&Blob::create([9; 97]))?;
    let retained = made.get(0).capture()?;
    let nested = tree![tree![input.get(1)]];
    let nested = fix::Cast::<Object<Tree>, _>::try_from(nested.get(0))?;
    assert_eq!(nested.get(0).read_u64()?, 22);
    assert_eq!(
        Blob::create([0; 9]).capture()?.read_u64(),
        Err(Error::WrongType)
    );
    assert_eq!(
        Blob::create([0; 7]).capture()?.read_u64(),
        Err(Error::WrongType)
    );
    fix::invalidate_externrefs()?;
    let _reply = Blob::create(b"pause").effect()?;
    fix::invalidate_externrefs()?;
    assert_eq!(input.len(), 2);
    assert_eq!(made.get(0).read_u64()?, input.get(1).read_u64()?);
    assert_eq!(made.get(1).read_u64()?, input.get(0).read_u64()?);
    assert_eq!(input.get(2).capture().err(), Some(Error::OutOfBounds));
    let mut tail = [0; 1];
    bytes.read_into(64, &mut tail)?;
    assert_eq!(tail, [7]);
    other.read_into(96, &mut tail)?;
    assert_eq!(tail, [9]);
    assert_eq!(bytes.read_into(65, &mut tail), Err(Error::OutOfBounds));
    drop(made);
    let reused = second.attach(&tree![0u64, 42u64])?;
    assert_eq!(reused.get(1).read_u64()?, 42);
    assert_eq!(retained.read_u64()?, input.get(1).read_u64()?);
    let sum = input.iter().try_fold(0u64, |sum, x: Handle<Any, _>| {
        Ok::<_, Error>(sum + x.read_u64()?)
    })?;
    Ok(sum)
}
