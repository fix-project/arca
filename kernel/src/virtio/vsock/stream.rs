use core::sync::atomic::{AtomicBool, Ordering};

use super::*;

type PartiallyReadVec = (Vec<u8>, usize, channel::Sender<Vec<u8>>);

pub struct Stream {
    outbound: Flow,
    rx: Receiver,
    peer_rx_closed: AtomicBool,
    peer_tx_closed: AtomicBool,
    closed: AtomicBool,
    last_read: SpinLock<Option<PartiallyReadVec>>,
}

impl Stream {
    pub fn new(outbound: Flow, rx: Receiver) -> Stream {
        Stream {
            outbound,
            rx,
            peer_rx_closed: AtomicBool::new(false),
            peer_tx_closed: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            last_read: SpinLock::new(None),
        }
    }

    pub async fn connect(peer: impl TryInto<SocketAddr>) -> Result<Stream> {
        let local = SocketAddr { cid: 3, port: 0 };
        let outbound = Flow {
            src: local,
            dst: peer.try_into().map_err(|_| SocketError::InvalidAddress)?,
        };
        let rx = connect(outbound).await;
        let outbound = rx.inbound().reverse();

        let result = rx.recv().await?;
        let StreamEvent::Connect = result else {
            return Err(SocketError::ConnectionFailed);
        };
        Ok(Stream {
            outbound,
            rx,
            peer_rx_closed: false.into(),
            peer_tx_closed: false.into(),
            closed: false.into(),
            last_read: SpinLock::new(None),
        })
    }

    pub async fn recv(&self, bytes: &mut [u8]) -> Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let mut last_read = self.last_read.lock();
        if let Some((buf, mut offset, release)) = last_read.take() {
            assert!(offset <= buf.len());
            let rest = &buf[offset..];
            let n = rest.len();
            bytes[..rest.len()].copy_from_slice(rest);
            offset += rest.len();
            if offset >= buf.len() {
                *last_read = None;
                release.send_blocking(buf).unwrap();
            } else {
                *last_read = Some((buf, offset, release));
            }
            Ok(n)
        } else {
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
                    StreamEvent::Data { data, release } => {
                        let n = core::cmp::min(data.len(), bytes.len());
                        bytes[..n].copy_from_slice(&data[..n]);
                        if n < data.len() {
                            *last_read = Some((data, n, release));
                        }
                        return Ok(n);
                    }
                };
            }
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
            if let StreamEvent::Reset = result {
                return Ok(());
            };
        }
    }

    pub async fn close(mut self) -> Result<()> {
        self.close_internal().await
    }

    pub fn peer(&self) -> SocketAddr {
        self.outbound.dst
    }

    pub fn local(&self) -> SocketAddr {
        self.outbound.src
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if let Some((v, _, ret)) = self.last_read.lock().take() {
            ret.send_blocking(v).unwrap();
        }
        if !self.closed.load(Ordering::SeqCst) {
            let _ = crate::rt::spawn_blocking(self.close_internal());
        }
    }
}
