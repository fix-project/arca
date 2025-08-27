use alloc::sync::Arc;
use common::util::sorter::Sorter;
use common::vhost::VSockMetadata;

use super::*;
use crate::virtio::virtqueue::*;

#[derive(Debug)]
pub struct Driver {
    rx: VirtQueue,
    tx: VirtQueue,
    status: SpinLock<Status>,
    listeners: sorter::Sorter<SocketAddr, SocketAddr>,
    streams: sorter::Sorter<Flow, StreamEvent>,
}

#[derive(Debug, Default)]
struct Status {
    peer_buf_alloc: usize,
    peer_fwd_cnt: usize,
    tx_cnt: usize,
    buf_alloc: usize,
    fwd_cnt: usize,
}

impl Driver {
    /// # Safety
    ///
    /// `info` must describe a valid VSock, which has an attached device but no driver.  Feature
    /// negotiation must have completed and match the features available in this driver.
    pub unsafe fn new(info: VSockMetadata) -> Arc<Self> {
        let len = info.rx.descriptors;
        let rx = VirtQueue::new("rx", info.rx);
        let tx = VirtQueue::new("tx", info.tx);
        let this = Arc::new(Self {
            rx,
            tx,
            status: SpinLock::new(Status::default()),
            listeners: Sorter::new(),
            streams: Sorter::new(),
        });
        for _ in 0..len / 2 {
            crate::rt::spawn(this.clone().recv_task());
        }
        crate::rt::spawn(this.clone().poll_task());

        this
    }

    pub async fn listen(&self, addr: SocketAddr) -> Listener {
        if addr.port == 0 {
            for port in 49152..=65535 {
                let addr = SocketAddr {
                    cid: addr.cid,
                    port,
                };
                if let Some(rx) = self.listeners.try_receiver(addr) {
                    return Listener { rx };
                }
            }
            panic!("out of ports");
        } else {
            Listener {
                rx: self.listeners.receiver(addr),
            }
        }
    }

    pub async fn accept(&self, flow: Flow) -> Receiver {
        let rx = self.streams.receiver(flow);
        self.response(flow.reverse()).await;
        Receiver { rx }
    }

    pub async fn connect(&self, flow: Flow) -> Receiver {
        if flow.src.port == 0 {
            for port in 49152..=65535 {
                let src = SocketAddr {
                    cid: flow.src.cid,
                    port,
                };
                let flow = Flow { src, dst: flow.dst };
                if let Some(rx) = self.streams.try_receiver(flow.reverse()) {
                    self.request(flow).await;
                    return Receiver { rx };
                }
            }
            panic!("out of ports");
        }
        let rx = self.streams.receiver(flow.reverse());
        self.request(flow).await;
        Receiver { rx }
    }

    async fn send_message(&self, msg: OutgoingMessage, buffers: Option<&BufferChain<'_>>) -> usize {
        unsafe {
            log::debug!("-> {msg:?}");
            let mut header: Header = msg.into();
            let len = buffers.map(|x| x.size()).unwrap_or(0);
            let mut waiting = false;
            loop {
                let mut status = self.status.lock();

                let tx_free = status
                    .peer_buf_alloc
                    .wrapping_sub(status.tx_cnt.wrapping_sub(status.peer_fwd_cnt));
                if tx_free < len {
                    SpinLock::unlock(status);
                    waiting = true;
                    log::info!("waiting for rx capacity");
                    crate::rt::wfi().await;
                    continue;
                }

                if waiting {
                    log::warn!("rx okay");
                }

                header.buf_alloc = status.buf_alloc as u32;
                header.fwd_cnt = status.fwd_cnt as u32;
                status.tx_cnt = status.tx_cnt.wrapping_add(len);

                let header_buf: &mut [u8; 44] = core::mem::transmute(&mut header);
                let buffers = BufferChain::cons(header_buf, buffers);
                SpinLock::unlock(status);
                return self.tx.send(&buffers).await;
            }
        }
    }

    pub async fn rst(&self, flow: Flow) {
        self.send_message(
            OutgoingMessage {
                flow,
                message: Outgoing::Rst,
            },
            None,
        )
        .await;
    }

    pub async fn response(&self, flow: Flow) {
        self.send_message(
            OutgoingMessage {
                flow,
                message: Outgoing::Response,
            },
            None,
        )
        .await;
    }

    pub async fn request(&self, flow: Flow) {
        self.send_message(
            OutgoingMessage {
                flow,
                message: Outgoing::Request,
            },
            None,
        )
        .await;
    }

