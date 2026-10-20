#![no_std]
use fix::{
    Blob, Cast, CreateTree, Error, Handle, Object, Tree, Value,
    files::{self, Stat},
    tree,
};
#[fix::apply]
fn apply<'a>(
    combination: &'a mut Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = fix::Any> + 'a, Error> {
    let bytes = 42u64.to_le_bytes();
    let offset = 1u64;
    let value = tree![Blob::create(&bytes)].with(0, |child| {
        let child: Cast<Object<Blob>, _> = child.try_into()?;
        Ok(child.read_u64()? + offset)
    })?;
    assert_eq!(value, 43);
    {
        let program: Handle<Object<Tree>, _> = combination.with(1, TryInto::try_into)?;
        let elf: Cast<Object<Blob>, _> = program.get(1).try_into()?;
        assert!(!elf.is_empty()?);
    }
    let source = tree![tree![42u64]];
    let mut calls = 0;
    let source: &dyn Value<Type = Object<Tree>> = &source;
    let nested: Cast<Object<Blob>, _> = source.with(0, |child| {
        calls += 1;
        let child: Cast<Object<Tree>, _> = child.try_into()?;
        child.with(0, TryInto::try_into)
    })?;
    assert_eq!(calls, 1);
    assert_eq!(nested.read_u64()?, 42);
    assert_eq!(nested.read_u64()?, 42);
    assert_eq!(calls, 1);
    assert!(!combination.is_empty()?);
    {
        let pair: CreateTree<_> = (42u64, tree![7u64]).into();
        let view: &dyn Value<Type = Object<Tree>> = &pair;
        let (number, children): (Cast<Object<Blob>, _>, Cast<Object<Tree>, _>) = view.try_into()?;
        let children: &dyn Value<Type = Object<Tree>> = &children;
        let (child,): (Cast<Object<Blob>, _>,) = children.unpack()?;
        assert_eq!(number.read_u64()?, 42);
        assert_eq!(child.read_u64()?, 7);
        assert!(matches!(
            view.unpack::<(Cast<Object<Blob>, _>,)>(),
            Err(Error::WrongType)
        ));
        assert!(matches!(
            view.unpack::<(Cast<Object<Blob>, _>, Cast<Object<Blob>, _>)>(),
            Err(Error::WrongType)
        ));
    }
    let number = Blob::create(42u64.to_le_bytes());
    assert!(number.is_comparable()?);
    assert!(number == Blob::create(42u64.to_le_bytes()));
    assert!(number != Blob::create(43u64.to_le_bytes()));
    let thunk = number.clone().reference().identification();
    assert!(number != thunk);
    assert!(thunk != number);
    assert!(!thunk.eq(&thunk));
    assert_eq!(number.equals(&thunk), Err(Error::NotComparable));
    let stat = Stat {
        dev: 0x0102030405060708,
        filetype: 4,
        nlink: 3,
        size: 4096,
        atim: 100,
        mtim: 200,
        ctim: 300,
    };
    let metadata = files::write_stat(&stat);
    assert_eq!(files::read_stat(&metadata)?, stat);
    let descriptor = files::make_file(
        Blob::create(b"file"),
        metadata.clone().erase(),
        Blob::create(42u64.to_le_bytes()).erase(),
    );
    {
        let saved = Cast::<Object<Tree>, _>::try_from(files::stat(&descriptor)?)?;
        assert_eq!(files::read_stat(&saved)?, stat);
        let name = files::name(&descriptor)?;
        let mut bytes = [0; 2];
        name.clone().read_into(1, &mut bytes)?;
        assert_eq!(&bytes, b"il");
        assert_eq!(
            name.read_into(usize::MAX, &mut bytes),
            Err(Error::OutOfBounds)
        );
    }
    let malformed = tree![
        stat.dev,
        Blob::create([stat.filetype]),
        stat.nlink,
        stat.size,
        stat.atim,
        stat.mtim,
        99u32
    ];
    assert_eq!(files::read_stat(&malformed), Err(Error::WrongType));
    assert_eq!(files::read_stat(&metadata)?, stat);
    {
        let opaque = files::make_file(
            Blob::create(b"opaque"),
            metadata.clone().reference().erase(),
            Blob::create(42u64.to_le_bytes()).reference().erase(),
        );
        let mut name = [0; 6];
        files::name(&opaque)?.read_into(0, &mut name)?;
        assert_eq!(&name, b"opaque");
        assert!(matches!(
            Cast::<Object<Tree>, _>::try_from(files::stat(&opaque)?).map(|_| ()),
            Err(Error::WrongType)
        ));
    }
    assert_eq!(
        Tree::tag(core::iter::empty::<Handle>(), combination, 0).map(|_| ()),
        Err(Error::OutOfBounds)
    );
    let combination: &'a Handle<Object<Tree>, fix::Focus> = combination;
    let author = combination.get(1);
    {
        let additional = tree![tree![], author].tag(combination, 1)?;
        let tag = tree![additional].tag(combination, 1)?;
        assert!(tag.get(0).is_tag()?);
    }
    let content = Blob::create(7u64.to_le_bytes()).erase();
    let tag = tree![tree![author], content].tag(combination, 1)?;
    assert!(tag.is_tag()?);
    assert_eq!(Cast::<Object<Tree>, _>::try_from(tag.get(0))?.len()?, 1);
    let tag = tree![tree![&*combination], tag].tag(combination, 0)?;
    assert_eq!(Cast::<Object<Tree>, _>::try_from(tag.get(0))?.len()?, 2);
    Ok(tag.erase())
}
