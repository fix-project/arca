extern crate alloc;

use crate::ringbuffer::{
    RingBufferEndPoint, RingBufferError, RingBufferReceiver, RingBufferSender,
};

#[derive(Clone)]
#[repr(u8)]
enum OpCode {
    CreateBlob = 0,
    CreateTree = 1,
    CreateThunk = 2,
    RunThunk = 3,
    Apply = 4,
    Reply = 5,
    Drop = 6,
}

impl TryFrom<u8> for OpCode {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            _ if value == OpCode::CreateBlob as u8 => Ok(OpCode::CreateBlob),
            _ if value == OpCode::CreateTree as u8 => Ok(OpCode::CreateTree),
            _ if value == OpCode::CreateThunk as u8 => Ok(OpCode::CreateThunk),
            _ if value == OpCode::RunThunk as u8 => Ok(OpCode::RunThunk),
            _ if value == OpCode::Apply as u8 => Ok(OpCode::Apply),
            _ if value == OpCode::Reply as u8 => Ok(OpCode::Reply),
            _ if value == OpCode::Drop as u8 => Ok(OpCode::Drop),
            _ => Err("Invalid raw u8 for OpCode"),
        }
    }
}

pub struct BlobHandle(usize);
pub struct TreeHandle(usize);
pub struct LambdaHandle(usize);
pub struct ThunkHandle(usize);

impl BlobHandle {
    pub fn new(s: usize) -> Self {
        Self(s)
    }
}

impl TreeHandle {
    pub fn new(s: usize) -> Self {
        Self(s)
    }
}

impl LambdaHandle {
    pub fn new(s: usize) -> Self {
        Self(s)
    }
}

impl ThunkHandle {
    pub fn new(s: usize) -> Self {
        Self(s)
    }
}

#[repr(u8)]
enum HandleType {
    Blob = 0,
    Tree = 1,
    Thunk = 2,
    Lambda = 3,
}

impl TryFrom<u8> for HandleType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            _ if value == HandleType::Blob as u8 => Ok(HandleType::Blob),
            _ if value == HandleType::Tree as u8 => Ok(HandleType::Tree),
            _ if value == HandleType::Thunk as u8 => Ok(HandleType::Thunk),
            _ if value == HandleType::Lambda as u8 => Ok(HandleType::Lambda),
            _ => Err("Invalid raw u8 for HandleType"),
        }
    }
}

pub enum ArcaHandle {
    BlobHandle(BlobHandle),
    TreeHandle(TreeHandle),
    LambdaHandle(LambdaHandle),
    ThunkHandle(ThunkHandle),
}

impl ArcaHandle {
    fn parse(buf: &[u8; 9]) -> Self {
        let handletype = HandleType::try_from(buf[0]).expect("");
        let offset = usize::from_ne_bytes(buf[1..9].try_into().unwrap());
        match handletype {
            HandleType::Blob => Self::BlobHandle(BlobHandle(offset)),
            HandleType::Tree => Self::TreeHandle(TreeHandle(offset)),
            HandleType::Lambda => Self::LambdaHandle(LambdaHandle(offset)),
            HandleType::Thunk => Self::ThunkHandle(ThunkHandle(offset)),
        }
    }

    fn to_offset_internal(self) -> usize {
        match self {
            ArcaHandle::BlobHandle(BlobHandle(h))
            | ArcaHandle::TreeHandle(TreeHandle(h))
            | ArcaHandle::LambdaHandle(LambdaHandle(h))
            | ArcaHandle::ThunkHandle(ThunkHandle(h)) => h,
        }
    }

    fn serialize(self) -> [u8; 9] {
        let mut result: [u8; 9] = [0; 9];
        result[0] = match &self {
            ArcaHandle::BlobHandle(_) => HandleType::Blob as u8,
            ArcaHandle::TreeHandle(_) => HandleType::Tree as u8,
            ArcaHandle::LambdaHandle(_) => HandleType::Lambda as u8,
            ArcaHandle::ThunkHandle(_) => HandleType::Thunk as u8,
        };

        let sub: &mut [u8; 8] = (&mut result[1..9]).try_into().unwrap();
        *sub = self.to_offset_internal().to_ne_bytes();

        result
    }

    pub fn to_offset<T: Into<ArcaHandle>>(x: T) -> usize {
        let x: ArcaHandle = x.into();
        x.to_offset_internal()
    }
}

impl From<BlobHandle> for ArcaHandle {
    fn from(value: BlobHandle) -> ArcaHandle {
        ArcaHandle::BlobHandle(value)
    }
}

impl From<TreeHandle> for ArcaHandle {
    fn from(value: TreeHandle) -> ArcaHandle {
        ArcaHandle::TreeHandle(value)
    }
}

impl From<LambdaHandle> for ArcaHandle {
    fn from(value: LambdaHandle) -> ArcaHandle {
        ArcaHandle::LambdaHandle(value)
    }
}

impl From<ThunkHandle> for ArcaHandle {
    fn from(value: ThunkHandle) -> ArcaHandle {
        ArcaHandle::ThunkHandle(value)
    }
}

