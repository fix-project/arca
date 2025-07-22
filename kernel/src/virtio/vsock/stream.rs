use super::*;

pub struct Stream {
    pub(crate) outbound: Flow,
    pub(crate) local: SocketAddr,
    pub(crate) peer: SocketAddr,
    pub(crate) rx: Receiver,
    pub(crate) peer_rx_closed: bool,
    pub(crate) peer_tx_closed: bool,
}

impl Stream {
    pub async fn connect(local: SocketAddr, peer: SocketAddr) -> Result<Stream> {
        let outbound = Flow {
            src: local,
            dst: peer,
        };
        let mut rx = connect(outbound).await;

        let result = rx.recv().await?;
        let StreamEvent::Connect = result else {
            return Err(SocketError::ConnectionFailed);
        };
        Ok(Stream {
            outbound,
            local,
            peer,
            rx,
            peer_rx_closed: false,
            peer_tx_closed: false,
        })
    }

    pub async fn read(&mut self) -> Result<Box<[u8]>> {
        loop {
            let result = self.rx.recv().await?;
            return match result {
                StreamEvent::Reset => Err(SocketError::ConnectionReset),
                StreamEvent::Connect => Err(SocketError::ConnectionReset),
                StreamEvent::Shutdown { rx, tx } => {
                    self.peer_rx_closed |= rx;
                    self.peer_tx_closed |= tx;
                    if self.peer_rx_closed && self.peer_tx_closed {
                        rst(self.outbound).await;
                    }
                    if self.peer_tx_closed {
                        Err(SocketError::ConnectionClosed)
                    } else {
                        continue;
                    }
                }
                StreamEvent::Data(items) => Ok(items),
            };
        }
    }

    pub async fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if self.peer_rx_closed {
            Err(SocketError::ConnectionClosed)
        } else {
            Ok(send(self.outbound, buf).await)
        }
    }

    pub async fn close(mut self) -> Result<()> {
        shutdown(self.outbound, true, true).await;
        let mut peer_closed = false;
        let mut acknowledged = false;
        while !peer_closed || !acknowledged {
            let result = self.rx.recv().await?;
            match result {
                StreamEvent::Reset => {
                    acknowledged = true;
                }
                StreamEvent::Connect => {
                    rst(self.outbound).await;
                    return Err(SocketError::ConnectionReset);
                }
                StreamEvent::Shutdown { rx, tx } => {
                    self.peer_rx_closed |= rx;
                    self.peer_tx_closed |= tx;
                    if self.peer_rx_closed && self.peer_tx_closed {
                        rst(self.outbound).await;
                        peer_closed = true;
                    }
                }
                StreamEvent::Data(_) => {
                    rst(self.outbound).await;
                }
            };
        }
        Ok(())
    }

    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    pub fn local(&self) -> SocketAddr {
        self.local
    }
}
