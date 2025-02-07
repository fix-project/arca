extern crate alloc;
use alloc::boxed::Box;

use crate::refcnt::RefCnt;
use crate::ringbuffer::RingBuffer;

#[derive(Clone)]
pub enum OpCode {
    Dummy = 0,
    CreateBlob = 1,
    CreateTree = 2,
    CreateThunk = 3,
    RunThunk = 4,
    Apply = 5,
    Reply = 6,
    Drop = 7,
}

impl From<u8> for OpCode {
    fn from(value: u8) -> Self {
        match value {
            _ if value == OpCode::CreateBlob as u8 => OpCode::CreateBlob,
            _ if value == OpCode::CreateTree as u8 => OpCode::CreateTree,
            _ if value == OpCode::CreateThunk as u8 => OpCode::CreateThunk,
            _ if value == OpCode::RunThunk as u8 => OpCode::RunThunk,
            _ if value == OpCode::Apply as u8 => OpCode::Apply,
            _ if value == OpCode::Reply as u8 => OpCode::Reply,
            _ if value == OpCode::Drop as u8 => OpCode::Drop,
            _ => OpCode::Dummy,
        }
    }
}

pub struct ArcaHandle(usize, usize);

unsafe fn as_u8_slice<T: Sized>(input: &T) -> &[u8] {
    core::slice::from_raw_parts((input as *const T) as *const u8, core::mem::size_of::<T>())
}

unsafe fn as_u8_slice_mut<T: Sized>(input: &mut T) -> &mut [u8] {
    core::slice::from_raw_parts_mut((input as *mut T) as *mut u8, core::mem::size_of::<T>())
}

unsafe fn from_boxed_u8_slice<T: Sized>(input: Box<[u8]>) -> Box<T> {
    let ptr = Box::into_raw(input) as *mut T;
    Box::from_raw(ptr)
}

unsafe fn to_boxed_u8_slice<T: Sized>(input: Box<T>) -> Box<[u8]> {
    let ptr = core::ptr::slice_from_raw_parts_mut(
        Box::into_raw(input) as *mut u8,
        core::mem::size_of::<T>(),
    );
    Box::from_raw(ptr)
}

pub trait Message {
    const OPCODE: OpCode;
}

#[repr(C)]
pub struct CreateBlobMessage {
    ptr: usize,
    size: usize,
}

impl Message for CreateBlobMessage {
    const OPCODE: OpCode = OpCode::CreateBlob;
}

#[repr(C)]
pub struct CreateTreeMessage {
    ptr: usize,
    size: usize,
}

impl Message for CreateTreeMessage {
    const OPCODE: OpCode = OpCode::CreateTree;
}

#[repr(C)]
pub struct CreateThunkMessage {
    handle: ArcaHandle,
}

impl Message for CreateThunkMessage {
    const OPCODE: OpCode = OpCode::CreateThunk;
}

#[repr(C)]
pub struct RunThunkMessage {
    handle: ArcaHandle,
}

impl Message for RunThunkMessage {
    const OPCODE: OpCode = OpCode::RunThunk;
}

#[repr(C)]
pub struct ApplyMessage {
    thunk_handle: ArcaHandle,
    arg_handle: ArcaHandle,
}

impl Message for ApplyMessage {
    const OPCODE: OpCode = OpCode::Apply;
}

#[repr(C)]
pub struct ReplyMessage {
    handle: ArcaHandle,
}

impl Message for ReplyMessage {
    const OPCODE: OpCode = OpCode::Reply;
}

#[repr(C)]
pub struct DropMessage {
    handle: ArcaHandle,
}

impl Message for DropMessage {
    const OPCODE: OpCode = OpCode::Drop;
}

pub enum Messages {
    CreateBlobMessage(Box<CreateBlobMessage>),
    CreateTreeMessage(Box<CreateTreeMessage>),
    CreateThunkMessage(Box<CreateThunkMessage>),
    RunThunkMessage(Box<RunThunkMessage>),
    ApplyMessage(Box<ApplyMessage>),
    ReplyMessage(Box<ReplyMessage>),
    DropMessage(Box<DropMessage>),
}

impl OpCode {
    pub fn to_payload_size(&self) -> usize {
        match self {
            OpCode::CreateBlob => core::mem::size_of::<CreateBlobMessage>(),
            OpCode::CreateTree => core::mem::size_of::<CreateTreeMessage>(),
            OpCode::CreateThunk => core::mem::size_of::<CreateThunkMessage>(),
            OpCode::RunThunk => core::mem::size_of::<RunThunkMessage>(),
            OpCode::Apply => core::mem::size_of::<ApplyMessage>(),
            OpCode::Reply => core::mem::size_of::<ReplyMessage>(),
            OpCode::Drop => core::mem::size_of::<DropMessage>(),
            OpCode::Dummy => 0,
        }
    }
}

