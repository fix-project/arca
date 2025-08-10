use super::*;

#[async_trait]
impl<T: DirType> DirLike for P9<T> {}

#[async_trait]
impl ClosedDirLike for P9<ClosedDir> {
    async fn open(self: Box<Self>, access: Access) -> Result<OpenDir> {
        let tag = self.connection.tag();
        let fid = self.fid;
        let RMessage::Open { qid, .. } = self
            .connection
            .send(TMessage::Open { tag, fid, access })
            .await??
        else {
            return Err(Error::InputOutputError);
        };
        Ok(P9 {
            connection: self.connection,
            fid,
            qid,
            _phantom: PhantomData,
        }
        .into())
    }

    async fn walk(&self, path: &Path) -> Result<ClosedNode> {
        let tag = self.connection.tag();
        let fid = self.fid;
        let newfid = self.connection.fid();
        let name: Vec<String> = path
            .components()
            .map(|x| match x {
                Component::RootDir => Some("/".to_owned()),
                Component::CurDir => None,
                Component::ParentDir => None,
                Component::Normal(x) => Some(x.to_owned()),
            })
            .try_collect()
            .ok_or(Error::PathTooLong)?;
        let n = name.len();
        let RMessage::Walk { qid, .. } = self
            .connection
            .send(TMessage::Walk {
                tag,
                fid,
                newfid,
                name,
            })
            .await??
        else {
            return Err(Error::InputOutputError);
        };
        if qid.len() != n {
            return Err(Error::NoSuchFileOrDirectory);
        }
        let qid = *qid.last().unwrap_or(&self.qid);
        if qid.flags.contains(Flag::Directory) {
            Ok(ClosedNode::Dir(
                P9 {
                    connection: self.connection.clone(),
                    fid: newfid,
                    qid,
                    _phantom: PhantomData,
                }
                .into(),
            ))
        } else {
            Ok(ClosedNode::File(
                P9 {
                    connection: self.connection.clone(),
                    fid: newfid,
                    qid,
                    _phantom: PhantomData,
                }
                .into(),
            ))
        }
    }

    async fn create(
        self: Box<Self>,
        name: &str,
        perm: BitFlags<Perm>,
        flags: BitFlags<Flag>,
        access: Access,
    ) -> Result<OpenNode> {
        let tag = self.connection.tag();
        let fid = self.fid;
        let RMessage::Create { qid, .. } = self
            .connection
            .send(TMessage::Create {
                tag,
                fid,
                name: name.to_owned(),
                mode: Mode {
                    perm,
                    _skip: 0,
                    flags,
                },
                access,
            })
            .await??
        else {
            return Err(Error::InputOutputError);
        };
        if qid.flags.contains(Flag::Directory) {
            Ok(OpenNode::Dir(
                P9 {
                    connection: self.connection.clone(),
                    fid,
                    qid,
                    _phantom: PhantomData,
                }
                .into(),
            ))
        } else {
            Ok(OpenNode::File(
                P9 {
                    connection: self.connection.clone(),
                    fid,
                    qid,
                    _phantom: PhantomData,
                }
                .into(),
            ))
        }
    }
}

#[async_trait]
impl OpenDirLike for P9<OpenDir> {
    async fn read(&self, offset: usize, count: usize) -> Result<Vec<Stat>> {
        let tag = self.connection.tag();
        let fid = self.fid;
        let RMessage::Read { data, .. } = self
            .connection
            .send(TMessage::Read {
                tag,
                fid,
                offset: offset as u64,
                count: count as u32,
            })
            .await??
        else {
            return Err(Error::InputOutputError);
        };
        let stat: Vec<WireStat> = wire::from_bytes(&data).map_err(|_| Error::InputOutputError)?;
        Ok(stat.into_iter().map(|x| x.0).collect())
    }
}
