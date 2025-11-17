use super::*;

pub struct StreamListener {
    addr: SocketAddr,
    rx: Listener,
}

impl StreamListener {
    pub async fn bind(cid: u64, port: u32) -> Result<StreamListener> {
        let rx = listen(SocketAddr { cid, port }).await;
        let addr = rx.addr();
        Ok(StreamListener { addr, rx })
    }

    pub async fn accept(&self) -> Result<Stream> {
        let local = self.addr;
        let peer = self.rx.listen().await?;
        let inbound = Flow {
            src: peer,
            dst: local,
        };
        let rx = accept(inbound).await;
        let outbound = Flow {
            src: local,
            dst: peer,
        };
        Ok(Stream::new(outbound, rx))
    }
}
