use alloc::{string::String, vec::Vec};
use common::protocol::control::{ErrorKind, FileMode, IoErrorKind};
use std::{fs, io::Write, path::PathBuf};

fn error(error: std::io::Error) -> ErrorKind {
    IoErrorKind::from(error.kind()).into()
}

fn path(path: PathBuf) -> Result<String, ErrorKind> {
    path.into_os_string()
        .into_string()
        .map_err(|_| ErrorKind::InvalidData)
}

pub fn mkdir(path: &str) -> Result<(), ErrorKind> {
    fs::create_dir_all(path).map_err(error)
}

pub fn is_directory(path: &str) -> Result<bool, ErrorKind> {
    fs::metadata(path)
        .map(|metadata| metadata.is_dir())
        .map_err(error)
}

pub fn exists(path: &str) -> Result<bool, ErrorKind> {
    fs::exists(path).map_err(error)
}

pub fn read(path: &str) -> Result<Vec<u8>, ErrorKind> {
    fs::read(path).map_err(error)
}

pub fn read_dir(path: &str) -> Result<Vec<String>, ErrorKind> {
    fs::read_dir(path)
        .map_err(error)?
        .map(|entry| {
            entry
                .map_err(error)?
                .file_name()
                .into_string()
                .map_err(|_| ErrorKind::InvalidData)
        })
        .collect()
}

pub fn write_with(path: &str, bytes: &[u8], mode: FileMode) -> Result<(), ErrorKind> {
    let mut file = fs::OpenOptions::new()
        .read(mode.read)
        .write(mode.write)
        .create(mode.create)
        .create_new(mode.create_new)
        .append(mode.append)
        .truncate(mode.truncate)
        .open(path)
        .map_err(error)?;
    let result = file.write_all(bytes);
    if mode.create_new && result.is_err() {
        let _ = fs::remove_file(path);
    }
    result.map_err(error)
}

pub fn rename(from: &str, to: &str) -> Result<(), ErrorKind> {
    fs::rename(from, to).map_err(error)
}

pub fn symlink(target: &str, path: &str) -> Result<(), ErrorKind> {
    std::os::unix::fs::symlink(target, path).map_err(error)
}

pub fn canonicalize(name: &str) -> Result<String, ErrorKind> {
    path(fs::canonicalize(name).map_err(error)?)
}

pub fn read_link(name: &str) -> Result<String, ErrorKind> {
    path(fs::read_link(name).map_err(error)?)
}

pub fn remove_file(path: &str) -> Result<(), ErrorKind> {
    fs::remove_file(path).map_err(error)
}