pub enum Message {
    CreateBlobMessage {
        ptr: usize,
        size: usize,
    },
    CreateTreeMessage {
        ptr: usize,
        size: usize,
    },
    CreateThunkMessage {
        handle: BlobHandle,
    },
    RunThunkMessage {
        handle: ThunkHandle,
    },
    ApplyMessage {
        lambda_handle: LambdaHandle,
        arg_handle: ArcaHandle,
    },
    ReplyMessage {
        handle: ArcaHandle,
    },
    DropMessage {
        handle: ArcaHandle,
    },
}

impl Message {
    fn opcode(&self) -> OpCode {
        match &self {
            Message::CreateBlobMessage { .. } => OpCode::CreateBlob,
            Message::CreateTreeMessage { .. } => OpCode::CreateTree,
            Message::CreateThunkMessage { .. } => OpCode::CreateThunk,
            Message::RunThunkMessage { .. } => OpCode::RunThunk,
            Message::ApplyMessage { .. } => OpCode::Apply,
            Message::ReplyMessage { .. } => OpCode::Reply,
            Message::DropMessage { .. } => OpCode::Drop,
        }
    }

    fn parse(op: &OpCode, buf: &[u8; 18]) -> Option<Message> {
        match op {
            OpCode::CreateBlob | OpCode::CreateTree => {
                let left: &[u8; 8] = buf[0..8].try_into().expect("split slice with wrong length");
                let right: &[u8; 8] = buf[8..16]
                    .try_into()
                    .expect("split slice with wrong length");
                let left = usize::from_ne_bytes(*left);
                let right = usize::from_ne_bytes(*right);

                match op {
                    OpCode::CreateBlob => Some(Message::CreateBlobMessage {
                        ptr: left,
                        size: right,
                    }),
                    OpCode::CreateTree => Some(Message::CreateTreeMessage {
                        ptr: left,
                        size: right,
                    }),
                    _ => unreachable!(),
                }
            }
            _ => {
                let left: &[u8; 9] = buf[0..9].try_into().expect("split slice with wrong length");
                let left = ArcaHandle::parse(left);

                match op {
                    OpCode::CreateThunk => {
                        if let ArcaHandle::BlobHandle(b) = left {
                            Some(Message::CreateThunkMessage { handle: b })
                        } else {
                            None
                        }
                    }
                    OpCode::RunThunk => {
                        if let ArcaHandle::ThunkHandle(t) = left {
                            Some(Message::RunThunkMessage { handle: t })
                        } else {
                            None
                        }
                    }
                    OpCode::Apply => {
                        if let ArcaHandle::LambdaHandle(l) = left {
                            let right: &[u8; 9] = buf[9..18]
                                .try_into()
                                .expect("split slice with wrong length");
                            let right = ArcaHandle::parse(right);
                            Some(Message::ApplyMessage {
                                lambda_handle: l,
                                arg_handle: right,
                            })
                        } else {
                            None
                        }
                    }
                    OpCode::Reply => Some(Message::ReplyMessage { handle: left }),
                    OpCode::Drop => Some(Message::DropMessage { handle: left }),
                    _ => unreachable!(),
                }
            }
        }
    }

    fn msg_size(op: &OpCode) -> usize {
        match op {
            OpCode::CreateBlob => 16,
            OpCode::CreateTree => 16,
            OpCode::CreateThunk => 9,
            OpCode::RunThunk => 9,
            OpCode::Apply => 18,
            OpCode::Reply => 9,
            OpCode::Drop => 9,
        }
    }
}

struct MessageParser<'a> {
    pending_opcode: Option<OpCode>,
    pending_opcode_buf: [u8; 1],
    pending_payload: [u8; 18],
    pending_payload_offset: usize,
    expected_payload_size: usize,
    rb: RingBufferReceiver<'a>,
}

