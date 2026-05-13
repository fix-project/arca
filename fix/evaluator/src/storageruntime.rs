use crate::fixruntime::{DeterministicEquivRuntime, RuntimeError};
use common::bitpack::BitPack;
use fixhandle::rawhandle::CanonicalHandle;
use std::{fmt::Write, fs, path::PathBuf};

pub struct StorageRuntime {
    objects_dir: PathBuf,
}

impl Default for StorageRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageRuntime {
    pub fn new() -> Self {
        let objects_dir = PathBuf::from(".fix/objects");
        fs::create_dir_all(&objects_dir).expect("failed to create objects directory");
        Self { objects_dir }
    }

    fn write(&self, data: &[u8]) -> CanonicalHandle {
        let handle = CanonicalHandle::new(Self::hash(data), data.len() as u64);
        let path = self
            .objects_dir
            .join(Self::hexadecimal_encode(&handle.pack()));
        fs::write(&path, data).expect("failed to write runtime object");
        handle
    }

    fn read(&self, handle: &[u8]) -> Result<Box<[u8]>, RuntimeError> {
        let path = self.objects_dir.join(Self::hexadecimal_encode(handle));
        fs::read(path)
            .map(|bytes| bytes.into_boxed_slice())
            .map_err(|_| RuntimeError::FileError)
    }

    fn hash(data: &[u8]) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(data);
        *hasher.finalize().as_bytes()
    }

    fn hexadecimal_encode(handle: &[u8]) -> String {
        let mut hex_name = String::with_capacity(handle.len() * 2);
        for &byte in handle {
            write!(&mut hex_name, "{:02x}", byte).expect("valid hex encode");
        }
        hex_name
    }
}

impl DeterministicEquivRuntime for StorageRuntime {
    type BlobData<'a> = Box<[u8]>;
    type TreeData<'a> = Box<[u8]>;
    type Handle = CanonicalHandle;
    type Error = RuntimeError;

    fn create_blob_i32(&mut self, data: u32) -> Self::Handle {
        self.create_blob(&data.to_le_bytes())
    }

    fn create_blob_i64(&mut self, data: u64) -> Self::Handle {
        self.create_blob(&data.to_le_bytes())
    }

    fn create_blob(&mut self, data: &[u8]) -> Self::Handle {
        self.write(data)
    }

    fn create_tree(&mut self, data: &[u8]) -> Self::Handle {
        self.write(data)
    }

    fn get_blob<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::BlobData<'a>, Self::Error> {
        self.read(&handle.pack())
    }

    fn get_tree<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::TreeData<'a>, Self::Error> {
        let data = self.read(&handle.pack())?;
        if data.len() % 32 != 0 {
            return Err(Self::Error::OOB);
        }
        Ok(data)
    }
}
