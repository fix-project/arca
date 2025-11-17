use core::sync::atomic::{AtomicBool, Ordering};

use alloc::collections::vec_deque::VecDeque;

use super::*;

pub struct Stream {
    outbound: Flow,
    rx: Receiver,
    peer_rx_closed: AtomicBool,
    peer_tx_closed: AtomicBool,
    closed: AtomicBool,
    last_read: SpinLock<Option<VecDeque<u8>>>,
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

    pub async fn connect(cid: u64, peer: impl TryInto<SocketAddr>) -> Result<Stream> {
        let local = SocketAddr { cid, port: 0 };
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
        if let Some(rest) = last_read.as_mut() {
            let n = core::cmp::min(bytes.len(), rest.len());
            for item in bytes.iter_mut().take(n) {
                *item = rest.pop_front().unwrap();
            }
            if rest.is_empty() {
                *last_read = None;
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
                    StreamEvent::Data { data } => {
                        let n = core::cmp::min(data.len(), bytes.len());
                        let mut rest: VecDeque<u8> = data.into();
                        for item in bytes.iter_mut().take(n) {
                            *item = rest.pop_front().unwrap();
                        }
                        if !rest.is_empty() {
                            *last_read = Some(rest);
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
        if !self.closed.load(Ordering::SeqCst) {
            let _ = crate::rt::spawn_blocking(self.close_internal());
        }
    }
}
