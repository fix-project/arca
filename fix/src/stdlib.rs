use crate::handle::*;
use crate::storage::Storage;
use kernel::prelude::Vec;

pub const PRIMITIVES: &[(&str, &[u8])] = &[(
    "identity",
    include_bytes!(concat!(env!("OUT_DIR"), "/identity")),
)];

pub fn build_environment(storage: &dyn Storage) -> Handle {
    let mut environment: Vec<Handle> = Vec::new();
    for (name, blob) in PRIMITIVES {
        let name = storage.add_blob(name.as_bytes());
        let primitive = storage.add_blob(blob);
        environment.push(storage.add_tree(&[name.into(), primitive.into()]).into());
    }
    storage.add_tree(&environment).into()
}
