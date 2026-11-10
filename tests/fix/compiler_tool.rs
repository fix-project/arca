#![no_std]
use fixwasm::{Blob, Error, Handle, Object, Tree, Value, tree};

#[fixwasm::apply]
fn apply(_: &Handle<Object<Tree>>) -> Result<impl Value, Error> {
    let unused = tree![].application().strict();
    let files = tree![
        tree![Blob::create(b"object.o"), tree![], Blob::create(b"object")],
        tree![
            Blob::create(b"program.elf"),
            tree![],
            Blob::create(b"program")
        ],
    ];
    let work = tree![Blob::create(b"/work"), tree![], files];
    Ok(tree![
        tree![unused, tree![], tree![], tree![], work],
        Blob::create(0u32.to_le_bytes()),
    ])
}
