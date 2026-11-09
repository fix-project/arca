use super::effects::storage;
use super::{Root, as_tree, child, root};
use alloc::vec::Vec;
use fix::{Blob, Error, Tree, Value};

fn bytes(value: &Root) -> Result<Vec<u8>, Error> {
    let mut bytes = alloc::vec![0; usize::try_from(value.len()?)?];
    value.read_into(0, &mut bytes)?;
    Ok(bytes)
}

pub fn run(input: &Root) -> Result<Root, Error> {
    let action = bytes(&child(input, 0)?)?;
    let (value, added) = match action.as_slice() {
        b"import" => {
            let uri = child(input, 1)?;
            let text = bytes(&uri)?;
            let text = core::str::from_utf8(&text).map_err(|_| Error::WrongType)?;
            let Some(object) = fix::importers::fetch(text)? else {
                return root(fix::tree![1u64, Blob::create(b"could not import URI")]);
            };
            (storage::put(&Root::new(object))?, alloc::vec![uri])
        }
        b"add" => {
            let added = child(input, 2)?;
            let added = as_tree(&added)?;
            let added = (0..usize::try_from(added.len()?)?)
                .map(|index| root(added.get(index)))
                .collect::<Result<Vec<_>, _>>()?;
            (child(input, 1)?, added)
        }
        b"list" => (child(input, 1)?, Vec::new()),
        _ => panic!("unknown todo action"),
    };
    let current = storage::get_todo(&value)?;
    let mut uris = Vec::new();
    if let Some(current) = current {
        let current = as_tree(&current)?;
        for index in 0..usize::try_from(current.len()?)? {
            uris.push(bytes(&root(current.get(index))?)?);
        }
    }
    for uri in added {
        let uri = bytes(&uri)?;
        let text = core::str::from_utf8(&uri).map_err(|_| Error::WrongType)?;
        assert!(
            !text.is_empty() && !text.chars().any(char::is_control),
            "invalid URI"
        );
        uris.push(uri);
    }
    uris.sort_unstable();
    uris.dedup();
    let uris: Vec<_> = uris
        .iter()
        .map(|uri| root(Blob::create(uri)))
        .collect::<Result<_, _>>()?;
    let result = root(Tree::create(&uris))?;
    if action != b"list" {
        storage::set_todo(&value, &result)?;
    }
    if action == b"import" {
        root(fix::tree![0u64, value])
    } else {
        Ok(result)
    }
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
            0x80 => fix::importers::blob(uri, size)?,
            0xa0 => fix::importers::directory(uri)?,
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
