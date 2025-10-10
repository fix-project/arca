use super::*;

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum PacketOperation {
    Request = 1,
    Response = 2,
    Rst = 3,
    Shutdown = 4,
    ReadWrite = 5,
    CreditUpdate = 6,
    CreditRequest = 7,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Outgoing {
    Request,
    Response,
    Rst,
    Shutdown { rx: bool, tx: bool },
    Write(usize),
    CreditUpdate,
    CreditRequest,
}

impl From<Outgoing> for PacketOperation {
    fn from(value: Outgoing) -> Self {
        match value {
            Outgoing::Request => PacketOperation::Request,
            Outgoing::Response => PacketOperation::Response,
            Outgoing::Rst => PacketOperation::Rst,
            Outgoing::Shutdown { rx: _, tx: _ } => PacketOperation::Shutdown,
            Outgoing::Write(_) => PacketOperation::ReadWrite,
            Outgoing::CreditUpdate => PacketOperation::CreditUpdate,
            Outgoing::CreditRequest => PacketOperation::CreditRequest,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct OutgoingMessage {
    pub flow: Flow,
    pub message: Outgoing,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Incoming {
    Invalid(u16),
    Request,
    Response,
    Rst,
    Shutdown { rx: bool, tx: bool },
    Read(usize),
    CreditUpdate,
    CreditRequest,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct IncomingMessage {
    pub flow: Flow,
    pub message: Incoming,
}

impl From<Header> for IncomingMessage {
    fn from(value: Header) -> Self {
        let flow = Flow {
            src: SocketAddr {
                cid: value.src_cid,
                port: value.src_port,
            },
            dst: SocketAddr {
                cid: value.dst_cid,
                port: value.dst_port,
            },
        };
        let message = match value.op {
            1 => Incoming::Request,
            2 => Incoming::Response,
            3 => Incoming::Rst,
            4 => Incoming::Shutdown {
                rx: value.flags & 1 == 1,
                tx: value.flags & 2 == 2,
            },
            5 => Incoming::Read(value.len as usize),
            6 => Incoming::CreditUpdate,
            7 => Incoming::CreditRequest,
            x => Incoming::Invalid(x),
        };
        IncomingMessage { flow, message }
    }
}

impl From<OutgoingMessage> for Header {
    fn from(value: OutgoingMessage) -> Self {
        if value.flow.src.cid != 3 || value.flow.dst.cid != 2 {
            log::error!("trying to send outgoing message to {:?}", value.flow);
        }
        Header {
            src_cid: value.flow.src.cid,
            src_port: value.flow.src.port,
            dst_cid: value.flow.dst.cid,
            dst_port: value.flow.dst.port,
            len: if let Outgoing::Write(x) = value.message {
                x as u32
            } else {
                0
            },
            ptype: 1, // TODO: seqpacket support
            op: PacketOperation::from(value.message) as u16,
            flags: if let Outgoing::Shutdown { rx: true, tx: _ } = value.message {
                1
            } else {
                0
            } | if let Outgoing::Shutdown { rx: _, tx: true } = value.message {
                2
            } else {
                0
            },
            buf_alloc: 0,
            fwd_cnt: 0,
        }
    }
}
