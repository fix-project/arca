use crate::handle::*;
use crate::storage::Storage;
use kernel::prelude::Vec;

pub const PRIMITIVES: &[(&str, &[u8])] = &[("identity", include_bytes!(env!("FIX_IDENTITY")))];

pub fn build_environment(storage: &dyn Storage) -> Handle {
    let mut environment: Vec<Handle> = Vec::new();
    for (name, blob) in PRIMITIVES {
        let name = storage.add_blob(name.as_bytes());
        let primitive = storage.add_blob(blob);
        let authors = storage.add_tree(&[primitive.into()]);
        let primitive = storage
            .add_tree(&[
                authors.into(),
                primitive.into(),
                Blob::literal(b"Runnable").unwrap().into(),
            ])
            .tag_descriptor();
        environment.push(storage.add_tree(&[name.into(), primitive.into()]).into());
    }
    storage.add_tree(&environment).into()
}
