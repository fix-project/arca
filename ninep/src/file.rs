use super::*;

pub struct ClosedFile {
    file: Box<dyn ClosedFileLike>,
}

pub struct OpenFile {
    offset: usize,
    file: Box<dyn OpenFileLike>,
}

#[async_trait]
pub trait ClosedFileLike: ClosedNodeLike + FileLike {
    async fn open(self: Box<Self>, access: Access) -> Result<OpenFile>;
    async fn dup(&self) -> Result<ClosedFile>;
}

#[async_trait]
pub trait OpenFileLike: OpenNodeLike + FileLike {
    async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize>;
    async fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize>;
}

#[async_trait]
pub trait FileLike: NodeLike {}

impl OpenFile {
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let len = self.file.read(self.offset, buf).await?;
        self.offset += len;
        Ok(len)
    }

    pub async fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let len = self.file.write(self.offset, buf).await?;
        self.offset += len;
        Ok(len)
    }

    pub async fn stat(&self) -> Result<Stat> {
        self.file.stat().await
    }
}

impl ClosedFile {
    pub async fn open(self, access: Access) -> Result<OpenFile> {
        self.file.open(access).await
    }

    pub async fn dup(&self) -> Result<ClosedFile> {
        self.file.dup().await
    }
}

impl<T: OpenFileLike + 'static> From<T> for OpenFile {
    fn from(value: T) -> Self {
        OpenFile {
            file: Box::new(value),
            offset: 0,
        }
    }
}

impl<T: ClosedFileLike + 'static> From<T> for ClosedFile {
    fn from(value: T) -> Self {
        ClosedFile {
            file: Box::new(value),
        }
    }
}
