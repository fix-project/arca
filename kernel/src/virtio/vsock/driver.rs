use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::collections::btree_map::BTreeMap;
use alloc::collections::vec_deque::VecDeque;
use alloc::sync::Arc;
use common::vhost::VSockMetadata;

use super::*;
use crate::rt;
use crate::virtio::virtqueue::*;

pub(crate) struct Driver {
    rx: VirtQueue,
    tx: VirtQueue,
    status: Status,
    listeners: RwLock<BTreeMap<SocketAddr, Weak<RwLock<ListenSocket>>>>,
    streams: RwLock<BTreeMap<Flow, Weak<RwLock<StreamSocket>>>>,
}

#[derive(Debug, Default)]
struct Status {
    peer_buf_alloc: AtomicUsize,
    peer_fwd_cnt: AtomicUsize,
    tx_cnt: AtomicUsize,
    buf_alloc: AtomicUsize,
    fwd_cnt: AtomicUsize,
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
            status: Status::default(),
            listeners: Default::default(),
            streams: Default::default(),
        });
        for _ in 0..len / 2 {
            crate::rt::spawn(this.clone().recv_task());
        }
        crate::rt::spawn(this.clone().poll_task());

        this
    }

    #[rt::profile]
    pub async fn listen(&self, addr: SocketAddr) -> Arc<RwLock<ListenSocket>> {
        let mut addr = addr;

        let mut listeners = self.listeners.write();

        if addr.port == 0 {
            let mut found = false;
            for port in 49152..=65535 {
                addr.port = port;
                if !listeners.contains_key(&addr) {
                    found = true;
                    break;
                }
            }
            if !found {
                panic!("out of ports");
            }
        }

        let socket = Arc::new(RwLock::new(ListenSocket {
            addr,
            waker: None,
            queue: VecDeque::new(),
        }));
        let weak = Arc::downgrade(&socket);
        if let Some(x) = listeners.get(&addr) {
            if x.strong_count() >= 1 {
                panic!("addr {addr:?} in use!");
            }
        }
        listeners.insert(addr, weak);
        core::mem::drop(listeners);
        socket
    }

    #[rt::profile]
    pub async fn accept(&self, flow: Flow) -> Arc<RwLock<StreamSocket>> {
        let mut streams = self.streams.write();
        assert!(!streams.contains_key(&flow.reverse()));
        let socket = Arc::new(RwLock::new(StreamSocket {
            outbound: flow,
            waker: None,
            queue: VecDeque::new(),
        }));
        streams.insert(flow.reverse(), Arc::downgrade(&socket));
        core::mem::drop(streams);
        self.response(flow).await;
        socket
    }

    #[rt::profile]
    pub async fn connect(&self, flow: Flow) -> Arc<RwLock<StreamSocket>> {
        let mut flow = flow;
        let mut streams = self.streams.write();

        if flow.src.port == 0 {
            let mut found = false;
            for port in 49152..=65535 {
                flow.src.port = port;
                if !streams.contains_key(&flow.reverse()) {
                    found = true;
                    break;
                }
            }
            if !found {
                panic!("out of ports");
            }
        }

        let socket = Arc::new(RwLock::new(StreamSocket {
            outbound: flow,
            waker: None,
            queue: VecDeque::new(),
        }));
        let weak = Arc::downgrade(&socket);
        if let Some(x) = streams.get(&flow.reverse()) {
            if x.strong_count() >= 1 {
                panic!("flow {flow:?} in use!");
            }
        }
        streams.insert(flow.reverse(), weak);
        core::mem::drop(streams);
        self.request(flow).await;
        socket
    }

    #[rt::profile]
    async fn send_message(&self, msg: OutgoingMessage, buffers: Option<&BufferChain<'_>>) -> usize {
        unsafe {
            log::debug!("-> {msg:?}");
            let mut header: Header = msg.into();
            let len = buffers.map(|x| x.size()).unwrap_or(0);
            let mut waiting = false;
            loop {
                let status = &self.status;
                let peer_buf_alloc = status.peer_buf_alloc.load(Ordering::SeqCst);
                let peer_fwd_count = status.peer_fwd_cnt.load(Ordering::SeqCst);
                let tx_cnt = status.tx_cnt.load(Ordering::SeqCst);

                let tx_free = peer_buf_alloc.wrapping_sub(tx_cnt.wrapping_sub(peer_fwd_count));
                if tx_free < len {
                    waiting = true;
                    log::info!("waiting for rx capacity");
                    crate::rt::wfi().await;
                    continue;
                }

                if waiting {
                    log::warn!("rx okay");
                }

                let buf_alloc = status.buf_alloc.load(Ordering::SeqCst);
                let fwd_count = status.fwd_cnt.load(Ordering::SeqCst);

                header.buf_alloc = buf_alloc as u32;
                header.fwd_cnt = fwd_count as u32;

                status.tx_cnt.fetch_sub(len, Ordering::SeqCst);

                let header_buf: &mut [u8; 44] = core::mem::transmute(&mut header);
                let buffers = BufferChain::cons(header_buf, buffers);
                return self.tx.send(&buffers).await;
            }
        }
    }

    #[rt::profile]
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

    #[rt::profile]
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

    #[rt::profile]
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

    #[rt::profile]
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

    #[rt::profile]
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

    #[rt::profile]
    async fn recv_task(self: Arc<Self>) {
        unsafe {
            loop {
                let mut payload_buf = vec![0; 65536];
                payload_buf.resize(65536, 0);
                let mut header = Header::default();
                let status = &self.status;
                let len = payload_buf.len();
                status.buf_alloc.fetch_add(len, Ordering::SeqCst);

                let header_buf: &mut [u8; 44] = core::mem::transmute(&mut header);
                let payload_chain = BufferChain::new_mut(&mut payload_buf);
                let chain = BufferChain::cons_mut(header_buf, Some(&payload_chain));

                let read = self.rx.send(&chain).await;

                status.buf_alloc.fetch_sub(len, Ordering::SeqCst);
                status.fwd_cnt.fetch_add(read - 44, Ordering::SeqCst);
                status
                    .peer_buf_alloc
                    .store(header.buf_alloc as usize, Ordering::SeqCst);
                status
                    .peer_fwd_cnt
                    .store(header.fwd_cnt as usize, Ordering::SeqCst);

                assert!(len >= 44);
                let incoming = header.into();
                log::debug!("<- {incoming:?}");
                let IncomingMessage { flow, message } = incoming;

                match message {
                    Incoming::Invalid(_) => self.rst(flow).await,
                    Incoming::Request => {
                        let listeners = self.listeners.read();
                        if let Some(socket) = listeners.get(&flow.dst).and_then(Weak::upgrade) {
                            core::mem::drop(listeners);
                            let mut socket = socket.write();
                            socket.queue.push_back(flow.reverse());
                            if let Some(waker) = socket.waker.take() {
                                core::mem::drop(socket);
                                waker.wake();
                            }
                        } else {
                            log::warn!("got incoming request {flow:?}, but no listener");
                            self.rst(flow.reverse()).await
                        };
                    }
                    Incoming::Response => {
                        let streams = self.streams.read();
                        if let Some(socket) = streams.get(&flow).and_then(Weak::upgrade) {
                            core::mem::drop(streams);
                            let mut socket = socket.write();
                            socket.queue.push_back(StreamEvent::Connect);
                            if let Some(waker) = socket.waker.take() {
                                core::mem::drop(socket);
                                waker.wake();
                            }
                        } else {
                            log::warn!("got response {flow:?}, but no stream");
                            self.rst(flow.reverse()).await
                        };
                    }
                    Incoming::Rst => {
                        let streams = self.streams.read();
                        if let Some(socket) = streams.get(&flow).and_then(Weak::upgrade) {
                            core::mem::drop(streams);
                            let mut socket = socket.write();
                            socket.queue.push_back(StreamEvent::Reset);
                            if let Some(waker) = socket.waker.take() {
                                core::mem::drop(socket);
                                waker.wake();
                            }
                        }
                    }
                    Incoming::Shutdown { rx, tx } => {
                        let streams = self.streams.read();
                        if let Some(socket) = streams.get(&flow).and_then(Weak::upgrade) {
                            core::mem::drop(streams);
                            let mut socket = socket.write();
                            socket.queue.push_back(StreamEvent::Shutdown { rx, tx });
                            if let Some(waker) = socket.waker.take() {
                                core::mem::drop(socket);
                                waker.wake();
                            }
                        } else {
                            // log::warn!("got shutdown for {flow:?}, but no stream");
                        };
                    }
                    Incoming::Read(len) => {
                        let streams = self.streams.read();
                        payload_buf.truncate(len);
                        if let Some(socket) = streams.get(&flow).and_then(Weak::upgrade) {
                            core::mem::drop(streams);
                            let mut socket = socket.write();
                            socket
                                .queue
                                .push_back(StreamEvent::Data { data: payload_buf });
                            if let Some(waker) = socket.waker.take() {
                                core::mem::drop(socket);
                                waker.wake();
                            }
                        } else {
                            log::warn!("got data for {flow:?}, but no stream");
                            self.rst(flow.reverse()).await
                        };
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
        self.listeners
            .read()
            .iter()
            .filter_map(|(k, v)| if v.weak_count() >= 1 { Some(*k) } else { None })
            .collect()
    }

    pub fn streams(&self) -> Vec<Flow> {
        self.streams
            .read()
            .iter()
            .filter_map(|(k, v)| if v.weak_count() >= 1 { Some(*k) } else { None })
            .collect()
    }
}

#[derive(Debug)]
pub enum StreamEvent {
    Reset,
    Connect,
    Shutdown { rx: bool, tx: bool },
    Data { data: Vec<u8> },
}
