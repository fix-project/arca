// TODO(kmohr) perhaps use hashbrown here for better performance?
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use futures::{StreamExt, stream::BoxStream};
use vfs::{Create, Dir, DirEnt, Error, ErrorKind, File, Object, Open, Result, SeekFrom};

// minimal in-memory filesystem for serverless evals based on locality
pub struct MemFs {
    files: BTreeMap<String, Vec<u8>>,
}

impl MemFs {
    pub fn new() -> Self {
        Self {
            files: BTreeMap::new(),
        }
    }

    pub fn create_file(&mut self, path: &str) {
        self.files.insert(path.into(), Vec::new());
    }

    pub fn write_file(&mut self, path: &str, data: &[u8]) -> Result<()> {
        if let Some(file) = self.files.get_mut(path) {
            file.extend_from_slice(data);
            Ok(())
        } else {
            Err(ErrorKind::NotFound.into())
        }
    }

    pub fn read_file(&self, path: &str) -> Result<&[u8]> {
        self.files
            .get(path)
            .map(|v| v.as_slice())
            .ok_or(ErrorKind::NotFound.into())
    }

    pub fn list_files(&self) -> Vec<&String> {
        self.files.keys().collect()
    }

    pub fn file_size(&self, path: &str) -> Option<usize> {
        self.files.get(path).map(|data| data.len())
    }

    pub fn clear(&mut self) {
        self.files.clear();
    }

    pub fn add_file(&mut self, path: String, data: Vec<u8>) {
        self.files.insert(path, data);
    }

    pub fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.files
    }

    /// Load files from disk using this function in vmm/arcade main.rs
    pub fn load_from_disk(&mut self, path: &str, data: &[u8]) {
        self.add_file(path.into(), data.into());
    }
}

// VFS trait implementations

/// File handle for MemFs
#[derive(Clone)]
pub struct MemFsFile {
    files: Arc<BTreeMap<String, Vec<u8>>>,
    path: String,
    cursor: Arc<AtomicUsize>,
    open: Open,
}

impl File for MemFsFile {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if !self.open.contains(Open::Read) {
            return Err(ErrorKind::PermissionDenied.into());
        }

        let data = self.files.get(&self.path).ok_or(ErrorKind::NotFound)?;
        let cursor = self.cursor.load(Ordering::Relaxed);
        let remaining = data.len().saturating_sub(cursor);
        let to_read = buf.len().min(remaining);

        if to_read > 0 {
            buf[..to_read].copy_from_slice(&data[cursor..cursor + to_read]);
            self.cursor.store(cursor + to_read, Ordering::Relaxed);
        }

        Ok(to_read)
    }

    async fn write(&mut self, _buf: &[u8]) -> Result<usize> {
        Err(ErrorKind::Unsupported.into())
    }

    async fn seek(&mut self, pos: SeekFrom) -> Result<usize> {
        let data = self.files.get(&self.path).ok_or(ErrorKind::NotFound)?;
        let data_len = data.len();

        let new_cursor = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {
                if offset >= 0 {
                    data_len + offset as usize
                } else {
                    data_len.saturating_sub((-offset) as usize)
                }
            }
            SeekFrom::Current(offset) => {
                let current = self.cursor.load(Ordering::Relaxed);
                if offset >= 0 {
                    current + offset as usize
                } else {
                    current.saturating_sub((-offset) as usize)
                }
            }
        };

        self.cursor.store(new_cursor, Ordering::Relaxed);
        Ok(new_cursor)
    }

    async fn dup(&self) -> Result<Self> {
        Ok(self.clone())
    }
}

impl Dir for MemFs {
    async fn open(&self, name: &str, open: Open) -> Result<Object> {
        let path = alloc::format!("/{}", name);

        log::info!("MemFs: open path '{}'", path);
        // Check if it's a file
        if self.files.contains_key(&path) {
            return Ok(Object::File(
                MemFsFile {
                    files: Arc::new(self.files.clone()),
                    path,
                    cursor: Arc::new(AtomicUsize::new(0)),
                    open,
                }
                .boxed(),
            ));
        }

        // Check if it's a directory (has files with this prefix)
        let dir_prefix = alloc::format!("{}/", path);
        let has_children = self.files.keys().any(|p| p.starts_with(&dir_prefix));

        if has_children {
            // Create a sub-MemFs for this directory
            let mut sub_fs = MemFs::new();
            for (file_path, data) in &self.files {
                if let Some(relative_path) = file_path.strip_prefix(&dir_prefix) {
                    sub_fs.add_file(alloc::format!("/{}", relative_path), data.clone());
                }
            }
            Ok(Object::Dir(sub_fs.boxed()))
        } else {
            Err(ErrorKind::NotFound.into())
        }
    }

    async fn readdir(&self) -> Result<BoxStream<'_, Result<DirEnt>>> {
        let mut entries = Vec::new();
        let mut seen_dirs = BTreeMap::new();

        for path in self.files.keys() {
            if path == "/" {
                continue;
            }
            let trimmed = path.strip_prefix('/').unwrap_or(path);

            if let Some(slash_pos) = trimmed.find('/') {
                // This is in a subdirectory
                let dir_name = &trimmed[..slash_pos];
                if !seen_dirs.contains_key(dir_name) {
                    seen_dirs.insert(dir_name, true);
                    entries.push(Ok(DirEnt {
                        name: dir_name.into(),
                        dir: true,
                    }));
                }
            } else {
                // This is a file in the root
                entries.push(Ok(DirEnt {
                    name: trimmed.into(),
                    dir: false,
                }));
            }
        }

        Ok(futures::stream::iter(entries).boxed())
    }

    async fn create(&self, _name: &str, _create: Create, _open: Open) -> Result<Object> {
        Err(ErrorKind::Unsupported.into())
    }

    async fn remove(&self, _name: &str) -> Result<()> {
        Err(ErrorKind::Unsupported.into())
    }

    async fn dup(&self) -> Result<Self> {
        Ok(MemFs {
            files: self.files.clone(),
        })
    }
}
