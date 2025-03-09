extern crate alloc;

use bincode::{Decode, Encode};

use crate::{
    ringbuffer::{RingBufferEndPoint, RingBufferError, RingBufferReceiver, RingBufferSender},
    BuddyAllocator,
};

#[derive(Encode, Decode, Debug)]
pub enum Message {
    CreateBlob { ptr: usize, len: usize },
    CreateTree { ptr: usize, len: usize },
    CreateThunk(BlobHandle),
    Run(ThunkHandle),
    Apply(LambdaHandle, Handle),
    Created(Handle),
    Drop(Handle),
    ReadBlob(BlobHandle),
    BlobContents { ptr: usize, len: usize },
}

pub type RawHandle = usize;

#[derive(Encode, Decode, Debug, Copy, Clone, Eq, PartialEq)]
pub struct BlobHandle(RawHandle);
#[derive(Encode, Decode, Debug, Copy, Clone, Eq, PartialEq)]
pub struct TreeHandle(RawHandle);
#[derive(Encode, Decode, Debug, Copy, Clone, Eq, PartialEq)]
pub struct LambdaHandle(RawHandle);
#[derive(Encode, Decode, Debug, Copy, Clone, Eq, PartialEq)]
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

#[derive(Encode, Decode, Debug, Copy, Clone, Eq, PartialEq)]
pub enum Handle {
    Blob(BlobHandle),
    Tree(TreeHandle),
    Lambda(LambdaHandle),
    Thunk(ThunkHandle),
}

pub trait ArcaHandle: Into<Handle> + Copy {}

impl Handle {
    pub fn to_raw(&self) -> usize {
        match self {
            Handle::Blob(BlobHandle(h))
            | Handle::Tree(TreeHandle(h))
            | Handle::Lambda(LambdaHandle(h))
            | Handle::Thunk(ThunkHandle(h)) => *h,
        }
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

pub struct Messenger<'a> {
    buffer: [u8; 16],
    sender: RingBufferSender<'a>,
    receiver: RingBufferReceiver<'a>,
}

impl<'a> Messenger<'a> {
    pub fn new(endpoint: RingBufferEndPoint<'a>) -> Self {
        let (sender, receiver) = endpoint.into_sender_receiver();
        Self {
            buffer: [0; 16],
            sender,
            receiver,
        }
    }

    pub fn send(&mut self, msg: Message) -> Result<(), RingBufferError> {
        log::debug!("sending {msg:x?}");
        let size = bincode::encode_into_slice(msg, &mut self.buffer, bincode::config::standard())
            .map_err(|_| RingBufferError::ParseError)?;
        self.sender.write_all(&size.to_ne_bytes())?;
        let buf = &mut self.buffer[..size];
        self.sender.write_all(buf)?;
        Ok(())
    }

    pub fn receive(&mut self) -> Result<Message, RingBufferError> {
        let mut size: [u8; 8] = [0; 8];
        self.receiver.read_exact(&mut size)?;
        let size = usize::from_ne_bytes(size);
        assert!(size <= self.buffer.len());
        let buf = &mut self.buffer[..size];
        self.receiver.read_exact(buf)?;
        let (decoded, _): (Message, usize) =
            bincode::decode_from_slice(buf, bincode::config::standard())
                .map_err(|_| RingBufferError::ParseError)?;
        Ok(decoded)
    }

    pub fn send_and_receive(&mut self, msg: Message) -> Result<Message, RingBufferError> {
        self.send(msg)?;
        let response = self.receive()?;
        log::debug!("received {response:x?}");
        Ok(response)
    }

    pub fn send_and_receive_handle(&mut self, msg: Message) -> Result<Handle, RingBufferError> {
        let response = self.send_and_receive(msg)?;
        let Message::Created(handle) = response else {
            return Err(RingBufferError::TypeError);
        };
        Ok(handle)
    }

    pub fn allocator(&self) -> &'a BuddyAllocator<'a> {
        assert_eq!(
            self.sender.allocator() as *const _,
            self.receiver.allocator() as *const _
        );
        self.sender.allocator()
    }
}
