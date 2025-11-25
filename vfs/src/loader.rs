use super::*;
use alloc::{string::ToString, vec::Vec};

impl MemDir {
    /// Create a new MemDir with specified permissions
    pub fn new(open: Open) -> Self {
        Self {
            open,
            contents: Default::default(),
        }
    }

    /// Load a file from byte data into this directory
    pub async fn load_file_from_bytes(
        &self,
        name: &str,
        data: &[u8],
    ) -> Result<()> {
        let mem_file = self
            .create(name, Create::default(), Open::ReadWrite)
            .await?
            .as_file()?;

        // Convert to mutable file and write data
        let mut file_obj = mem_file;
        file_obj.write(data).await?;

        Ok(())
    }

    /// Load multiple files from a list of (name, data) pairs
    pub async fn load_files_from_data(
        &self,
        files: &[(&str, &[u8])],
    ) -> Result<()> {
        for (name, data) in files {
            self.load_file_from_bytes(name, data).await?;
        }
        Ok(())
    }

    /// Create a subdirectory and return a reference to it
    pub async fn create_subdir(&self, name: &str) -> Result<Box<dyn Dir>> {
        let subdir = self
            .create(name, Create::Directory, Open::ReadWrite)
            .await?
            .as_dir()?;
        Ok(subdir)
    }
}

#[cfg(feature = "std")]
impl MemDir {
    /// Load files and directories from a disk path into this MemDir
    pub async fn load_from_disk<P: AsRef<std::path::Path>>(
        &self,
        disk_path: P,
    ) -> Result<()> {
        use std::fs;
        
        let entries = fs::read_dir(disk_path).map_err(Error::other)?;

        for entry in entries {
            let entry = entry.map_err(Error::other)?;
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy().to_string();
            let entry_path = entry.path();
            let metadata = entry.metadata().map_err(Error::other)?;

            if metadata.is_dir() {
                // Create subdirectory
                let subdir = self.create_subdir(&name).await?;
                
                // For recursive loading, we'd need to somehow get back to MemDir
                // This is a limitation of the trait object approach
                // You might need to handle subdirectories differently
                
            } else if metadata.is_file() {
                // Read file contents
                let file_contents = fs::read(&entry_path).map_err(Error::other)?;
                
                // Load into memory
                self.load_file_from_bytes(&name, &file_contents).await?;
            }
            // Skip other file types (symlinks, etc.)
        }

        Ok(())
    }

    /// Load a single file from disk into this directory
    pub async fn load_file_from_disk<P: AsRef<std::path::Path>>(
        &self,
        disk_path: P,
        vfs_name: &str,
    ) -> Result<()> {
        use std::fs;
        
        let disk_path = disk_path.as_ref();
        
        if !disk_path.is_file() {
            return Err(Error::from(ErrorKind::NotFound));
        }

        let file_contents = fs::read(disk_path).map_err(Error::other)?;
        self.load_file_from_bytes(vfs_name, &file_contents).await?;

        Ok(())
    }

    /// Create a new MemDir and populate it with contents from disk
    pub async fn from_disk<P: AsRef<std::path::Path>>(disk_path: P) -> Result<Self> {
        let mem_dir = MemDir::new(Open::ReadWrite);
        mem_dir.load_from_disk(disk_path).await?;
        Ok(mem_dir)
    }
}
