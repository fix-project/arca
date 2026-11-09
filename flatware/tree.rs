//! Converts between host files and Flatware descriptor Trees in a Fix repository.

use fixhandle::{Blob, Handle, Tree};
use fixrepository::{Repository, RepositoryError};
use std::{
    fs,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::Path,
};

const DIRECTORY: u8 = 3;
const FILE: u8 = 4;
const SYMLINK: u8 = 7;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Repository(RepositoryError),
    Handle(fixhandle::Error),
    Unsupported(std::path::PathBuf),
    Malformed,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Repository(error) => write!(f, "repository error: {error:?}"),
            Self::Handle(error) => write!(f, "invalid handle: {error:?}"),
            Self::Unsupported(path) => write!(f, "unsupported file type: {}", path.display()),
            Self::Malformed => f.write_str("malformed Flatware descriptor"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<RepositoryError> for Error {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<fixhandle::Error> for Error {
    fn from(error: fixhandle::Error) -> Self {
        Self::Handle(error)
    }
}

fn stat(repository: &Repository, filetype: u8, size: usize) -> Result<Handle, Error> {
    let number = |value: u64| -> Result<Handle, Error> {
        Ok(repository.create_blob(&value.to_le_bytes())?.into())
    };
    let fields = [
        number(0)?,
        repository.create_blob(&[filetype])?.into(),
        number(1)?,
        number(size as u64)?,
        number(0)?,
        number(0)?,
        number(0)?,
    ];
    Ok(repository.create_tree(&fields)?.into())
}

fn descriptor(
    repository: &Repository,
    name: &[u8],
    filetype: u8,
    size: usize,
    contents: Handle,
) -> Result<Tree, Error> {
    let name = repository.create_blob(name)?.into();
    let stat = stat(repository, filetype, size)?;
    Ok(repository.create_tree(&[name, stat, contents])?)
}

/// How [`import`] treats host symlinks.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Symlinks {
    /// Store each symlink's target.
    Preserve,
    /// Import what each symlink refers to, as `cpio --dereference` does.
    Follow,
}

/// Imports `path` as a normalized Flatware descriptor named `name`.
///
/// Directory entries are sorted by name, `.fix` entries are skipped, and every
/// timestamp, device, and link count is normalized.
pub fn import(
    repository: &Repository,
    path: &Path,
    name: &[u8],
    symlinks: Symlinks,
) -> Result<Tree, Error> {
    let (filetype, size, contents) = import_node(repository, path, symlinks)?;
    descriptor(repository, name, filetype, size, contents)
}

/// Imports a file's bytes or a directory's child descriptors.
pub fn import_contents(
    repository: &Repository,
    path: &Path,
    symlinks: Symlinks,
) -> Result<Handle, Error> {
    Ok(import_node(repository, path, symlinks)?.2)
}

fn import_node(
    repository: &Repository,
    path: &Path,
    symlinks: Symlinks,
) -> Result<(u8, usize, Handle), Error> {
    let metadata = match symlinks {
        Symlinks::Preserve => fs::symlink_metadata(path)?,
        Symlinks::Follow => fs::metadata(path)?,
    };
    let kind = metadata.file_type();
    if kind.is_symlink() {
        let target = fs::read_link(path)?.into_os_string().into_vec();
        let contents = repository.create_blob(&target)?;
        return Ok((SYMLINK, target.len(), contents.into()));
    }
    if kind.is_file() {
        let bytes = fs::read(path)?;
        let contents = repository.create_blob(&bytes)?;
        return Ok((FILE, bytes.len(), contents.into()));
    }
    if !kind.is_dir() {
        return Err(Error::Unsupported(path.into()));
    }
    let mut entries = fs::read_dir(path)?
        .map(|entry| Ok(entry?.file_name()))
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    entries.retain(|entry| entry.as_bytes() != b".fix");
    entries.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    let children = entries
        .iter()
        .map(|entry| Ok(import(repository, &path.join(entry), entry.as_bytes(), symlinks)?.into()))
        .collect::<Result<Vec<Handle>, Error>>()?;
    let contents = repository.create_tree(&children)?;
    Ok((DIRECTORY, children.len(), contents.into()))
}

/// Writes the Flatware descriptor `tree` to `path`, which must not exist.
pub fn export(repository: &Repository, tree: Tree, path: &Path) -> Result<(), Error> {
    let descriptor = repository.read_tree(tree)?;
    let [_, stat, contents] = descriptor[..] else {
        return Err(Error::Malformed);
    };
    let fields = repository.read_tree(Tree::try_from(stat)?)?;
    let filetype = fields
        .get(1)
        .map(|field| Blob::try_from(*field))
        .transpose()?
        .map(|blob| repository.read_blob(blob))
        .transpose()?;
    match filetype.as_deref() {
        Some([DIRECTORY]) => {
            fs::create_dir(path)?;
            for child in repository.read_tree(Tree::try_from(contents)?)?.iter() {
                let child = Tree::try_from(*child)?;
                let name =
                    repository.read_blob(Blob::try_from(repository.read_tree(child)?[0])?)?;
                let name = std::ffi::OsStr::from_bytes(&name);
                if name.is_empty() || name == "." || name == ".." || name.as_bytes().contains(&b'/')
                {
                    return Err(Error::Malformed);
                }
                export(repository, child, &path.join(name))?;
            }
        }
        Some([FILE]) => {
            let bytes = repository.read_blob(Blob::try_from(contents)?)?;
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .and_then(|mut file| std::io::Write::write_all(&mut file, &bytes))?;
        }
        Some([SYMLINK]) => {
            let target = repository.read_blob(Blob::try_from(contents)?)?;
            std::os::unix::fs::symlink(std::ffi::OsStr::from_bytes(&target), path)?;
        }
        _ => return Err(Error::Malformed),
    }
    Ok(())
}
