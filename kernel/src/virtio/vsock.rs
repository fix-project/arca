use common::util::{channel::ChannelClosed, sorter::Sorter};
use hashbrown::HashMap;

use crate::{
    prelude::*,
    virtio::{ReceiveQueue, TransmitQueue, VirtQueue},
};

// TODO: this global lock is likely to cause deadlocks
pub(crate) static VSOCK_DRIVER: OnceLock<VSockDriver> = OnceLock::new();

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum PacketOperation {
    Request = 1,
    Response = 2,
    Rst = 3,
    Shutdown = 4,
    ReadWrite = 5,
    CreditUpdate = 6,
    CreditRequest = 7,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct Flow {
    src: SocketAddr,
    dst: SocketAddr,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[allow(unused)]
enum Outgoing<'a> {
    Request,
    Response,
    Rst,
    Shutdown { rx: bool, tx: bool },
    ReadWrite(&'a [u8]),
    CreditUpdate,
    CreditRequest,
}

impl From<Outgoing<'_>> for PacketOperation {
    fn from(value: Outgoing<'_>) -> Self {
        match value {
            Outgoing::Request => PacketOperation::Request,
            Outgoing::Response => PacketOperation::Response,
            Outgoing::Rst => PacketOperation::Rst,
            Outgoing::Shutdown { rx: _, tx: _ } => PacketOperation::Shutdown,
            Outgoing::ReadWrite(_) => PacketOperation::ReadWrite,
            Outgoing::CreditUpdate => PacketOperation::CreditUpdate,
            Outgoing::CreditRequest => PacketOperation::CreditRequest,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum Incoming {
    Invalid(u16),
    Request,
    Response,
    Rst,
    Shutdown { rx: bool, tx: bool },
    Read(Box<[u8]>),
    CreditUpdate,
    CreditRequest,
}

async fn send(flow: Flow, msg: Outgoing<'_>) {
    let driver = &VSOCK_DRIVER;
    driver.send(flow, msg).await
}

async fn bind(addr: SocketAddr) -> Listener {
    let driver = &VSOCK_DRIVER;
    driver.bind(addr).await
}

async fn connect(flow: Flow) -> Receiver {
    let driver = &VSOCK_DRIVER;
    driver.connect(flow).await
}

pub(crate) fn tick() {
    if let Some(driver) = OnceLock::get(&VSOCK_DRIVER) {
        driver.tick();
    }
}

#[derive(Debug)]
pub(crate) struct VSockDriver {
    outbound_tx: channel::Sender<(*mut Header, *const [u8])>,
    outbound_rx: channel::Receiver<(*mut Header, *const [u8])>,
    inbound_connections: Sorter<SocketAddr, (SocketAddr, Receiver)>,
    inbound_packets: Sorter<Flow, Incoming>,
    outbound_waker: SpinLock<HashMap<usize, oneshot::Sender<()>>>,
    rx: ReceiveQueue,
    tx: TransmitQueue,
}

unsafe impl Send for VSockDriver {}
unsafe impl Sync for VSockDriver {}

impl VSockDriver {
    pub unsafe fn new(value: common::vhost::VSockMetadata) -> Self {
        let (outbound_tx, outbound_rx) = channel::unbounded();
        let rx = ReceiveQueue::new(VirtQueue::new(value.rx));
        let tx = TransmitQueue::new(VirtQueue::new(value.tx));
        Self {
            outbound_tx,
            outbound_rx,
            inbound_connections: Sorter::new(),
            inbound_packets: Sorter::new(),
            outbound_waker: SpinLock::new(HashMap::new()),
            rx,
            tx,
        }
    }

    async fn send(&self, flow: Flow, msg: Outgoing<'_>) {
        let buf = if let Outgoing::ReadWrite(buf) = msg {
            Some(buf)
        } else {
            None
        };
        let mut header = Header {
            src_cid: flow.src.cid,
            src_port: flow.src.port,
            dst_cid: flow.dst.cid,
            dst_port: flow.dst.port,
            len: buf.map(|b| b.len() as u32).unwrap_or(0),
            ptype: 1, // TODO: support seqpacket
            op: PacketOperation::from(msg) as u16,
            flags: if let Outgoing::Shutdown { rx: true, tx: _ } = msg {
                1
            } else {
                0
            } | if let Outgoing::Shutdown { rx: _, tx: true } = msg {
                2
            } else {
                0
            },
            buf_alloc: 0, // filled in later
            fwd_cnt: 0,   // filled in later
        };
        if let Some(buf) = buf {
            let (tx, rx) = oneshot::channel();
            self.outbound_waker.with(|wakers| {
                wakers.insert(buf.as_ptr() as usize, tx);
            });
            self.outbound_tx.send_blocking((&mut header, buf)).unwrap();
            rx.await;
        } else {
            let (tx, rx) = oneshot::channel();
            self.outbound_waker.with(|wakers| {
                wakers.insert(&raw const header as usize, tx);
            });
            self.outbound_tx.send_blocking((&mut header, &[])).unwrap();
            rx.await;
        }
    }

    async fn bind(&self, addr: SocketAddr) -> Listener {
        log::debug!("listening on {addr:?}");
        Listener {
            rx: self.inbound_connections.receiver(addr),
        }
    }

    async fn connect(&self, flow: Flow) -> Receiver {
        Receiver {
            rx: self.inbound_packets.receiver(flow),
        }
    }

    fn tick(&self) {
        self.release_buffers();
        self.send_pending();
        self.handle_incoming();
    }

    fn handle_one(&self) -> Option<(Flow, Incoming)> {
        let buf = self.rx.try_recv()?;
        assert!(buf.len() >= core::mem::size_of::<Header>());
        let (flow, incoming) = unsafe {
            let p: *const u8 = buf.as_ptr();
            let header: *const Header = core::mem::transmute(p);
            let header = header.read();
            let start = core::mem::size_of::<Header>();
            let end = start + header.len as usize;
            if header.ptype != 1 {
                todo!("handle seqpacket");
            }
            let flow = Flow {
                src: SocketAddr {
                    cid: header.src_cid,
                    port: header.src_port,
                },
                dst: SocketAddr {
                    cid: header.dst_cid,
                    port: header.dst_port,
                },
            };
            let incoming = match header.op {
                1 => Incoming::Request,
                2 => Incoming::Response,
                3 => Incoming::Rst,
                4 => Incoming::Shutdown {
                    rx: header.flags & 1 == 1,
                    tx: header.flags & 2 == 2,
                },
                // TODO: can we avoid this copy?
                5 => Incoming::Read(buf[start..end].to_vec().into()),
                6 => Incoming::CreditUpdate,
                7 => Incoming::CreditRequest,
                x => Incoming::Invalid(x),
            };
            (flow, incoming)
        };
        Some((flow, incoming))
    }

    fn send_pending(&self) {
        while let Some(x) = self.outbound_rx.try_recv() {
            let (header, buf) = x.unwrap();
            unsafe {
                let header = &mut *header;
                self.rx.q.with(|q| {
                    header.buf_alloc = q.buf_alloc() as u32;
                    header.fwd_cnt = q.fwd_cnt() as u32;
                });
                let header: *const [u8] = core::slice::from_raw_parts(
                    core::mem::transmute(header),
                    core::mem::size_of::<Header>(),
                );
                if buf.is_empty() {
                    self.tx.send(&[header])
                } else {
                    self.tx.send(&[header, buf])
                }
            }
        }
    }

    fn release_buffers(&self) {
        self.outbound_waker.with(|wakers| {
            self.tx.get_used(|ptr| {
                if let Some(waker) = wakers.remove(&(ptr.as_ptr() as usize)) {
                    waker(())
                }
            });
        });
    }

    fn handle_incoming(&self) {
        // TODO: reject bad packets with RST
        while let Some((flow, incoming)) = self.handle_one() {
            log::debug!("handling {incoming:?} for {flow:?}");
            match incoming {
                Incoming::Request => {
                    let rx = self.inbound_packets.receiver(flow);
                    let rx = Receiver { rx };
                    self.inbound_connections
                        .sender()
                        .send_blocking(flow.dst, (flow.src, rx))
                        .unwrap();
                }
                _ => {
                    let _ = self.inbound_packets.sender().send_blocking(flow, incoming);
                }
            }
        }
    }
}

#[derive(Debug)]
struct Listener {
    rx: sorter::Receiver<SocketAddr, (SocketAddr, Receiver)>,
}

impl Listener {
    pub async fn listen(&mut self) -> Result<(SocketAddr, Receiver)> {
        Ok(self.rx.recv().await?)
    }
}

#[derive(Debug)]
struct Receiver {
    rx: sorter::Receiver<Flow, Incoming>,
}

impl Receiver {
    pub async fn recv(&mut self) -> Result<Incoming> {
        Ok(self.rx.recv().await?)
    }
}

#[derive(Debug)]
pub enum Error {
    InvalidAddress(SocketAddr),
    AddressInUse(SocketAddr),
    ConnectionReset,
    ConnectionClosed,
}

impl From<ChannelClosed> for Error {
    fn from(_: ChannelClosed) -> Self {
        Error::ConnectionClosed
    }
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct SocketAddr {
    cid: u64,
    port: u32,
}

impl SocketAddr {
    pub fn new(cid: u64, port: u32) -> SocketAddr {
        Self { cid, port }
    }
}

pub struct StreamListener {
    addr: SocketAddr,
    rx: Listener,
}

impl StreamListener {
    pub async fn bind(addr: SocketAddr) -> Result<StreamListener> {
        let rx = bind(addr).await;
        Ok(StreamListener { addr, rx })
    }

    pub async fn accept(&mut self) -> Result<Stream> {
        let local = self.addr;
        let (peer, rx) = self.rx.listen().await?;
        let outbound = Flow {
            src: local,
            dst: peer,
        };
        send(outbound, Outgoing::Response).await;
        Ok(Stream {
            outbound,
            local,
            peer,
            rx,
        })
    }
}

pub struct Stream {
    outbound: Flow,
    local: SocketAddr,
    peer: SocketAddr,
    rx: Receiver,
}

impl Stream {
    pub async fn connect(local: SocketAddr, peer: SocketAddr) -> Result<Stream> {
        let inbound = Flow {
            src: peer,
            dst: local,
        };
        let outbound = Flow {
            src: local,
            dst: peer,
        };
        let mut rx = connect(inbound).await;

        send(outbound, Outgoing::Request).await;
        let result = rx.recv().await?;
        let Incoming::Response = result else {
            panic!("connection failed!");
        };
        Ok(Stream {
            outbound,
            local,
            peer,
            rx,
        })
    }

    pub async fn read(&mut self) -> Result<Box<[u8]>> {
        loop {
            let result = self.rx.recv().await?;
            match result {
                Incoming::Invalid(_) => continue,
                Incoming::Request => todo!(),
                Incoming::Response => todo!(),
                Incoming::Rst => return Err(Error::ConnectionReset),
                Incoming::Shutdown { rx: _, tx: true } => return Err(Error::ConnectionClosed),
                Incoming::Shutdown { rx: _, tx: _ } => continue,
                Incoming::Read(items) => {
                    return Ok(items);
                }
                Incoming::CreditUpdate => {
                    send(self.outbound, Outgoing::CreditUpdate).await;
                    continue;
                }
                Incoming::CreditRequest => todo!(),
            }
        }
    }

    pub async fn write(&mut self, buf: &[u8]) {
        send(self.outbound, Outgoing::ReadWrite(buf)).await
    }

    pub async fn close(self) {
        send(self.outbound, Outgoing::Shutdown { rx: true, tx: true }).await
    }

    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    pub fn local(&self) -> SocketAddr {
        self.local
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
struct Header {
    src_cid: u64,
    dst_cid: u64,
    src_port: u32,
    dst_port: u32,
    len: u32,
    ptype: u16,
    op: u16,
    flags: u32,
    buf_alloc: u32,
    fwd_cnt: u32,
}

const _: () = const {
    assert!(core::mem::size_of::<Header>() == 44);
};
