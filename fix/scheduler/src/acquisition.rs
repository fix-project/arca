use crate::shell::{Value, as_tree, capture};
use crate::storage;
use alloc::vec::Vec;
use fixwasm::{Blob, Error, Tree, Value as AsValue};

fn bytes(value: &Value) -> Result<Vec<u8>, Error> {
    let mut bytes = alloc::vec![0; usize::try_from(value.len()?)?];
    value.read_into(0, &mut bytes)?;
    Ok(bytes)
}

pub fn import_source(uri: &Value) -> Result<Option<Value>, Error> {
    let text = bytes(uri)?;
    let text = core::str::from_utf8(&text).map_err(|_| Error::WrongType)?;
    let Some(object) = fixwasm::importers::fetch(text)? else {
        return Ok(None);
    };
    let value = storage::put(&Value::from_inner(object))?;
    add_sources(&value, core::slice::from_ref(uri))?;
    Ok(Some(value))
}

fn source_bytes(value: &Value) -> Result<Vec<Vec<u8>>, Error> {
    let Some(metadata) = storage::get_todo(value)? else {
        return Ok(Vec::new());
    };
    let metadata = as_tree(&metadata)?;
    (0..usize::try_from(metadata.len()?)?)
        .map(|index| bytes(&capture(metadata.get(index))?))
        .collect()
}

fn source_tree(mut uris: Vec<Vec<u8>>) -> Result<Value, Error> {
    uris.sort_unstable();
    uris.dedup();
    let uris = uris
        .iter()
        .map(|uri| capture(Blob::create(uri)))
        .collect::<Result<Vec<_>, Error>>()?;
    capture(Tree::from_iter(&uris))
}

pub fn sources(value: &Value) -> Result<Value, Error> {
    source_tree(source_bytes(value)?)
}

pub fn add_sources(value: &Value, added: &[Value]) -> Result<Value, Error> {
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

pub fn acquire_from(value: &Value, uri: &Value) -> Result<Option<Value>, Error> {
    let mut expected = value.raw_handle_bytes()?;
    expected[31] &= 0xe0;
    let mut length = [0; 8];
    length[..6].copy_from_slice(&expected[24..30]);
    let size = usize::try_from(u64::from_le_bytes(length))?;
    let uri = bytes(uri)?;
    let uri = core::str::from_utf8(&uri).map_err(|_| Error::WrongType)?;
    let source = if uri.starts_with("file://") || !uri.contains("://") {
        ""
    } else {
        uri
    };
    let load = crate::progress::load(value, source, "");
    let result = (|| {
        let object = match expected[31] {
            0x80 => fixwasm::importers::blob(uri, size)?,
            0xa0 => fixwasm::importers::directory(uri)?,
            _ => return Ok(None),
        };
        if let Some(object) = object {
            let object = storage::put(&Value::from_inner(object))?;
            if object.raw_handle_bytes()? == expected {
                return Ok(Some(object));
            }
        }
        Ok(None)
    })();
    load.finish(result.as_ref().is_ok_and(|object| object.is_some()));
    result
}