pub struct MessageParser<'a> {
    opcode_read: bool,
    pending_opcode: u8,
    pending_payload: Option<Box<[u8]>>,
    pending_payload_offset: usize,
    rb: RefCnt<'a, RingBuffer>,
}

impl<'a> MessageParser<'a> {
    pub fn new(rb: RefCnt<'a, RingBuffer>) -> MessageParser<'a> {
        Self {
            opcode_read: false,
            pending_opcode: 0,
            pending_payload: None,
            pending_payload_offset: 0,
            rb,
        }
    }

    fn to_message(raw: Box<[u8]>, opcode: &OpCode) -> Messages {
        match opcode {
            OpCode::CreateBlob => Messages::CreateBlobMessage(unsafe { from_boxed_u8_slice(raw) }),
            OpCode::CreateTree => Messages::CreateTreeMessage(unsafe { from_boxed_u8_slice(raw) }),
            OpCode::CreateThunk => {
                Messages::CreateThunkMessage(unsafe { from_boxed_u8_slice(raw) })
            }
            OpCode::RunThunk => Messages::RunThunkMessage(unsafe { from_boxed_u8_slice(raw) }),
            OpCode::Apply => Messages::ApplyMessage(unsafe { from_boxed_u8_slice(raw) }),
            OpCode::Reply => Messages::ReplyMessage(unsafe { from_boxed_u8_slice(raw) }),
            OpCode::Drop => Messages::DropMessage(unsafe { from_boxed_u8_slice(raw) }),
            _ => todo!(),
        }
    }

    // Read exactly once
    pub fn try_read(&mut self) -> Option<Messages> {
        if !self.rb.readable() {
            return None;
        }

        if !self.opcode_read {
            unsafe { self.rb.read(as_u8_slice_mut(&mut self.pending_opcode)) };
            self.opcode_read = true;

            // Allocate buffer for pending payload
            let size = OpCode::to_payload_size(&OpCode::from(self.pending_opcode));
            self.pending_payload = unsafe { Some(Box::new_uninit_slice(size).assume_init()) };
            self.pending_payload_offset = 0;
        } else {
            // Read into pending_payload_offset
            self.pending_payload_offset += match &mut self.pending_payload {
                Some(p) => self.rb.try_read(&mut (*p)[self.pending_payload_offset..]),
                None => panic!("Uninitialized paylaod"),
            };

            // Check whether end of message
            if self.pending_payload_offset == self.pending_payload.as_ref().unwrap().len() {
                // Reset variables
                self.opcode_read = false;
                self.pending_payload_offset = 0;

                return Some(Self::to_message(
                    self.pending_payload.take().unwrap(),
                    &OpCode::from(self.pending_opcode),
                ));
            }
        }

        return None;
    }
}

fn serialize<T: Message>(msg: Box<T>) -> (u8, Box<[u8]>) {
    (T::OPCODE as u8, unsafe { to_boxed_u8_slice(msg) })
}

pub struct MessageSerializer<'a> {
    opcode_written: bool,
    pending_opcode: u8,
    pending_payload: Option<Box<[u8]>>,
    pending_payload_offset: usize,
    rb: RefCnt<'a, RingBuffer>,
}

impl<'a> MessageSerializer<'a> {
    pub fn new(rb: RefCnt<'a, RingBuffer>) -> MessageSerializer<'a> {
        Self {
            opcode_written: false,
            pending_opcode: 0,
            pending_payload: None,
            pending_payload_offset: 0,
            rb,
        }
    }

    pub fn loadable(&self) -> bool {
        match &self.pending_payload {
            Some(_) => false,
            None => true,
        }
    }

    pub fn load<T: Message>(&mut self, msg: Box<T>) -> () {
        if !self.loadable() {
            panic!("Not loadable");
        }

        self.opcode_written = false;
        let res = serialize(msg);
        self.pending_opcode = res.0;
        self.pending_payload = Some(res.1);
        return;
    }

    pub fn try_write(&mut self) -> () {
        if !self.rb.writable() {
            return;
        }

        if !self.opcode_written {
            unsafe { self.rb.write(as_u8_slice(&self.pending_opcode)) };
            self.opcode_written = true;
        } else {
            self.pending_payload_offset += match &self.pending_payload {
                Some(p) => self.rb.try_write(&(*p)[self.pending_payload_offset..]),
                None => panic!("Uninitialized payload"),
            };

            if self.pending_payload_offset == self.pending_payload.as_ref().unwrap().len() {
                self.opcode_written = false;
                self.pending_payload_offset = 0;
                self.pending_payload = None;
            }
        }
    }
}
