use super::*;

#[async_trait]
impl<T: FileType> FileLike for P9<T> {}

#[async_trait]
impl ClosedFileLike for P9<ClosedFile> {
    async fn open(self: Box<Self>, access: Access) -> Result<OpenFile> {
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

    async fn dup(&self) -> Result<ClosedFile> {
        let tag = self.connection.tag();
        let fid = self.fid;
        let newfid = self.connection.fid();
        let RMessage::Walk { .. } = self
            .connection
            .send(TMessage::Walk {
                tag,
                fid,
                newfid,
                name: vec![],
            })
            .await??
        else {
            return Err(Error::InputOutputError);
        };
        Ok(P9 {
            connection: self.connection.clone(),
            fid,
            qid: self.qid,
            _phantom: PhantomData,
        }
        .into())
    }
}

#[async_trait]
impl OpenFileLike for P9<OpenFile> {
    async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let tag = self.connection.tag();
        let fid = self.fid;
        let RMessage::Read { data, .. } = self
            .connection
            .send(TMessage::Read {
                tag,
                fid,
                offset: offset as u64,
                count: buf.len() as u32,
            })
            .await??
        else {
            return Err(Error::InputOutputError);
        };
        let n = core::cmp::min(data.len(), buf.len());
        buf[..n].copy_from_slice(&data[..n]);
        Ok(data.len())
    }

    async fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize> {
        let tag = self.connection.tag();
        let fid = self.fid;
        let RMessage::Write { count, .. } = self
            .connection
            .send(TMessage::Write {
                tag,
                fid,
                offset: offset as u64,
                data: buf.to_owned(),
            })
            .await??
        else {
            return Err(Error::InputOutputError);
        };
        Ok(count as usize)
    }
}
