use super::*;
use crate::Tree;
use alloc::borrow::ToOwned;
use fixgit::{Object as GitObject, Repository, gix_hash, gix_object};

fn fetch(uri: &str) -> Result<Option<(Repository, GitObject)>, Error> {
    let Ok(remote) = fixgit::Remote::parse(uri) else {
        return Ok(None);
    };
    let Some(stream) = Stream::connect(remote.host(), remote.port())? else {
        return Ok(None);
    };
    let mut stream = stream;
    let result = fixgit::fetch(&mut stream, &remote);
    stream.close()?;
    let Ok(repository) = result else {
        return Ok(None);
    };
    let Ok(object) = repository.resolve(&remote) else {
        return Ok(None);
    };
    Ok(Some((repository, object)))
}

pub(super) fn blob(uri: &str, limit: usize) -> Result<Option<Handle<Any, Capture>>, Error> {
    let Some((_, object)) = fetch(uri)? else {
        return Ok(None);
    };
    if object.kind != gix_object::Kind::Blob || object.data.len() > limit {
        return Ok(None);
    }
    Ok(Some(Blob::create(object.data).capture()?))
}

pub(super) fn object(uri: &str) -> Result<Option<Handle<Any, Capture>>, Error> {
    let Some((repository, object)) = fetch(uri)? else {
        return Ok(None);
    };
    match object.kind {
        gix_object::Kind::Blob => Ok(Some(Blob::create(object.data).capture()?)),
        gix_object::Kind::Tree => directory_contents(&repository, &object, &mut Vec::new()),
        _ => Ok(None),
    }
}

pub(super) fn directory(uri: &str) -> Result<Option<Handle<Any, Capture>>, Error> {
    let Some((repository, object)) = fetch(uri)? else {
        return Ok(None);
    };
    directory_contents(&repository, &object, &mut Vec::new())
}

fn directory_contents(
    repository: &Repository,
    object: &GitObject,
    ancestors: &mut Vec<gix_hash::ObjectId>,
) -> Result<Option<Handle<Any, Capture>>, Error> {
    if object.kind != gix_object::Kind::Tree {
        return Ok(None);
    }
    let Ok(mut entries) = gix_object::TreeRefIter::from_bytes(&object.data, gix_hash::Kind::Sha1)
        .collect::<Result<Vec<_>, _>>()
    else {
        return Ok(None);
    };
    entries.sort_unstable_by(|a, b| a.filename.cmp(b.filename));
    if entries
        .windows(2)
        .any(|pair| pair[0].filename == pair[1].filename)
    {
        return Ok(None);
    }
    let mut children = Vec::with_capacity(entries.len());
    for entry in entries {
        let name: &[u8] = entry.filename.as_ref();
        if name.is_empty()
            || name == b"."
            || name == b".."
            || name.contains(&b'/')
            || name.contains(&0)
        {
            return Ok(None);
        }
        if entry.mode.is_commit() {
            children.push(descriptor(
                name,
                3,
                0,
                Tree::from_iter(core::iter::empty::<u64>()),
            )?);
            continue;
        }
        if !(entry.mode.is_tree() || entry.mode.is_blob() || entry.mode.is_link()) {
            return Ok(None);
        }
        let Ok(object) = repository.object(entry.oid) else {
            return Ok(None);
        };
        if entry.mode.is_tree() {
            if ancestors.iter().any(|oid| oid.as_ref() == entry.oid) {
                return Ok(None);
            }
            ancestors.push(entry.oid.to_owned());
            let child = directory_contents(repository, &object, ancestors)?;
            ancestors.pop();
            let Some(child) = child else { return Ok(None) };
            children.push(descriptor(name, 3, 0, child)?);
        } else {
            if object.kind != gix_object::Kind::Blob {
                return Ok(None);
            }
            let filetype = if entry.mode.is_link() { 7 } else { 4 };
            children.push(descriptor(
                name,
                filetype,
                object.data.len() as u64,
                Blob::create(object.data),
            )?);
        }
    }
    Ok(Some(Tree::from_iter(&children).capture()?))
}
