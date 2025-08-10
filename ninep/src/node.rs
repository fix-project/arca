use super::*;

#[derive(From)]
pub enum OpenNode {
    Dir(OpenDir),
    File(OpenFile),
}

#[derive(From)]
pub enum ClosedNode {
    Dir(ClosedDir),
    File(ClosedFile),
}

impl ClosedNode {
    pub async fn open(self, access: Access) -> Result<OpenNode> {
        Ok(match self {
            ClosedNode::Dir(d) => OpenNode::Dir(d.open(access).await?),
            ClosedNode::File(f) => OpenNode::File(f.open(access).await?),
        })
    }

    pub async fn walk(&self, path: &Path) -> Result<ClosedNode> {
        match self {
            ClosedNode::Dir(d) => d.walk(path).await,
            ClosedNode::File(f) => {
                if path.is_empty() {
                    Ok(ClosedNode::File(f.dup().await?))
                } else {
                    Err(Error::NoSuchFileOrDirectory)
                }
            }
        }
    }

    pub fn is_dir(&self) -> bool {
        match self {
            ClosedNode::Dir(_) => true,
            ClosedNode::File(_) => false,
        }
    }

    pub fn is_file(&self) -> bool {
        match self {
            ClosedNode::Dir(_) => false,
            ClosedNode::File(_) => true,
        }
    }
}

#[async_trait]
pub trait ClosedNodeLike: NodeLike {}

#[async_trait]
pub trait OpenNodeLike: NodeLike {}

#[async_trait]
pub trait NodeLike: Send + Sync {
    async fn stat(&self) -> Result<Stat>;
    async fn wstat(&mut self, stat: &Stat) -> Result<()>;
    async fn clunk(self: Box<Self>) -> Result<()>;
    async fn remove(self: Box<Self>) -> Result<()>;
    fn qid(&self) -> Qid;

    fn flags(&self) -> BitFlags<Flag> {
        self.qid().flags
    }

    fn is_dir(&self) -> bool {
        self.flags().contains(Flag::Directory)
    }

    fn is_file(&self) -> bool {
        !self.is_dir()
    }

    fn is_append_only(&self) -> bool {
        self.flags().contains(Flag::Append)
    }

    fn is_auth(&self) -> bool {
        self.flags().contains(Flag::Authentication)
    }

    fn is_exclusive(&self) -> bool {
        self.flags().contains(Flag::Exclusive)
    }

    fn is_temporary(&self) -> bool {
        self.flags().contains(Flag::Temporary)
    }
}

impl TryFrom<ClosedNode> for ClosedFile {
    type Error = Error;

    fn try_from(value: ClosedNode) -> core::result::Result<Self, Self::Error> {
        match value {
            ClosedNode::Dir(_) => Err(Error::IsADirectory),
            ClosedNode::File(f) => Ok(f),
        }
    }
}

impl TryFrom<ClosedNode> for ClosedDir {
    type Error = Error;

    fn try_from(value: ClosedNode) -> core::result::Result<Self, Self::Error> {
        match value {
            ClosedNode::File(_) => Err(Error::NotADirectory),
            ClosedNode::Dir(d) => Ok(d),
        }
    }
}

impl TryFrom<OpenNode> for OpenFile {
    type Error = Error;

    fn try_from(value: OpenNode) -> core::result::Result<Self, Self::Error> {
        match value {
            OpenNode::Dir(_) => Err(Error::IsADirectory),
            OpenNode::File(f) => Ok(f),
        }
    }
}

impl TryFrom<OpenNode> for OpenDir {
    type Error = Error;

    fn try_from(value: OpenNode) -> core::result::Result<Self, Self::Error> {
        match value {
            OpenNode::File(_) => Err(Error::NotADirectory),
            OpenNode::Dir(d) => Ok(d),
        }
    }
}

impl<'a> TryFrom<&'a ClosedNode> for &'a ClosedFile {
    type Error = Error;

    fn try_from(value: &'a ClosedNode) -> core::result::Result<Self, Self::Error> {
        match value {
            ClosedNode::Dir(_) => Err(Error::IsADirectory),
            ClosedNode::File(f) => Ok(&f),
        }
    }
}

impl<'a> TryFrom<&'a ClosedNode> for &'a ClosedDir {
    type Error = Error;

    fn try_from(value: &'a ClosedNode) -> core::result::Result<Self, Self::Error> {
        match value {
            ClosedNode::File(_) => Err(Error::NotADirectory),
            ClosedNode::Dir(d) => Ok(&d),
        }
    }
}

impl<'a> TryFrom<&'a OpenNode> for &'a OpenFile {
    type Error = Error;

    fn try_from(value: &'a OpenNode) -> core::result::Result<Self, Self::Error> {
        match value {
            OpenNode::Dir(_) => Err(Error::IsADirectory),
            OpenNode::File(f) => Ok(&f),
        }
    }
}

impl<'a> TryFrom<&'a OpenNode> for &'a OpenDir {
    type Error = Error;

    fn try_from(value: &'a OpenNode) -> core::result::Result<Self, Self::Error> {
        match value {
            OpenNode::File(_) => Err(Error::NotADirectory),
            OpenNode::Dir(d) => Ok(&d),
        }
    }
}

impl<'a> TryFrom<&'a mut ClosedNode> for &'a mut ClosedFile {
    type Error = Error;

    fn try_from(value: &'a mut ClosedNode) -> core::result::Result<Self, Self::Error> {
        match value {
            ClosedNode::Dir(_) => Err(Error::IsADirectory),
            ClosedNode::File(f) => Ok(f),
        }
    }
}

impl<'a> TryFrom<&'a mut ClosedNode> for &'a mut ClosedDir {
    type Error = Error;

    fn try_from(value: &'a mut ClosedNode) -> core::result::Result<Self, Self::Error> {
        match value {
            ClosedNode::File(_) => Err(Error::NotADirectory),
            ClosedNode::Dir(d) => Ok(d),
        }
    }
}

impl<'a> TryFrom<&'a mut OpenNode> for &'a mut OpenFile {
    type Error = Error;

    fn try_from(value: &'a mut OpenNode) -> core::result::Result<Self, Self::Error> {
        match value {
            OpenNode::Dir(_) => Err(Error::IsADirectory),
            OpenNode::File(f) => Ok(f),
        }
    }
}

impl<'a> TryFrom<&'a mut OpenNode> for &'a mut OpenDir {
    type Error = Error;

    fn try_from(value: &'a mut OpenNode) -> core::result::Result<Self, Self::Error> {
        match value {
            OpenNode::File(_) => Err(Error::NotADirectory),
            OpenNode::Dir(d) => Ok(d),
        }
    }
}
