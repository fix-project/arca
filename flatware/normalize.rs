#![no_std]

use fix::files;
use fix::{Error, Handle, Object, Tree, Value, choose, tree};

#[fix::apply]
pub fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Object<Tree>> + 'a, Error> {
    assert_eq!(combination.len()?, 3, "expected a Flatware descriptor");
    let descriptor = Handle::<Object<Tree>, _>::try_from(combination.get(2))?;
    assert_eq!(descriptor.len()?, 3, "expected a Flatware descriptor");
    let metadata = Handle::<Object<Tree>, _>::try_from(descriptor.get(1))?;
    let mut stat = files::read_stat(&metadata)?;
    stat.atim = 0;
    stat.mtim = 0;
    stat.ctim = 0;
    stat.nlink = 1;
    let children = if stat.filetype == 3 {
        Some(Handle::<Object<Tree>, _>::try_from(descriptor.get(2))?)
    } else {
        None
    };
    let length = usize::try_from(
        children
            .map(|children| children.len())
            .transpose()?
            .unwrap_or(0),
    )?;
    let normalizer = combination.get(1);
    let normalized = Tree::from_iter((0..length).map(move |index| {
        tree![
            fix::Blob::create(b""),
            normalizer,
            children.unwrap().get(index)
        ]
        .application()
        .strict()
    }));
    Ok(tree![
        descriptor.get(0),
        files::write_stat(&stat),
        choose(
            stat.filetype == 3,
            normalized.erase(),
            descriptor.get(2).erase()
        )
    ])
}