    pub async fn send(&self, flow: Flow, buf: &[u8]) -> usize {
        let chain = BufferChain::new(buf);
        self.send_message(
            OutgoingMessage {
                flow,
                message: Outgoing::Write(buf.len()),
            },
            Some(&chain),
        )
        .await
    }

    pub async fn shutdown(&self, flow: Flow, rx: bool, tx: bool) {
        self.send_message(
            OutgoingMessage {
                flow,
                message: Outgoing::Shutdown { rx, tx },
            },
            None,
        )
        .await;
    }

    pub async fn update(&self, flow: Flow) {
        self.send_message(
            OutgoingMessage {
                flow,
                message: Outgoing::CreditUpdate,
            },
            None,
        )
        .await;
    }

    async fn recv_task(self: Arc<Self>) {
        let mut listeners = self.listeners.sender();
        let mut streams = self.streams.sender();
        unsafe {
            loop {
                let mut payload_buf = vec![0; 65536];
                payload_buf.resize(65536, 0);
                let mut header = Header::default();
                let mut status = self.status.lock();
                let len = payload_buf.len();
                status.buf_alloc += len;

                let header_buf: &mut [u8; 44] = core::mem::transmute(&mut header);
                let payload_chain = BufferChain::new_mut(&mut payload_buf);
                let chain = BufferChain::cons_mut(header_buf, Some(&payload_chain));
                SpinLock::unlock(status);

                let read = self.rx.send(&chain).await;

                let mut status = self.status.lock();
                status.buf_alloc -= len;
                status.fwd_cnt += read - 44;
                status.peer_buf_alloc = header.buf_alloc as usize;
                status.peer_fwd_cnt = header.fwd_cnt as usize;
                SpinLock::unlock(status);

                assert!(len >= 44);
                let incoming = header.into();
                log::debug!("<- {incoming:?}");
                let IncomingMessage { flow, message } = incoming;

                match message {
                    Incoming::Invalid(_) => self.rst(flow).await,
                    Incoming::Request => {
                        if listeners.send_blocking(flow.dst, flow.src).is_err() {
                            log::warn!("got incoming request {flow:?}, but no listener");
                            self.rst(flow.reverse()).await
                        }
                    }
                    Incoming::Response => {
                        if streams.send_blocking(flow, StreamEvent::Connect).is_err() {
                            log::error!("got response {flow:?}, but no stream");
                            self.rst(flow.reverse()).await
                        }
                    }
                    Incoming::Rst => {
                        let _ = streams.send_blocking(flow, StreamEvent::Reset).is_ok();
                    }
                    Incoming::Shutdown { rx, tx } => {
                        if streams
                            .send_blocking(flow, StreamEvent::Shutdown { rx, tx })
                            .is_err()
                        {
                            // TODO: why are we receiving these?
                            self.rst(flow.reverse()).await;
                        }
                    }
                    Incoming::Read(len) => {
                        payload_buf.truncate(len);
                        if streams
                            .send_blocking(flow, StreamEvent::Data { data: payload_buf })
                            .is_err()
                        {
                            self.rst(flow.reverse()).await
                        }
                    }
                    Incoming::CreditUpdate => {}
                    Incoming::CreditRequest => self.update(flow.reverse()).await,
                }
            }
        }
    }

    async fn poll_task(self: Arc<Self>) {
        loop {
            self.poll();
            crate::rt::wfi().await;
        }
    }

    pub fn poll(&self) {
        self.rx.poll();
        self.tx.poll();
    }

    pub fn listeners(&self) -> Vec<SocketAddr> {
        self.listeners.keys()
    }

    pub fn streams(&self) -> Vec<Flow> {
        self.streams.keys()
    }
}

#[derive(Debug)]
pub struct Listener {
    rx: sorter::Receiver<SocketAddr, SocketAddr>,
}

impl Listener {
    pub async fn listen(&self) -> Result<SocketAddr> {
        Ok(self.rx.recv().await?)
    }

    pub fn addr(&self) -> SocketAddr {
        *self.rx.key()
    }
}

#[derive(Debug)]
pub struct Receiver {
    rx: sorter::Receiver<Flow, StreamEvent>,
}

impl Receiver {
    pub async fn recv(&self) -> Result<StreamEvent> {
        Ok(self.rx.recv().await?)
    }

    pub fn inbound(&self) -> Flow {
        *self.rx.key()
    }
}

#[derive(Debug)]
pub enum StreamEvent {
    Reset,
    Connect,
    Shutdown { rx: bool, tx: bool },
    Data { data: Vec<u8> },
}
