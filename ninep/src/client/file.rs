use super::*;

pub struct File9P {
    pub(super) conn: Arc<Connection>,
    pub(super) fid: Fid,
    pub(super) qid: Qid,
    pub(super) cursor: usize,
    pub(super) spawn: Arc<super::SpawnFn<'static>>,
}

impl File9P {
    pub fn qid(&self) -> Qid {
        self.qid
    }
}

impl File for File9P {
    async fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        let fid = self.fid;
        let tag = self.conn.tag();
        let offset = self.cursor;
        let mut read = 0;
        for chunk in bytes.chunks_mut(self.conn.msize() - 24) {
            send!(self.conn; {data, ..} <- Read {
                tag,
                fid,
                offset: (offset + read) as u64,
                count: chunk.len() as u32
            });
            let n = core::cmp::min(data.len(), chunk.len());
            chunk[..n].copy_from_slice(&data[..n]);
            read += n;
            if data.len() != chunk.len() {
                break;
            }
        }
        self.cursor += read;
        Ok(read)
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        let fid = self.fid;
        let mut written = 0;
        let offset = self.cursor;
        for chunk in bytes.chunks(self.conn.msize() - 24) {
            let tag = self.conn.tag();
            send!(self.conn; {count, ..} <- Write {
                tag,
                fid,
                offset: (offset + written) as u64,
                data: chunk.to_owned(),
            });
            let count = count as usize;
            written += count;
            if count != chunk.len() {
                break;
            }
        }
        self.cursor += written;
        Ok(written)
    }

    async fn seek(&mut self, from: SeekFrom) -> Result<usize> {
        match from {
            SeekFrom::Start(offset) => self.cursor = offset,
            SeekFrom::End(offset) => {
                let tag = self.conn.tag();
                let fid = self.fid;
                send!(self.conn; {stat, ..} <- Stat {tag, fid});
                let len = stat.length as usize;
                self.cursor = len.saturating_add_signed(offset)
            }
            SeekFrom::Current(offset) => self.cursor = self.cursor.saturating_add_signed(offset),
        }
        Ok(self.cursor)
    }

    async fn dup(&self) -> Result<Self> {
        let tag = self.conn.tag();
        let fid = self.fid;
        let newfid = self.conn.fid();
        send!(self.conn; {qid, ..} <- Walk {tag, fid, newfid, name: vec![]});
        let qid = *qid.last().ok_or(Error::from(ErrorKind::Other))?;
        Ok(File9P {
            conn: self.conn.clone(),
            fid: newfid,
            qid,
            cursor: self.cursor,
            spawn: self.spawn.clone(),
        })
    }
}

impl Drop for File9P {
    fn drop(&mut self) {
        let fid = self.fid;
        let conn = self.conn.clone();
        if self.fid != Fid(!0) {
            let future = Box::pin(async move {
                let tag = conn.tag();
                let _ = conn.send(TMessage::Clunk { tag, fid }).await;
            });
            (self.spawn)(future);
        }
    }
}
