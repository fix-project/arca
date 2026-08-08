extern crate alloc;

use super::*;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use bitint::U48;
use common::protocol::control::ErrorKind;
use fixhandle::canonicalize;
use kernel::host::fs::{self, File};

const OBJECTS_DIR: &str = ".fix/objects";
const LABELS_DIR: &str = ".fix/labels";

#[derive(Debug, Copy, Clone)]
pub struct DiskError(pub ErrorKind);

impl From<ErrorKind> for DiskError {
    fn from(value: ErrorKind) -> Self {
        Self(value)
    }
}

/// Canonical Fix storage backed by the existing host filesystem interface.
/// Import establishes the semantic invariants; this backend makes persistence
/// failures and pre-existing corruption observable to that operation.
#[derive(Debug)]
pub struct DiskStorage;

impl DiskStorage {
    pub fn try_new() -> Result<Self, DiskError> {
        fs::mkdir(OBJECTS_DIR)?;
        fs::mkdir(LABELS_DIR)?;
        Ok(Self)
    }

    fn object_path(handle: Handle) -> String {
        format!("{OBJECTS_DIR}/{handle}")
    }

    fn invalid_data() -> DiskError {
        DiskError(ErrorKind::InvalidData)
    }

    fn read_file(mut file: File, expected_len: usize) -> Result<Box<[u8]>, DiskError> {
        let mut data = vec![0; expected_len];
        if file.read_exact(&mut data) != expected_len {
            return Err(Self::invalid_data());
        }

        let mut extra = [0];
        if file.read(&mut extra) != 0 {
            return Err(Self::invalid_data());
        }
        Ok(data.into())
    }

    fn read_object(handle: Handle, expected_len: usize) -> Result<Box<[u8]>, DiskError> {
        let file = File::open(&Self::object_path(handle), true, false, false, false, false)?;
        Self::read_file(file, expected_len)
    }

    fn verify_existing(path: &str, expected: &[u8]) -> Result<bool, DiskError> {
        let file = match File::open(path, true, false, false, false, false) {
            Ok(file) => file,
            Err(ErrorKind::NotFound) => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let existing = Self::read_file(file, expected.len())?;
        if existing.as_ref() != expected {
            return Err(Self::invalid_data());
        }
        Ok(true)
    }

    fn write_object(handle: Handle, data: &[u8]) -> Result<(), DiskError> {
        let path = Self::object_path(handle);
        if Self::verify_existing(&path, data)? {
            return Ok(());
        }

        let mut file = File::open(&path, false, true, true, false, true)?;
        if file.write_exact(data) != data.len() {
            return Err(Self::invalid_data());
        }
        drop(file);

        let persisted = Self::read_object(handle, data.len())?;
        if persisted.as_ref() != data {
            return Err(Self::invalid_data());
        }
        Ok(())
    }

    pub(crate) fn try_add_blob(&self, data: &[u8]) -> Result<Blob, DiskError> {
        if data.len() < 30 {
            return Ok(Blob::Literal(LiteralName::new(data)));
        }

        let blob = unsafe {
            BlobName::new(RawName {
                name: canonicalize(data),
                size: U48::new(data.len() as u64).expect("blob larger than 2^48 bytes"),
                meta: RawName::CANONICAL_NAME,
            })
            .into()
        };
        Self::write_object(Handle::from(blob), data)?;
        Ok(blob)
    }

    pub fn try_get_blob(&self, name: Blob) -> Result<Box<[u8]>, DiskError> {
        match name {
            Blob::Literal(literal) => Ok(literal.bytes().into()),
            Blob::Blob(blob) => Self::read_object(Handle::from(Blob::Blob(blob)), blob.len()),
        }
    }

}

impl Storage for DiskStorage {
    fn add_blob(&self, data: &[u8]) -> Blob {
        self.try_add_blob(data)
            .expect("fix: DiskStorage failed to create Blob")
    }

    fn add_tree(&self, _data: &[Handle]) -> Tree {
        todo!("DiskStorage::add_tree is not implemented")
    }

    fn get_blob(&self, name: Blob) -> Option<Box<[u8]>> {
        self.try_get_blob(name).ok()
    }

    fn get_tree(&self, _name: Tree) -> Option<Box<[Handle]>> {
        todo!("DiskStorage::get_tree is not implemented")
    }
}
