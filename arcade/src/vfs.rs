use alloc::collections::btree_map::BTreeMap;
use async_trait::async_trait;
use enumflags2::BitFlags;
use kernel::prelude::*;

use ninep::*;

pub mod dev;
pub mod fs;
pub mod mem;
pub mod vsock;

#[async_trait]
pub trait VirtualClosedDir: Send + Sync {
    fn init(&self) -> BTreeMap<String, VClosedNode> {
        BTreeMap::new()
    }

    async fn open(&self, access: Access) -> Result<Box<dyn VirtualOpenDir>>;

    async fn mkdir(
        &mut self,
        name: &str,
        perm: BitFlags<Perm>,
        mode: BitFlags<Flag>,
    ) -> Result<VClosedDir>;

    async fn create(
        &mut self,
        name: &str,
        perm: BitFlags<Perm>,
        flags: BitFlags<Flag>,
    ) -> Result<VClosedFile>;
}

#[async_trait]
pub trait VirtualOpenDir: Send + Sync {}

#[async_trait]
pub trait VirtualClosedFile: Send + Sync {
    async fn open(&self, access: Access) -> Result<Box<dyn VirtualOpenFile>>;
}

#[async_trait]
pub trait VirtualOpenFile: Send + Sync {
    async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize>;
    async fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize>;
}

#[derive(Clone)]
pub struct VClosedDir {
    entries: Arc<SpinLock<BTreeMap<String, VClosedNode>>>,
    dir: Arc<SpinLock<dyn VirtualClosedDir>>,
}

impl VClosedDir {
    pub async fn open(&self, access: Access) -> Result<VOpenDir> {
        Ok(VOpenDir {
            entries: self.entries.clone(),
            dir: Arc::from(self.dir.lock().open(access).await?),
            access,
        })
    }
}

#[derive(Clone)]
pub struct VClosedFile {
    file: Arc<dyn VirtualClosedFile>,
}

impl VClosedFile {
    pub async fn open(&self, access: Access) -> Result<VOpenFile> {
        Ok(VOpenFile {
            file: self.file.open(access).await?,
            access,
        })
    }
}

pub struct VOpenDir {
    entries: Arc<SpinLock<BTreeMap<String, VClosedNode>>>,
    dir: Arc<dyn VirtualOpenDir>,
    access: Access,
}

pub struct VOpenFile {
    file: Box<dyn VirtualOpenFile>,
    access: Access,
}

#[derive(Clone)]
pub enum VClosedNode {
    File(VClosedFile),
    Dir(VClosedDir),
}

impl VClosedNode {
    pub fn is_dir(&self) -> bool {
        match self {
            VClosedNode::Dir(_) => true,
            VClosedNode::File(_) => false,
        }
    }

    pub fn is_file(&self) -> bool {
        match self {
            VClosedNode::Dir(_) => false,
            VClosedNode::File(_) => true,
        }
    }
}

pub enum VOpenNode {
    File(VOpenFile),
    Dir(VOpenDir),
}

impl VClosedNode {
    pub async fn open(&self, access: Access) -> Result<OpenNode> {
        Ok(match self {
            VClosedNode::File(f) => OpenNode::File(f.open(access).await?.into()),
            VClosedNode::Dir(d) => OpenNode::Dir(d.open(access).await?.into()),
        })
    }

    pub async fn walk(&self, path: &Path) -> Result<ClosedNode> {
        match self {
            VClosedNode::File(f) => {
                if path.is_empty() {
                    Ok(ClosedNode::File(f.clone().into()))
                } else {
                    Err(Error::NoSuchFileOrDirectory)
                }
            }
            VClosedNode::Dir(d) => d.walk(path).await,
        }
    }
}

#[async_trait]
impl NodeLike for VClosedDir {
    async fn stat(&self) -> Result<Stat> {
        todo!()
    }

    async fn wstat(&mut self, _stat: &Stat) -> Result<()> {
        todo!()
    }

    async fn clunk(self: Box<Self>) -> Result<()> {
        Ok(())
    }

    async fn remove(self: Box<Self>) -> Result<()> {
        Ok(())
    }

    fn qid(&self) -> Qid {
        todo!()
    }
}

#[async_trait]
impl NodeLike for VOpenDir {
    async fn stat(&self) -> Result<Stat> {
        todo!()
    }

    async fn wstat(&mut self, _stat: &Stat) -> Result<()> {
        todo!()
    }

    async fn clunk(self: Box<Self>) -> Result<()> {
        Ok(())
    }

    async fn remove(self: Box<Self>) -> Result<()> {
        Ok(())
    }

    fn qid(&self) -> Qid {
        todo!()
    }
}

#[async_trait]
impl NodeLike for VClosedFile {
    async fn stat(&self) -> Result<Stat> {
        todo!()
    }

