use core::sync::atomic::{AtomicBool, Ordering};

use super::*;

pub struct Stream {
    pub(crate) outbound: Flow,
    pub(crate) local: SocketAddr,
    pub(crate) peer: SocketAddr,
    pub(crate) rx: Receiver,
    pub(crate) peer_rx_closed: AtomicBool,
    pub(crate) peer_tx_closed: AtomicBool,
    pub(crate) closed: AtomicBool,
}

impl Stream {
    pub async fn connect(local: SocketAddr, peer: SocketAddr) -> Result<Stream> {
        let outbound = Flow {
            src: local,
            dst: peer,
        };
        let rx = connect(outbound).await;
        let outbound = rx.inbound().reverse();

        let result = rx.recv().await?;
        let StreamEvent::Connect = result else {
            return Err(SocketError::ConnectionFailed);
        };
        Ok(Stream {
            outbound,
            local,
            peer,
            rx,
            peer_rx_closed: false.into(),
            peer_tx_closed: false.into(),
            closed: false.into(),
        })
    }

    pub async fn recv(&self) -> Result<Box<[u8]>> {
        loop {
            let result = self.rx.recv().await?;
            return match result {
                StreamEvent::Reset => Err(SocketError::ConnectionReset),
                StreamEvent::Connect => Err(SocketError::ConnectionReset),
                StreamEvent::Shutdown { rx, tx } => {
                    let rx = if rx {
                        self.peer_rx_closed.store(true, Ordering::SeqCst);
                        true
                    } else {
                        self.peer_rx_closed.load(Ordering::SeqCst)
                    };
                    let tx = if tx {
                        self.peer_tx_closed.store(true, Ordering::SeqCst);
                        true
                    } else {
                        self.peer_tx_closed.load(Ordering::SeqCst)
                    };
                    if rx && tx {
                        rst(self.outbound).await;
                    }
                    if tx {
                        Err(SocketError::ConnectionClosed)
                    } else {
                        continue;
                    }
                }
                StreamEvent::Data(items) => Ok(items),
            };
        }
    }

    pub async fn send(&self, buf: &[u8]) -> Result<usize> {
        if self.peer_rx_closed.load(Ordering::SeqCst) {
            Err(SocketError::ConnectionClosed)
        } else {
            send(self.outbound, buf).await;
            Ok(buf.len())
        }
    }

    async fn close_internal(&mut self) -> Result<()> {
        if self.closed.fetch_or(true, Ordering::SeqCst) {
            return Ok(());
        }
        shutdown(self.outbound, true, true).await;

        loop {
            let result = self.rx.recv().await?;
            if result == StreamEvent::Reset {
                return Ok(());
            };
        }
    }

    pub async fn close(mut self) -> Result<()> {
        self.close_internal().await
    }

    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    pub fn local(&self) -> SocketAddr {
        self.local
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if !self.closed.load(Ordering::SeqCst) {
            let _ = crate::rt::spawn_blocking(self.close_internal());
        }
    }
}
