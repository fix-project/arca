use crate::files::{self, Stat};
use crate::stream::Stream;
use crate::{Any, Blob, Capture, Error, Handle, Object, Value, tree};
use alloc::vec::Vec;

#[cfg(feature = "git")]
mod git;

mod filesystem;

/// Fetches file bytes or a directory's child descriptors.
pub fn fetch(uri: &str) -> Result<Option<Handle<Any, Capture>>, Error> {
    #[cfg(feature = "git")]
    if is_git(uri) {
        return git::object(uri);
    }
    if let Some(contents) = directory(uri)? {
        return Ok(Some(contents));
    }
    blob(uri, usize::MAX)
}

/// Imports a directory as a Tree of normalized child descriptors.
pub fn directory(uri: &str) -> Result<Option<Handle<Any, Capture>>, Error> {
    #[cfg(feature = "git")]
    if is_git(uri) {
        return git::directory(uri);
    }
    filesystem::directory(uri)
}

/// Fetches a Blob, returning None if unavailable or larger than the limit.
pub fn blob(uri: &str, limit: usize) -> Result<Option<Handle<Any, Capture>>, Error> {
    #[cfg(feature = "git")]
    if is_git(uri) {
        return git::blob(uri, limit);
    }
    let Some(stream) = Stream::open_uri(uri)? else {
        return Ok(None);
    };
    let result = read_blob(&stream, limit);
    stream.close()?;
    result
}

#[cfg(feature = "git")]
fn is_git(uri: &str) -> bool {
    uri.split_once(':')
        .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("git"))
}

fn read_blob(stream: &Stream, limit: usize) -> Result<Option<Handle<Any, Capture>>, Error> {
    let mut contents = Vec::new();
    loop {
        let length = limit
            .saturating_sub(contents.len())
            .saturating_add(1)
            .min(65536);
        let Some(chunk) = stream.receive(length)? else {
            return Ok(None);
        };
        let chunk = Handle::<Object<Blob>, _>::try_from(&chunk)?;
        let length = usize::try_from(chunk.len()?)?;
        if length == 0 {
            return Ok(Some(Blob::create(contents).capture()?));
        }
        if length > limit.saturating_sub(contents.len()) {
            return Ok(None);
        }
        let offset = contents.len();
        contents.resize(offset + length, 0);
        chunk.read_into(0, &mut contents[offset..])?;
    }
}

fn descriptor(
    name: &[u8],
    filetype: u8,
    size: u64,
    data: impl Value,
) -> Result<Handle<Any, Capture>, Error> {
    let stat = Stat {
        filetype,
        nlink: 1,
        size,
        ..Stat::default()
    };
    tree![Blob::create(name), files::write_stat(&stat), data].capture()
}
