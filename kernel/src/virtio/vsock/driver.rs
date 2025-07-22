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
    tx_buf_alloc: usize,
    tx_fwd_cnt: usize,
    tx_cnt: usize,
    rx_buf_alloc: usize,
    rx_fwd_cnt: usize,
}

impl Driver {
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
        Listener {
            rx: self.listeners.receiver(addr),
        }
    }

    pub async fn accept(&self, flow: Flow) -> Receiver {
        let rx = self.streams.receiver(flow);
        self.response(flow.reverse()).await;
        Receiver { rx }
    }

    pub async fn connect(&self, flow: Flow) -> Receiver {
        let rx = self.streams.receiver(flow.reverse());
        self.request(flow).await;
        Receiver { rx }
    }

    async fn send_message(&self, msg: OutgoingMessage, buffers: Option<&BufferChain<'_>>) -> usize {
        unsafe {
            log::debug!("-> {msg:?}");
            let mut header: Header = msg.into();
            let len = buffers.map(|x| x.len()).unwrap_or(0);
            loop {
                let mut status = self.status.lock();

                // TODO: we probably should check if the other end can receive
                // let tx_free = status
                //     .tx_buf_alloc
                //     .wrapping_sub(status.tx_cnt.wrapping_sub(status.tx_fwd_cnt));
                // if tx_free <= len {
                //     log::error!("not enough space to transmit ({tx_free} free, need {len})! yielding for now");
                //     SpinLock::unlock(status);
                //     crate::rt::yield_now().await;
                //     continue;
                // }
                if len > 0 && status.rx_buf_alloc == 0 {
                    SpinLock::unlock(status);
                    crate::rt::yield_now().await;
                    continue;
                }

                header.buf_alloc = status.rx_buf_alloc as u32;
                header.fwd_cnt = status.rx_fwd_cnt as u32;
                status.tx_cnt = status.tx_cnt.wrapping_add(len);

                // TODO: this is definitely wrong
                header.buf_alloc = 4096;
                header.fwd_cnt = 0;

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
            let mut payload_buf: Box<[u8]> = Box::new_zeroed_slice(4096).assume_init();
            loop {
                let mut header = Header::default();
                let mut status = self.status.lock();
                let len = payload_buf.len();
                status.rx_buf_alloc += len;

                let header_buf: &mut [u8; 44] = core::mem::transmute(&mut header);
                let payload_chain = BufferChain::new_mut(&mut payload_buf);
                let chain = BufferChain::cons_mut(header_buf, Some(&payload_chain));
                SpinLock::unlock(status);

                let read = self.rx.send(&chain).await;

                let mut status = self.status.lock();
                status.rx_buf_alloc -= len;
                status.rx_fwd_cnt += read - 44;
                status.tx_buf_alloc = header.buf_alloc as usize;
                status.tx_fwd_cnt = header.fwd_cnt as usize;
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
                            log::error!("got response {:?}, but no stream", flow);
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
                            log::error!("got shutdown {:?}, but no stream", flow);
                            self.rst(flow.reverse()).await;
                        }
                    }
                    Incoming::Read(len) => {
                        let mut buf = Box::new_uninit_slice(4096).assume_init();
                        core::mem::swap(&mut buf, &mut payload_buf);
                        let mut v = buf.into_vec();
                        v.truncate(len);
                        let b = v.into_boxed_slice();
                        if streams.send_blocking(flow, StreamEvent::Data(b)).is_err() {
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
            let _ = self.try_poll();
            crate::rt::yield_now().await;
        }
    }

    pub fn try_poll(&self) -> Option<()> {
        self.rx.try_poll();
        self.tx.try_poll();
        Some(())
    }
}

#[derive(Debug)]
pub struct Listener {
    rx: sorter::Receiver<SocketAddr, SocketAddr>,
}

impl Listener {
    pub async fn listen(&mut self) -> Result<SocketAddr> {
        Ok(self.rx.recv().await?)
    }
}

#[derive(Debug)]
pub struct Receiver {
    rx: sorter::Receiver<Flow, StreamEvent>,
}

impl Receiver {
    pub async fn recv(&mut self) -> Result<StreamEvent> {
        Ok(self.rx.recv().await?)
    }
}

#[derive(Debug)]
pub enum StreamEvent {
    Reset,
    Connect,
    Shutdown { rx: bool, tx: bool },
    Data(Box<[u8]>),
}
