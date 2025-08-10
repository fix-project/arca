use super::*;

#[async_trait]
impl<T: NodeType> NodeLike for P9<T> {
    async fn stat(&self) -> Result<Stat> {
        let tag = self.connection.tag();
        let fid = self.fid;
        let RMessage::Stat { stat, .. } =
            self.connection.send(TMessage::Stat { tag, fid }).await??
        else {
            return Err(Error::InputOutputError);
        };
        Ok(stat)
    }

    async fn wstat(&mut self, stat: &Stat) -> Result<()> {
        let tag = self.connection.tag();
        let fid = self.fid;
        let RMessage::WStat { .. } = self
            .connection
            .send(TMessage::WStat {
                tag,
                fid,
                stat: stat.clone(),
            })
            .await??
        else {
            return Err(Error::InputOutputError);
        };
        Ok(())
    }

    async fn clunk(self: Box<Self>) -> Result<()> {
        let tag = self.connection.tag();
        let fid = self.fid;
        let RMessage::Clunk { .. } = self.connection.send(TMessage::Clunk { tag, fid }).await??
        else {
            return Err(Error::InputOutputError);
        };
        Ok(())
    }

    async fn remove(self: Box<Self>) -> Result<()> {
        let tag = self.connection.tag();
        let fid = self.fid;
        let RMessage::Remove { .. } = self.connection.send(TMessage::Clunk { tag, fid }).await??
        else {
            return Err(Error::InputOutputError);
        };
        Ok(())
    }

    fn qid(&self) -> Qid {
        self.qid
    }
}

#[async_trait]
impl<T: OpenType> OpenNodeLike for P9<T> {}

#[async_trait]
impl<T: ClosedType> ClosedNodeLike for P9<T> {}
