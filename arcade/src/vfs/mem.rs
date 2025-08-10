use alloc::collections::btree_map::BTreeMap;
use async_trait::async_trait;
use enumflags2::BitFlags;
use kernel::prelude::*;
use ninep::*;

use crate::*;

pub struct MemFS;

#[async_trait]
impl VirtualClosedDir for MemFS {
    fn init(&self) -> BTreeMap<String, VClosedNode> {
        BTreeMap::new()
    }

    async fn open(&self, _: Access) -> Result<Box<dyn VirtualOpenDir>> {
        Ok(Box::new(MemFS))
    }

    async fn mkdir(&mut self, _: &str, _: BitFlags<Perm>, _: BitFlags<Flag>) -> Result<VClosedDir> {
        Ok(MemFS.into())
    }

    async fn create(
        &mut self,
        _: &str,
        _: BitFlags<Perm>,
        _: BitFlags<Flag>,
    ) -> Result<VClosedFile> {
        Ok(MemFile::default().into())
    }
}

#[async_trait]
impl VirtualOpenDir for MemFS {}

#[derive(Default, Clone)]
struct MemFile(Vec<u8>);

#[async_trait]
impl VirtualClosedFile for MemFile {
    async fn open(&self, _: Access) -> Result<Box<dyn VirtualOpenFile>> {
        Ok(Box::new(self.clone()))
    }
}

#[async_trait]
impl VirtualOpenFile for MemFile {
    async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let offset = core::cmp::min(offset, self.0.len());
        let n = core::cmp::min(self.0.len() - offset, buf.len());
        buf[..n].copy_from_slice(&self.0[..n]);
        Ok(n)
    }

    async fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize> {
        let end = offset + buf.len();
        if end > self.0.len() {
            self.0.resize(end, 0);
        }
        self.0[offset..end].copy_from_slice(buf);
        Ok(buf.len())
    }
}
