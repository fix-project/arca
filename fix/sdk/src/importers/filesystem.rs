use super::*;
use crate::Tree;
use alloc::string::String;

pub(super) fn directory(uri: &str) -> Result<Option<Handle<Any, Capture>>, Error> {
    if !uri
        .split_once(':')
        .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
    {
        return Ok(None);
    }
    let reply = tree![Blob::create(b"uri_read_directory"), Blob::create(uri)].effect()?;
    let reply = Handle::<Object<Tree>, _>::try_from(reply)?;
    if reply.get(0).read_u64()? != 0 {
        return Ok(None);
    }
    let entries = Handle::<Object<Tree>, _>::try_from(reply.get(1).capture()?)?;
    let mut named = Vec::new();
    for entry in entries.iter()? {
        let entry = Handle::<Object<Tree>, _>::try_from(entry)?;
        let name = Handle::<Object<Blob>, _>::try_from(entry.get(0).capture()?)?.read()?;
        if name == b".fix" {
            continue;
        }
        if name.is_empty()
            || name == b"."
            || name == b".."
            || name.contains(&b'/')
            || name.contains(&0)
        {
            return Ok(None);
        }
        named.push((name, entry));
    }
    named.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let mut children = Vec::with_capacity(named.len());
    for (name, entry) in named {
        let kind = u8::try_from(entry.get(1).read_u64()?)?;
        let child_uri = child_uri(uri, &name);
        let contents = match kind {
            3 => directory(&child_uri)?,
            4 => blob(&child_uri, usize::MAX)?,
            7 => Some(entry.get(2).capture()?),
            _ => return Ok(None),
        };
        let Some(contents) = contents else {
            return Ok(None);
        };
        children.push(descriptor(&name, kind, contents.len()?, contents)?);
    }
    Ok(Some(Tree::create(children).capture()?))
}

fn child_uri(parent: &str, name: &[u8]) -> String {
    let mut uri = String::from(parent);
    if !uri.ends_with('/') {
        uri.push('/');
    }
    const HEX: &[u8] = b"0123456789ABCDEF";
    for &byte in name {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            uri.push(byte as char);
        } else {
            uri.push('%');
            uri.push(HEX[(byte >> 4) as usize] as char);
            uri.push(HEX[(byte & 15) as usize] as char);
        }
    }
    uri
}
