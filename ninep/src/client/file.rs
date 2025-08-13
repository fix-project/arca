use super::*;

#[async_trait]
impl<T: FileType> FileLike for P9<T> {}

#[async_trait]
impl ClosedFileLike for P9<ClosedFile> {
    async fn open(self: Box<Self>, access: Access) -> Result<OpenFile> {
        let tag = self.conn.tag();
        let fid = self.fid;
        let qid = if self.conn.linux() {
            let lflags = match access {
                Access::Read => 0,
                Access::Write => 1,
                Access::ReadWrite => 2,
                Access::Execute => 010000000,
            };
            let RMessage::LOpen { qid, .. } = self
                .conn
                .send(TMessage::LOpen {
                    tag,
                    fid,
                    flags: lflags,
                })
                .await??
            else {
                return Err(Error::InputOutputError);
            };
            qid
        } else {
            let RMessage::Open { qid, .. } = self
                .conn
                .send(TMessage::Open { tag, fid, access })
                .await??
            else {
                return Err(Error::InputOutputError);
            };
            qid
        };
        Ok(P9 {
            conn: self.conn,
            fid,
            qid,
            _phantom: PhantomData,
        }
        .into())
    }

    async fn dup(&self) -> Result<ClosedFile> {
        let tag = self.conn.tag();
        let fid = self.fid;
        let newfid = self.conn.fid();
        let RMessage::Walk { .. } = self
            .conn
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
            conn: self.conn.clone(),
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
        let fid = self.fid;
        let tag = self.conn.tag();
        let mut read = 0;
        for chunk in buf.chunks_mut(self.conn.msize() - 24) {
            let RMessage::Read { data, .. } = self
                .conn
                .send(TMessage::Read {
                    tag,
                    fid,
                    offset: (offset + read) as u64,
                    count: chunk.len() as u32,
                })
                .await??
            else {
                return Err(Error::InputOutputError);
            };
            let n = core::cmp::min(data.len(), chunk.len());
            chunk[..n].copy_from_slice(&data[..n]);
            read += n;
            if data.len() != chunk.len() {
                break;
            }
        }
        Ok(read)
    }

    async fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize> {
        let fid = self.fid;
        let mut written = 0;
        for chunk in buf.chunks(self.conn.msize() - 24) {
            let tag = self.conn.tag();
            let RMessage::Write { count, .. } = self
                .conn
                .send(TMessage::Write {
                    tag,
                    fid,
                    offset: (offset + written) as u64,
                    data: chunk.to_owned(),
                })
                .await??
            else {
                return Err(Error::InputOutputError);
            };
            let count = count as usize;
            written += count;
            if count != chunk.len() {
                break;
            }
        }
        Ok(written)
    }
}
