use super::*;

pub struct ClosedDir {
    dir: Box<dyn ClosedDirLike>,
}

pub struct OpenDir {
    dir: Box<dyn OpenDirLike>,
    offset: usize,
}

#[async_trait]
pub trait ClosedDirLike: ClosedNodeLike + DirLike {
    async fn open(self: Box<Self>, access: Access) -> Result<OpenDir>;
    async fn walk(&self, path: &Path) -> Result<ClosedNode>;
    async fn create(
        self: Box<Self>,
        name: &str,
        perm: BitFlags<Perm>,
        flags: BitFlags<Flag>,
        access: Access,
    ) -> Result<OpenNode>;

    async fn dup(&self) -> Result<ClosedDir> {
        let ClosedNode::Dir(d) = self.walk("".as_ref()).await? else {
            return Err(Error::InputOutputError);
        };
        Ok(d)
    }
}

#[async_trait]
pub trait OpenDirLike: OpenNodeLike + DirLike {
    async fn read(&self, offset: usize, count: usize) -> Result<Vec<Stat>>;
}

#[async_trait]
pub trait DirLike: NodeLike {}

impl OpenDir {
    pub async fn read(&mut self, count: usize) -> Result<Vec<Stat>> {
        let v = self.dir.read(self.offset, count).await?;
        self.offset += v.len();
        Ok(v)
    }
}

impl ClosedDir {
    pub async fn open(self, access: Access) -> Result<OpenDir> {
        self.dir.open(access).await
    }

    pub async fn walk(&self, path: impl AsRef<Path>) -> Result<ClosedNode> {
        self.dir.walk(path.as_ref()).await
    }

    pub async fn dup(&self) -> Result<ClosedDir> {
        let ClosedNode::Dir(d) = self.dir.walk("".as_ref()).await? else {
            return Err(Error::InputOutputError);
        };
        Ok(d)
    }

    pub async fn create(
        self,
        name: &str,
        perm: BitFlags<Perm>,
        flags: BitFlags<Flag>,
        access: Access,
    ) -> Result<OpenNode> {
        self.dir.create(name, perm, flags, access).await
    }
}

impl<T: OpenDirLike + 'static> From<T> for OpenDir {
    fn from(value: T) -> Self {
        OpenDir {
            dir: Box::new(value),
            offset: 0,
        }
    }
}

impl<T: ClosedDirLike + 'static> From<T> for ClosedDir {
    fn from(value: T) -> Self {
        ClosedDir {
            dir: Box::new(value),
        }
    }
}
