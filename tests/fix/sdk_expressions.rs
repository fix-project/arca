#![no_std]
extern crate alloc;
use fix::{Blob, Cast, EncodeType, Error, Handle, Object, Ref, Tree, Value, ValueType, tree};
#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[fix::apply]
fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Object<Tree>> + 'a, Error> {
    for argument in combination.iter()?.skip(1) {
        assert_eq!(argument.value_type()?, ValueType::Thunk);
        assert_eq!(argument.data_type(), Err(Error::WrongType));
        assert!(matches!(
            Handle::<Object<Blob>, _>::try_from(argument.clone()).map(|_| ()),
            Err(Error::WrongType)
        ));
    }
    let values = Tree::from_iter((0..3).map(|index| tree![index as u64, index as u64]));
    assert_eq!(values.clone().len()?, 3);
    for index in 0..3 {
        let child = Cast::<Object<Tree>, _>::try_from(values.get(index))?;
        assert_eq!(
            Cast::<Object<Blob>, _>::try_from(child.get(0))?.read_u64()?,
            index as u64
        );
    }
    let bytes = alloc::vec![7u8; 65537];
    let mut boundary = [0; 2];
    Blob::create(bytes.as_slice()).read_into(65535, &mut boundary)?;
    assert_eq!(boundary, [7, 7]);
    assert_eq!(
        Blob::create(bytes.as_slice()).read_into(65536, &mut boundary),
        Err(Error::OutOfBounds)
    );
    let literal = Blob::create(b"literal");
    let contents = literal.read()?;
    assert_eq!(&contents[1..4], b"ite");
    assert_eq!(contents[0], b'l');
    let sliced = Blob::create(&contents[1..4]);
    let mut selected = [0; 3];
    sliced.read_into(0, &mut selected)?;
    assert_eq!(&selected, b"ite");
    let children = values.children();
    assert_eq!(children.len(), 3);
    let copied = Tree::create(&children[1..]);
    assert_eq!(copied.len()?, 2);
    let item = Cast::<Object<Tree>, _>::try_from(copied.get(0))?;
    assert_eq!(
        Cast::<Object<Blob>, _>::try_from(item.get(0))?.read_u64()?,
        1
    );
    let reference = literal.clone().reference();
    assert_eq!(
        Cast::<Ref<Blob>, _>::try_from(reference.clone().erase())?.len()?,
        7
    );
    let thunk = reference.identification();
    assert_eq!(thunk.value_type()?, ValueType::Thunk);
    assert_eq!(thunk.clone().strict().encode_type()?, EncodeType::Strict);
    assert_eq!(thunk.shallow().encode_type()?, EncodeType::Shallow);
    Ok(tree![42u64, Blob::create(bytes), values])
}
