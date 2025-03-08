extern crate alloc;

use crate::{
    ringbuffer::{RingBufferEndPoint, RingBufferError, RingBufferReceiver, RingBufferSender},
    BuddyAllocator,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
enum OpCode {
    CreateBlob = 0,
    CreateTree = 1,
    CreateThunk = 2,
    RunThunk = 3,
    Apply = 4,
    Reply = 5,
    Drop = 6,
    ReadBlob = 7,
    BlobContents = 8,
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
            _ if value == OpCode::ReadBlob as u8 => Ok(OpCode::ReadBlob),
            _ if value == OpCode::BlobContents as u8 => Ok(OpCode::BlobContents),
            _ => Err("Invalid raw u8 for OpCode"),
        }
    }
}

pub type RawHandle = usize;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct BlobHandle(RawHandle);
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct TreeHandle(RawHandle);
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct LambdaHandle(RawHandle);
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ThunkHandle(RawHandle);

impl BlobHandle {
    pub fn new(s: RawHandle) -> Self {
        Self(s)
    }
}

impl TreeHandle {
    pub fn new(s: RawHandle) -> Self {
        Self(s)
    }
}

impl LambdaHandle {
    pub fn new(s: RawHandle) -> Self {
        Self(s)
    }
}

impl ThunkHandle {
    pub fn new(s: RawHandle) -> Self {
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Handle {
    Blob(BlobHandle),
    Tree(TreeHandle),
    Lambda(LambdaHandle),
    Thunk(ThunkHandle),
}

pub trait ArcaHandle: Into<Handle> + Copy {}

impl Handle {
    fn parse(buf: &[u8; 9]) -> Self {
        let handletype = HandleType::try_from(buf[0]).expect("");
        let offset = usize::from_ne_bytes(buf[1..9].try_into().unwrap());
        match handletype {
            HandleType::Blob => Self::Blob(BlobHandle(offset)),
            HandleType::Tree => Self::Tree(TreeHandle(offset)),
            HandleType::Lambda => Self::Lambda(LambdaHandle(offset)),
            HandleType::Thunk => Self::Thunk(ThunkHandle(offset)),
        }
    }

    pub fn to_raw(&self) -> usize {
        match self {
            Handle::Blob(BlobHandle(h))
            | Handle::Tree(TreeHandle(h))
            | Handle::Lambda(LambdaHandle(h))
            | Handle::Thunk(ThunkHandle(h)) => *h,
        }
    }

    fn serialize(self) -> [u8; 9] {
        let mut result: [u8; 9] = [0; 9];
        result[0] = match &self {
            Handle::Blob(_) => HandleType::Blob as u8,
            Handle::Tree(_) => HandleType::Tree as u8,
            Handle::Lambda(_) => HandleType::Lambda as u8,
            Handle::Thunk(_) => HandleType::Thunk as u8,
        };

        let sub: &mut [u8; 8] = (&mut result[1..9]).try_into().unwrap();
        *sub = self.to_raw().to_ne_bytes();

        result
    }

    pub fn to_offset<T: Into<Handle>>(x: T) -> usize {
        let x: Handle = x.into();
        x.to_raw()
    }
}

impl From<BlobHandle> for Handle {
    fn from(value: BlobHandle) -> Handle {
        Handle::Blob(value)
    }
}

impl From<TreeHandle> for Handle {
    fn from(value: TreeHandle) -> Handle {
        Handle::Tree(value)
    }
}

impl From<LambdaHandle> for Handle {
    fn from(value: LambdaHandle) -> Handle {
        Handle::Lambda(value)
    }
}

impl From<ThunkHandle> for Handle {
    fn from(value: ThunkHandle) -> Handle {
        Handle::Thunk(value)
    }
}

impl ArcaHandle for BlobHandle {}
impl ArcaHandle for TreeHandle {}
impl ArcaHandle for LambdaHandle {}
impl ArcaHandle for ThunkHandle {}

#[derive(Debug)]
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
        arg_handle: Handle,
    },
    ReplyMessage {
        handle: Handle,
    },
    DropMessage {
        handle: Handle,
    },
    ReadBlobMessage {
        handle: BlobHandle,
    },
    BlobContentsMessage {
        ptr: usize,
        size: usize,
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
            Message::ReadBlobMessage { .. } => OpCode::ReadBlob,
            Message::BlobContentsMessage { .. } => OpCode::BlobContents,
        }
    }

    fn parse(op: &OpCode, buf: &[u8; 18]) -> Option<Message> {
        match op {
            OpCode::CreateBlob | OpCode::CreateTree | OpCode::BlobContents => {
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
                    OpCode::BlobContents => Some(Message::BlobContentsMessage {
                        ptr: left,
                        size: right,
                    }),
                    _ => unreachable!(),
                }
            }
            _ => {
                let left: &[u8; 9] = buf[0..9].try_into().expect("split slice with wrong length");
                let left = Handle::parse(left);

                match op {
                    OpCode::CreateThunk => {
                        if let Handle::Blob(b) = left {
                            Some(Message::CreateThunkMessage { handle: b })
                        } else {
                            None
                        }
                    }
                    OpCode::ReadBlob => {
                        if let Handle::Blob(b) = left {
                            Some(Message::ReadBlobMessage { handle: b })
                        } else {
                            None
                        }
                    }
                    OpCode::RunThunk => {
                        if let Handle::Thunk(t) = left {
                            Some(Message::RunThunkMessage { handle: t })
                        } else {
                            None
                        }
                    }
                    OpCode::Apply => {
                        if let Handle::Lambda(l) = left {
                            let right: &[u8; 9] = buf[9..18]
                                .try_into()
                                .expect("split slice with wrong length");
                            let right = Handle::parse(right);
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

    fn msg_size(op: OpCode) -> usize {
        match op {
            OpCode::CreateBlob => 16,
            OpCode::CreateTree => 16,
            OpCode::CreateThunk => 9,
            OpCode::RunThunk => 9,
            OpCode::Apply => 18,
            OpCode::Reply => 9,
            OpCode::Drop => 9,
            OpCode::ReadBlob => 9,
            OpCode::BlobContents => 16,
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
                    self.expected_payload_size = Message::msg_size(self.pending_opcode.unwrap());
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

    fn read_one(&mut self) -> Result<Message, RingBufferError> {
        loop {
            match self.read() {
                Ok(Some(m)) => {
                    return Ok(m);
                }
                _ => core::hint::spin_loop(),
            }
        }
    }

    pub fn allocator(&self) -> &'a BuddyAllocator<'a> {
        self.rb.allocator()
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

                *left = Handle::serialize(Handle::Lambda(l));
                *right = Handle::serialize(r);
            }
            Message::CreateThunkMessage { handle: b } => {
                let left: &mut [u8; 9] = (&mut self.pending_payload[0..9]).try_into().unwrap();
                *left = Handle::serialize(Handle::Blob(b))
            }
            Message::RunThunkMessage { handle: t } => {
                let left: &mut [u8; 9] = (&mut self.pending_payload[0..9]).try_into().unwrap();
                *left = Handle::serialize(Handle::Thunk(t))
            }
            Message::ReplyMessage { handle: h } | Message::DropMessage { handle: h } => {
                let left: &mut [u8; 9] = (&mut self.pending_payload[0..9]).try_into().unwrap();
                *left = Handle::serialize(h)
            }
            Message::ReadBlobMessage { handle: b } => {
                let left: &mut [u8; 9] = (&mut self.pending_payload[0..9]).try_into().unwrap();
                *left = Handle::serialize(Handle::Blob(b))
            }
            Message::BlobContentsMessage { ptr, size } => {
                let (left, right) = self.pending_payload.split_at_mut(8);
                let left: &mut [u8; 8] = left.try_into().expect("split slice with wrong length");
                let right: &mut [u8; 8] = (&mut right[0..8])
                    .try_into()
                    .expect("split slice with wrong length");

                *left = usize::to_ne_bytes(ptr);
                *right = usize::to_ne_bytes(size);
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
        self.pending_payload_size = Message::msg_size(msg.opcode());
        self.serialize(msg);

        Ok(())
    }

    fn write(&mut self) -> Result<(), RingBufferError> {
        if !self.opcode_written {
            log::debug!("writing opcode");
            match self.rb.write(&self.pending_opcode) {
                Ok(_) => {
                    self.opcode_written = true;
                    Ok(())
                }
                Err(e) => Err(e),
            }
        } else {
            log::debug!("writing message");
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
            self.write()?;
        }
        Ok(())
    }

    pub fn allocator(&self) -> &'a BuddyAllocator<'a> {
        self.rb.allocator()
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

    pub fn receive(&mut self) -> Result<Message, RingBufferError> {
        self.parser.read_one()
    }

    pub fn send_and_receive(&mut self, msg: Message) -> Result<Message, RingBufferError> {
        self.send(msg)?;
        let response = self.parser.read_one()?;
        log::debug!("received {response:x?}");
        Ok(response)
    }

    pub fn send_and_receive_handle(&mut self, msg: Message) -> Result<Handle, RingBufferError> {
        let response = self.send_and_receive(msg)?;
        let Message::ReplyMessage { handle } = response else {
            return Err(RingBufferError::TypeError);
        };
        Ok(handle)
    }

    pub fn send(&mut self, msg: Message) -> Result<(), RingBufferError> {
        log::debug!("sending {msg:x?}");
        self.serializer.load(msg)?;
        self.serializer.write_all()?;
        Ok(())
    }

    pub fn allocator(&self) -> &'a BuddyAllocator<'a> {
        assert_eq!(
            self.serializer.allocator() as *const _,
            self.parser.allocator() as *const _
        );
        self.serializer.allocator()
    }
}
