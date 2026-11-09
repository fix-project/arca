#![no_std]

use fix::{Blob, Error, Handle, Object, Path, Tree, Value, tree};

unsafe extern "C" {
    fn checkpoint();
}

struct Callback;
impl Value for Callback {
    type Type = Object<Blob>;
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        fix::invalidate_externrefs()?;
        Blob::create([7; 64]).build_at(path)
    }
}

struct Failure;
impl Value for Failure {
    type Type = Object<Blob>;
    fn build_at(&self, _: Option<&Path<'_>>) -> Result<(), Error> {
        Err(Error::WrongType)
    }
}

#[fix::apply]
fn apply(
    combination: &Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Object<Blob>>, Error> {
    let count = Handle::<Object<Blob>, _>::try_from(combination.get(2))?.read_u64()?;
    let pending = tree![combination.get(2), tree![Blob::create([7; 64])]];
    for _ in 0..2 {
        for index in 0..count {
            assert_eq!(combination.get(2).read_u64()?, count);
            assert!(pending.equals(&pending)?);
            if index % 32 == 31 {
                fix::invalidate_externrefs()?;
            }
        }
        assert!(tree![Blob::create([7; 64]), Callback].is_comparable()?);
        assert!(Blob::create([7; 64]).equals(Callback)?);
        assert_eq!(
            tree![Blob::create([7; 64]), Failure].is_comparable(),
            Err(Error::WrongType)
        );
        assert_eq!(Blob::create([7; 64]).equals(Failure), Err(Error::WrongType));
        fix::invalidate_externrefs()?;
        unsafe { checkpoint() };
    }
    assert_eq!(combination.get(2).read_u64()?, count);
    assert!(pending.equals(&pending)?);
    Ok(Blob::create(42u64.to_le_bytes()))
}
