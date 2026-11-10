#![no_std]
use fixwasm::{
    Blob, Error, Handle, Object, Tree, Value,
    files::{self, Stat},
    tree,
};
#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<impl Value, Error> {
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
    let file = files::make_file(Blob::create(b"file"), metadata.erase(), 42u64.erase());
    let saved = Handle::<Object<Tree>>::try_from(files::stat(&file)?)?;
    assert_eq!(files::read_stat(&saved)?, stat);
    let mut name = [0; 2];
    files::name(&file)?.read_into(1, &mut name)?;
    assert_eq!(&name, b"il");
    assert_eq!(
        files::name(&file)?.read_into(usize::MAX, &mut name),
        Err(Error::OutOfBounds)
    );
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
    let author = combination.get(1);
    let additional = tree![&author, &author].tag()?;
    let tag = tree![&author, additional].tag()?;
    assert!(tag.get(1).is_tag()?);
    let tag = tree![&author, 7u64].tag()?;
    assert!(tag.is_tag()?);
    assert!(tag.get(0).equals(&author)?);
    Ok(tree![combination, tag].tag()?)
}
