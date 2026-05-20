use crate::fixruntime::{DeterministicEquivRuntime, RuntimeError};
use common::bitpack::BitPack;
use fixhandle::rawhandle::{
    BlobName, CanonicalHandle, FixHandle, Handle, LiteralHandle, Object, TreeName,
};
use std::{fmt::Write, fs, io, os::unix::fs::symlink, path::PathBuf};

use hex::FromHex;

pub struct StorageRuntime {
    objects_dir: PathBuf,
    tags_dir: PathBuf,
}

impl Default for StorageRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageRuntime {
    pub fn new() -> Self {
        let objects_dir = PathBuf::from(".fix/objects");
        let tags_dir = PathBuf::from(".fix/tags");
        fs::create_dir_all(&objects_dir).expect("failed to create objects directory");
        fs::create_dir_all(&tags_dir).expect("failed to create objects directory");
        Self {
            objects_dir,
            tags_dir,
        }
    }

    fn read(&self, handle: &[u8]) -> Result<Box<[u8]>, RuntimeError> {
        let path = self.objects_dir.join(Self::hexadecimal_encode(handle));
        fs::read(path)
            .map(|bytes| bytes.into_boxed_slice())
            .map_err(|_| RuntimeError::FileError)
    }

    fn read_tag(&self, handle: &[u8]) -> Result<Box<[u8]>, RuntimeError> {
        let path = self.tags_dir.join(Self::hexadecimal_encode(handle));
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

    fn hexadecimal_decode(handle: &str) -> [u8; 32] {
        <[u8; 32]>::from_hex(handle).expect("Failed to decode hex")
    }
}

impl DeterministicEquivRuntime for StorageRuntime {
    type BlobData<'a> = Box<[u8]>;
    type TreeData<'a> = Box<[u8]>;
    type Handle = FixHandle;
    type Error = RuntimeError;

    fn create_blob_i32(&mut self, data: u32) -> Self::Handle {
        self.create_blob(&data.to_le_bytes())
    }

    fn create_blob_i64(&mut self, data: u64) -> Self::Handle {
        self.create_blob(&data.to_le_bytes())
    }

    fn create_blob(&mut self, data: &[u8]) -> Self::Handle {
        if data.len() <= 30 {
            Object::from(BlobName::Literal(LiteralHandle::new(data))).into()
        } else {
            let handle = CanonicalHandle::new(Self::hash(data), data.len() as u64);
            let handle: FixHandle =
                Object::from(BlobName::Blob(Handle::CanonicalHandle(handle))).into();
            let path = self
                .objects_dir
                .join(Self::hexadecimal_encode(&handle.pack()));
            fs::write(&path, data).expect("failed to write runtime object");
            handle
        }
    }

    fn create_tree(&mut self, data: &[u8]) -> Self::Handle {
        let handle = CanonicalHandle::new(Self::hash(data), data.len() as u64);
        let handle: FixHandle =
            Object::from(TreeName::NotTag(Handle::CanonicalHandle(handle))).into();
        let path = self
            .objects_dir
            .join(Self::hexadecimal_encode(&handle.pack()));
        fs::write(&path, data).expect("failed to write runtime object");
        handle
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

impl StorageRuntime {
    pub fn create_tag(&mut self, handle: &TreeName) -> Result<(), RuntimeError> {
        match handle {
            TreeName::NotTag(_) => Err(RuntimeError::TypeMismatch),
            TreeName::Tag(Handle::CanonicalHandle(canonical)) => {
                let tree_handle: FixHandle =
                    Object::from(TreeName::NotTag(Handle::CanonicalHandle(*canonical))).into();
                let tag_handle: FixHandle =
                    Object::from(TreeName::Tag(Handle::CanonicalHandle(*canonical))).into();
                let tree_path = self
                    .objects_dir
                    .join(Self::hexadecimal_encode(&tree_handle.pack()));
                let tag_path = self
                    .tags_dir
                    .join(Self::hexadecimal_encode(&tag_handle.pack()));
                let tree_path = PathBuf::from("../../").join(tree_path);
                let _ = fs::remove_file(&tag_path);
                symlink(tree_path, tag_path).map_err(|_| RuntimeError::FileError)
            }
            TreeName::Tag(_) => Err(RuntimeError::TypeMismatch),
        }
    }

    pub fn get_tag(&self, handle: &FixHandle) -> Result<Box<[u8]>, RuntimeError> {
        let data = self.read_tag(&handle.pack())?;
        if data.len() % 32 != 0 {
            return Err(RuntimeError::OOB);
        }
        Ok(data)
    }

    fn find_handle(root: &PathBuf, short: &str) -> io::Result<FixHandle> {
        assert_eq!(short.len(), 8);

        let mut matches = Vec::new();

        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                continue;
            }

            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };

            if name.len() == 64 && name[..8].eq_ignore_ascii_case(short) {
                matches.push(String::from(
                    entry.path().file_name().unwrap().to_str().unwrap(),
                ));
            }
        }

        match matches.len() {
            1 => {
                let str = matches.pop().unwrap();
                let res = Self::hexadecimal_decode(str.as_str());
                Ok(FixHandle::unpack(res))
            }
            0 => Err(std::io::Error::other(format!("no match for {short}"))),
            _ => Err(std::io::Error::other(format!(
                "ambiguous prefix {short}: {} matches",
                matches.len()
            ))),
        }
    }

    pub fn get_handle(&self, handle: &str) -> Result<FixHandle, RuntimeError> {
        Self::find_handle(&self.objects_dir, handle).map_err(|_| RuntimeError::FileError)
    }

    pub fn get_tag_handle(&self, handle: &str) -> Result<FixHandle, RuntimeError> {
        Self::find_handle(&self.tags_dir, handle).map_err(|_| RuntimeError::FileError)
    }
}
