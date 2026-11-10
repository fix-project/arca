#![no_std]
use fixwasm::{Error, Handle, Memory, Object, Tree, Value};
#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<impl Value, Error> {
    let input = Handle::<Object<Tree>>::try_from(combination.get(2))?;
    let mut memory = unsafe { Memory::from_index(3) }.initialize(2)?;
    let bytes = core::array::from_fn::<_, 64, _>(|i| i as u8);
    memory.write(65520, &bytes)?;
    let original = memory.create_blob(65584)?;
    let attached = unsafe { Memory::from_index(4) }.attach(&original);
    let memory = memory.detach().attach(&original);
    let mut memory = memory.make_mut();
    memory.write(65520, &[99; 64])?;
    let changed = memory.create_blob(65584)?;
    let _reply = fixwasm::Blob::create(b"pause").effect()?;
    let mut copied = [0; 64];
    attached.read_into(65520, &mut copied)?;
    assert_eq!(copied, bytes);
    original.read_into(65520, &mut copied)?;
    assert_eq!(copied, bytes);
    changed.read_into(65520, &mut copied)?;
    assert_eq!(copied, [99; 64]);
    assert_eq!(
        attached.read_into(65584, &mut copied),
        Err(Error::OutOfBounds)
    );
    drop(memory);
    let mut fresh = unsafe { Memory::from_index(3) }.initialize(1)?;
    fresh.read_into(0, &mut copied)?;
    assert_eq!(copied, [0; 64]);
    fresh.write(0, &42u64.to_le_bytes())?;
    assert_eq!(fresh.create_blob(8)?.read_u64()?, 42);
    Ok(input.get(0).read_u64()? + input.get(1).read_u64()?)
}
