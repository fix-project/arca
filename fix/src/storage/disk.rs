extern crate alloc;

use super::*;
use alloc::boxed::Box;
use alloc::format;
use alloc::vec;
use bitint::U48;
use common::protocol::control::ErrorKind;
use fixhandle::canonicalize;
use kernel::host::fs::{self, File};

const OBJECTS_DIR: &str = ".fix/objects";
const LABELS_DIR: &str = ".fix/labels";

/// Canonical Fix storage backed by the existing host filesystem interface.
/// Import establishes the semantic invariants; this backend makes persistence
/// failures and pre-existing corruption observable to that operation.
#[derive(Debug)]
pub struct DiskStorage;

impl DiskStorage {
    pub fn try_new() -> Result<Self, StorageError> {
        fs::mkdir(OBJECTS_DIR)?;
        fs::mkdir(LABELS_DIR)?;
        Ok(Self)
    }

    fn read_object(handle: Handle, expected_len: usize) -> Result<Box<[u8]>, StorageError> {
        let path = format!("{OBJECTS_DIR}/{handle}");
        let mut file = File::open(&path, true, false, false, false, false)?;
        let mut data = vec![0; expected_len];
        if file.read_exact(&mut data) != expected_len {
            return Err(ErrorKind::InvalidData.into());
        }

        let mut extra = [0];
        if file.read(&mut extra) != 0 {
            return Err(ErrorKind::InvalidData.into());
        }
        Ok(data.into())
    }

    fn write_object(handle: Handle, data: &[u8]) -> Result<(), StorageError> {
        match Self::read_object(handle, data.len()) {
            Ok(existing) if existing.as_ref() == data => Ok(()),
            Ok(_) => Err(ErrorKind::InvalidData.into()),
            Err(StorageError(ErrorKind::NotFound)) => {
                let path = format!("{OBJECTS_DIR}/{handle}");
                let mut file = File::open(&path, false, true, true, false, true)?;
                if file.write_exact(data) != data.len() {
                    return Err(ErrorKind::InvalidData.into());
                }
                drop(file);

                let persisted = Self::read_object(handle, data.len())?;
                if persisted.as_ref() != data {
                    return Err(ErrorKind::InvalidData.into());
                }
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}

impl Storage for DiskStorage {
    fn add_blob(&self, data: &[u8]) -> Result<Blob, StorageError> {
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

    fn add_tree(&self, _data: &[Handle]) -> Result<Tree, StorageError> {
        todo!("DiskStorage::add_tree is not implemented")
    }

    fn get_blob(&self, name: Blob) -> Option<Box<[u8]>> {
        match name {
            Blob::Literal(literal) => Some(literal.bytes().into()),
            Blob::Blob(blob) => Self::read_object(Handle::from(Blob::Blob(blob)), blob.len()).ok(),
        }
    }

    fn get_tree(&self, _name: Tree) -> Option<Box<[Handle]>> {
        todo!("DiskStorage::get_tree is not implemented")
    }

    fn import(&self, from: &dyn Storage, handle: Handle) -> Result<Handle, ImportError> {
        match handle {
            Handle::Object(Object::Blob(blob)) if blob.is_literal() => Ok(handle),
            Handle::Object(Object::Blob(blob)) => {
                let bytes = from
                    .get_blob(blob)
                    .ok_or_else(|| ImportError::Unresolved(Handle::from(blob)))?;
                Ok(Handle::from(self.add_blob(&bytes)?))
            }
            _ => todo!("DiskStorage::import only supports Blob objects"),
        }
    }
}
