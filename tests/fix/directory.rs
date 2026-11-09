use std::{
    ops::Deref,
    path::{Path, PathBuf},
};

pub struct Directory(PathBuf);

impl Directory {
    pub fn new(name: &str) -> Self {
        let path = PathBuf::from(std::env::var_os("TEST_TMPDIR").unwrap()).join(name);
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Deref for Directory {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Directory {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
