#![no_std]

use fix::{EncodeType, Invocation};
use fixwasm::{
    Error, Memory,
    backend::{Blob, Object, Shell, Tree, Value},
};

#[fixwasm::apply]
fn apply(combination: &Tree) -> Result<Value, Error> {
    assert!(combination.len() >= 2);
    assert_eq!(Blob::default(), Blob::new(&[]));
    assert_eq!(Tree::default(), Tree::new(&[]));
    let blob = Blob::from(42u64);
    assert_eq!(blob, Blob::from(42u64));
    assert_ne!(blob, Blob::from(43u64));
    assert_eq!(blob, blob.clone());
    let opaque = Tree::from([Tree::default().application().into()]);
    assert_ne!(opaque, opaque);
    let tree = Tree::new(&[blob.clone().into()]);
    let child = tree.get(0)?.ok_or(Error::OutOfBounds)?;
    assert!(child.equals(blob.as_value())?);
    let child = Blob::try_from(child)?;
    drop(tree);
    drop(blob);
    let mut bytes = [0; 8];
    child.read_exact(0, &mut bytes)?;
    assert_eq!(u64::from_le_bytes(bytes), 42);
    let object: Object = child.clone().into();
    let reference = object.reference();
    let request = reference.identification().strict();
    assert_eq!(request.encode_type(), EncodeType::Strict);
    let result = request.eval()?;
    assert!(result.equals(child.as_value())?);
    let current = Value::from_inner(Shell::combination());
    assert!(
        Object::try_from(current)?
            .as_value()
            .equals(combination.as_value())?
    );
    let memory = unsafe { Memory::from_index(2) }.attach(&child);
    memory.read_into(0, &mut bytes)?;
    assert_eq!(u64::from_le_bytes(bytes), 42);
    let mut memory = memory.make_mut();
    memory.write(0, &43u64.to_le_bytes())?;
    let snapshot = memory.create_blob(8)?;
    snapshot.read_exact(0, &mut bytes)?;
    assert_eq!(u64::from_le_bytes(bytes), 43);
    child.read_exact(0, &mut bytes)?;
    assert_eq!(u64::from_le_bytes(bytes), 42);
    Ok(result)
}