impl<'a> MessageParser<'a> {
    fn new(rb: RingBufferReceiver<'a>) -> MessageParser<'a> {
        Self {
            pending_opcode: None,
            pending_opcode_buf: [0; 1],
            pending_payload: [0; 18],
            pending_payload_offset: 0,
            expected_payload_size: 0,
            rb,
        }
    }

    // Read exactly once
    fn read(&mut self) -> Result<Option<Message>, RingBufferError> {
        match &self.pending_opcode {
            None => match self.rb.read(&mut self.pending_opcode_buf) {
                Ok(_) => {
                    self.pending_opcode = Some(
                        OpCode::try_from(self.pending_opcode_buf[0])
                            .expect("Failed to parse OpCode"),
                    );
                    self.expected_payload_size =
                        Message::msg_size(&self.pending_opcode.as_ref().unwrap());
                    Ok(None)
                }
                Err(e) => Err(e),
            },
            Some(op) => {
                match self.rb.read(
                    &mut self.pending_payload
                        [self.pending_payload_offset..self.expected_payload_size],
                ) {
                    Ok(n) => {
                        self.pending_payload_offset += n;
                        // Check whether end of message
                        if self.pending_payload_offset == self.expected_payload_size {
                            let result = Message::parse(op, &self.pending_payload);

                            // Reset variables
                            self.pending_payload_offset = 0;
                            self.pending_opcode = None;

                            match result {
                                Some(msg) => Ok(Some(msg)),
                                None => Err(RingBufferError::ParseError),
                            }
                        } else {
                            Ok(None)
                        }
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

    fn read_exact_one(&mut self) -> Result<Message, RingBufferError> {
        loop {
            match self.read() {
                Ok(Some(m)) => {
                    return Ok(m);
                }
                _ => {}
            }
        }
    }
}

struct MessageSerializer<'a> {
    opcode_written: bool,
    pending_opcode: [u8; 1],
    pending_payload: [u8; 18],
    pending_payload_offset: usize,
    pending_payload_size: usize,
    rb: RingBufferSender<'a>,
}

impl<'a> MessageSerializer<'a> {
    fn new(rb: RingBufferSender<'a>) -> MessageSerializer<'a> {
        Self {
            opcode_written: true,
            pending_opcode: [0; 1],
            pending_payload: [0; 18],
            pending_payload_offset: 0,
            pending_payload_size: 0,
            rb,
        }
    }

    fn loadable(&self) -> bool {
        self.opcode_written && self.pending_payload_offset == self.pending_payload_size
    }

    fn serialize(&mut self, msg: Message) {
        match msg {
            Message::CreateBlobMessage { ptr: l, size: r }
            | Message::CreateTreeMessage { ptr: l, size: r } => {
                let (left, right) = self.pending_payload.split_at_mut(8);
                let left: &mut [u8; 8] = left.try_into().expect("split slice with wrong length");
                let right: &mut [u8; 8] = (&mut right[0..8])
                    .try_into()
                    .expect("split slice with wrong length");

                *left = usize::to_ne_bytes(l);
                *right = usize::to_ne_bytes(r);
            }
            Message::ApplyMessage {
                lambda_handle: l,
                arg_handle: r,
            } => {
                let (left, right) = self.pending_payload.split_at_mut(9);
                let left: &mut [u8; 9] = left.try_into().expect("split slice with wrong length");
                let right: &mut [u8; 9] = right.try_into().expect("split slice with wrong length");

                *left = ArcaHandle::serialize(ArcaHandle::LambdaHandle(l));
                *right = ArcaHandle::serialize(r);
            }
            Message::CreateThunkMessage { handle: b } => {
                let left: &mut [u8; 9] = (&mut self.pending_payload[0..9]).try_into().unwrap();
                *left = ArcaHandle::serialize(ArcaHandle::BlobHandle(b))
            }
            Message::RunThunkMessage { handle: t } => {
                let left: &mut [u8; 9] = (&mut self.pending_payload[0..9]).try_into().unwrap();
                *left = ArcaHandle::serialize(ArcaHandle::ThunkHandle(t))
            }
            Message::ReplyMessage { handle: h } | Message::DropMessage { handle: h } => {
                let left: &mut [u8; 9] = (&mut self.pending_payload[0..9]).try_into().unwrap();
                *left = ArcaHandle::serialize(h)
            }
        }
    }

    fn load(&mut self, msg: Message) -> Result<(), RingBufferError> {
        if !self.loadable() {
            return Err(RingBufferError::WouldBlock);
        }

        self.opcode_written = false;
        self.pending_opcode[0] = msg.opcode() as u8;
        self.pending_payload_offset = 0;
        self.pending_payload_size = Message::msg_size(&msg.opcode());
        self.serialize(msg);

        return Ok(());
    }

    fn write(&mut self) -> Result<(), RingBufferError> {
        if !self.opcode_written {
            match self.rb.write(&self.pending_opcode) {
                Ok(_) => {
                    self.opcode_written = true;
                    Ok(())
                }
                Err(e) => Err(e),
            }
        } else {
            match self.rb.write(
                &self.pending_payload[self.pending_payload_offset..self.pending_payload_size],
            ) {
                Ok(n) => {
                    self.pending_payload_offset += n;
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
    }

    fn write_all(&mut self) -> Result<(), RingBufferError> {
        while !self.loadable() {
            let _ = self.write();
        }
        Ok(())
    }
}

pub struct Messenger<'a> {
    serializer: MessageSerializer<'a>,
    parser: MessageParser<'a>,
}

impl<'a> Messenger<'a> {
    pub fn new(endpoint: RingBufferEndPoint<'a>) -> Self {
        let (sender, receiver) = endpoint.into_sender_receiver();
        Self {
            serializer: MessageSerializer::new(sender),
            parser: MessageParser::new(receiver),
        }
    }

    pub fn send(&mut self, msg: Message) -> Result<(), RingBufferError> {
        self.serializer.load(msg)?;
        self.serializer.write_all()
    }

    pub fn get_exact_one(&mut self) -> Result<Message, RingBufferError> {
        self.parser.read_exact_one()
    }

    pub fn get_reply(&mut self, msg: Message) -> Result<Message, RingBufferError> {
        self.send(msg)?;
        self.get_exact_one()
    }
}
