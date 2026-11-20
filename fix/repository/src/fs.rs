use alloc::{string::String, vec::Vec};
use common::protocol::control::{ErrorKind, FileMode, IoErrorKind};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

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
    let mut file = fs::File::open(path).map_err(error)?;
    let length = file.metadata().map_err(error)?.len();
    let progress = (length >= 1024 * 1024)
        .then(|| common::progress::Progress::bytes("Reading repository data", length));
    let mut bytes = Vec::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(error)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(progress) = &progress {
            progress.inc(count as u64);
        }
    }
    if let Some(progress) = progress {
        progress.finish();
    }
    Ok(bytes)
}

pub fn read_range(path: &str, offset: u64, length: usize) -> Result<Vec<u8>, ErrorKind> {
    use std::os::unix::fs::FileExt;
    let file = fs::File::open(path).map_err(error)?;
    let mut bytes = alloc::vec![0; length];
    file.read_exact_at(&mut bytes, offset).map_err(error)?;
    Ok(bytes)
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
    let progress = (bytes.len() >= 1024 * 1024)
        .then(|| common::progress::Progress::bytes("Writing repository data", bytes.len() as u64));
    let result = (|| {
        for chunk in bytes.chunks(65536) {
            file.write_all(chunk)?;
            if let Some(progress) = &progress {
                progress.inc(chunk.len() as u64);
            }
        }
        Ok::<(), std::io::Error>(())
    })();
    if mode.create_new && result.is_err() {
        let _ = fs::remove_file(path);
    }
    result.map_err(error)?;
    if let Some(progress) = progress {
        progress.finish();
    }
    Ok(())
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
