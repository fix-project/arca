use crate::shell::{Root, as_tree, root};
use crate::storage;
use alloc::vec::Vec;
use fixwasm::{Blob, Error, Tree, Value};

fn bytes(value: &Root) -> Result<Vec<u8>, Error> {
    let mut bytes = alloc::vec![0; usize::try_from(value.len()?)?];
    value.read_into(0, &mut bytes)?;
    Ok(bytes)
}

pub fn import_source(uri: &Root) -> Result<Option<Root>, Error> {
    let text = bytes(uri)?;
    let text = core::str::from_utf8(&text).map_err(|_| Error::WrongType)?;
    let Some(object) = fixwasm::importers::fetch(text)? else {
        return Ok(None);
    };
    let value = storage::put(&Root::new(object))?;
    add_sources(&value, core::slice::from_ref(uri))?;
    Ok(Some(value))
}

fn source_bytes(value: &Root) -> Result<Vec<Vec<u8>>, Error> {
    let Some(metadata) = storage::get_todo(value)? else {
        return Ok(Vec::new());
    };
    let metadata = as_tree(&metadata)?;
    (0..usize::try_from(metadata.len()?)?)
        .map(|index| bytes(&root(metadata.get(index))?))
        .collect()
}

fn source_tree(mut uris: Vec<Vec<u8>>) -> Result<Root, Error> {
    uris.sort_unstable();
    uris.dedup();
    let uris = uris
        .iter()
        .map(|uri| root(Blob::create(uri)))
        .collect::<Result<Vec<_>, Error>>()?;
    root(Tree::from_iter(&uris))
}

pub fn sources(value: &Root) -> Result<Root, Error> {
    source_tree(source_bytes(value)?)
}

pub fn add_sources(value: &Root, added: &[Root]) -> Result<Root, Error> {
    let mut uris = source_bytes(value)?;
    for uri in added {
        let uri = bytes(uri)?;
        let text = core::str::from_utf8(&uri).map_err(|_| Error::WrongType)?;
        assert!(
            !text.is_empty() && !text.chars().any(char::is_control),
            "invalid URI"
        );
        uris.push(uri);
    }
    let result = source_tree(uris)?;
    storage::set_todo(value, &result)?;
    Ok(result)
}

pub fn acquire(value: &Root) -> Result<Option<Root>, Error> {
    let mut expected = value.raw_handle_bytes()?;
    expected[31] &= 0xe0;
    if !matches!(expected[31], 0x80 | 0xa0) {
        return Ok(None);
    }
    let Some(metadata) = storage::get_todo(value)? else {
        return Ok(None);
    };
    let uris = as_tree(&metadata)?;
    let mut length = [0; 8];
    length[..6].copy_from_slice(&expected[24..30]);
    let size = usize::try_from(u64::from_le_bytes(length))?;
    for index in 0..usize::try_from(uris.len()?)? {
        let uri = bytes(&root(uris.get(index))?)?;
        let Ok(uri) = core::str::from_utf8(&uri) else {
            continue;
        };
        let object = match expected[31] {
            0x80 => fixwasm::importers::blob(uri, size)?,
            0xa0 => fixwasm::importers::directory(uri)?,
            _ => unreachable!(),
        };
        if let Some(object) = object {
            let object = storage::put(&Root::new(object))?;
            if object.raw_handle_bytes()? == expected {
                return Ok(Some(object));
            }
        }
    }
    Ok(None)
}