    async fn wstat(&mut self, _stat: &Stat) -> Result<()> {
        todo!()
    }

    async fn clunk(self: Box<Self>) -> Result<()> {
        Ok(())
    }

    async fn remove(self: Box<Self>) -> Result<()> {
        Ok(())
    }

    fn qid(&self) -> Qid {
        todo!()
    }
}

#[async_trait]
impl NodeLike for VOpenFile {
    async fn stat(&self) -> Result<Stat> {
        todo!()
    }

    async fn wstat(&mut self, _stat: &Stat) -> Result<()> {
        todo!()
    }

    async fn clunk(self: Box<Self>) -> Result<()> {
        Ok(())
    }

    async fn remove(self: Box<Self>) -> Result<()> {
        Ok(())
    }

    fn qid(&self) -> Qid {
        todo!()
    }
}

#[async_trait]
impl ClosedNodeLike for VClosedDir {}

#[async_trait]
impl OpenNodeLike for VOpenDir {}

#[async_trait]
impl DirLike for VClosedDir {}

#[async_trait]
impl DirLike for VOpenDir {}

#[async_trait]
impl ClosedDirLike for VClosedDir {
    async fn open(self: Box<Self>, access: Access) -> Result<OpenDir> {
        let dir = self.dir.lock().open(access).await?;
        Ok(OpenDir::from(VOpenDir {
            entries: self.entries.clone(),
            dir: dir.into(),
            access,
        }))
    }

    async fn walk(&self, path: &Path) -> Result<ClosedNode> {
        if path.is_empty() {
            return Ok(ClosedNode::Dir(self.clone().into()));
        }
        let (head, rest) = path.split();
        let entries = self.entries.lock();
        let entry = entries.get(head).ok_or(Error::NoSuchFileOrDirectory)?;
        entry.walk(rest).await
    }

    async fn create(
        mut self: Box<Self>,
        name: &str,
        perm: BitFlags<Perm>,
        flags: BitFlags<Flag>,
        access: Access,
    ) -> Result<OpenNode> {
        if flags.contains(Flag::Directory) {
            let dir = self.dir.lock().mkdir(name, perm, flags).await?;
            let dir = VClosedDir::from(dir);
            let mut entries = self.entries.lock();
            entries.insert(name.to_owned(), VClosedNode::Dir(dir.clone()));
            let dir = dir.open(access).await?;
            Ok(OpenNode::Dir(dir.into()))
        } else {
            let file = self.dir.lock().create(name, perm, flags).await?;
            let file = VClosedFile::from(file);
            let mut entries = self.entries.lock();
            entries.insert(name.to_owned(), VClosedNode::File(file.clone()));
            let file = file.open(access).await?;
            Ok(OpenNode::File(file.into()))
        }
    }
}

#[async_trait]
impl OpenDirLike for VOpenDir {
    async fn read(&self, offset: usize, count: usize) -> Result<Vec<Stat>> {
        if self.access != Access::Execute {
            return Err(Error::PermissionDenied);
        }
        let entries = self.entries.lock();
        let start = core::cmp::min(entries.len(), offset);
        let end = core::cmp::min(entries.len(), offset + count);
        Ok(entries.iter().skip(start).take(end - start).map(|(name, node)| {
            Stat {
                name: name.clone(),
                qid: Qid {
                    flags: if node.is_dir() {
                        Flag::Directory.into()
                    } else {
                        BitFlags::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            }
        }).collect())
    }
}

impl ClosedNodeLike for VClosedFile {}
impl OpenNodeLike for VOpenFile {}

impl FileLike for VClosedFile {}
impl FileLike for VOpenFile {}

#[async_trait]
impl ClosedFileLike for VClosedFile {
    async fn open(self: Box<Self>, access: Access) -> Result<OpenFile> {
        let file = self.file.open(access).await?;
        Ok(OpenFile::from(VOpenFile { file, access }))
    }

    async fn dup(&self) -> Result<ClosedFile> {
        Ok(self.clone().into())
    }
}

#[async_trait]
impl OpenFileLike for VOpenFile {
    async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        if !self.access.read() {
            return Err(Error::PermissionDenied);
        }
        self.file.read(offset, buf).await
    }

    async fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize> {
        if !self.access.write() {
            return Err(Error::PermissionDenied);
        }
        self.file.write(offset, buf).await
    }
}

impl<T: VirtualClosedDir + 'static> From<T> for VClosedDir {
    fn from(value: T) -> Self {
        let entries = value.init();
        VClosedDir {
            entries: Arc::new(SpinLock::new(entries)),
            dir: Arc::new(SpinLock::new(value)),
        }
    }
}

impl<T: VirtualClosedFile + 'static> From<T> for VClosedFile {
    fn from(value: T) -> Self {
        VClosedFile {
            file: Arc::new(value),
        }
    }
}
