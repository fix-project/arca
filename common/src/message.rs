extern crate alloc;

use crate::refcnt::RefCnt;
use crate::ringbuffer::RingBuffer;

#[derive(Clone)]
#[repr(u8)]
pub enum OpCode {
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

pub struct ArcaHandle(usize);

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
        handle: ArcaHandle,
    },
    RunThunkMessage {
        handle: ArcaHandle,
    },
    ApplyMessage {
        lambda_handle: ArcaHandle,
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

    fn parse(op: &OpCode, buf: &[u8; 16]) -> Message {
        let left: &[u8; 8] = buf[0..8].try_into().expect("split slice with wrong length");
        let right: &[u8; 8] = buf[8..16]
            .try_into()
            .expect("split slice with wrong length");
        let left = usize::from_ne_bytes(*left);
        let right = usize::from_ne_bytes(*right);

        match op {
            OpCode::CreateBlob => Message::CreateBlobMessage {
                ptr: left,
                size: right,
            },
            OpCode::CreateTree => Message::CreateTreeMessage {
                ptr: left,
                size: right,
            },
            OpCode::CreateThunk => Message::CreateThunkMessage {
                handle: ArcaHandle(left),
            },
            OpCode::RunThunk => Message::RunThunkMessage {
                handle: ArcaHandle(left),
            },
            OpCode::Apply => Message::ApplyMessage {
                lambda_handle: ArcaHandle(left),
                arg_handle: ArcaHandle(right),
            },
            OpCode::Reply => Message::ReplyMessage {
                handle: ArcaHandle(left),
            },
            OpCode::Drop => Message::DropMessage {
                handle: ArcaHandle(left),
            },
        }
    }

    fn msg_size(op: &OpCode) -> usize {
        match op {
            OpCode::CreateBlob => 16,
            OpCode::CreateTree => 16,
            OpCode::CreateThunk => 8,
            OpCode::RunThunk => 8,
            OpCode::Apply => 16,
            OpCode::Reply => 8,
            OpCode::Drop => 8,
        }
    }
}

pub struct MessageParser<'a> {
    pending_opcode: Option<OpCode>,
    pending_opcode_buf: [u8; 1],
    pending_payload: [u8; 16],
    pending_payload_offset: usize,
    expected_payload_size: usize,
    rb: RefCnt<'a, RingBuffer>,
}

impl<'a> MessageParser<'a> {
    pub fn new(rb: RefCnt<'a, RingBuffer>) -> MessageParser<'a> {
        Self {
            pending_opcode: None,
            pending_opcode_buf: [0; 1],
            pending_payload: [0; 16],
            pending_payload_offset: 0,
            expected_payload_size: 0,
            rb,
        }
    }

    // Read exactly once
    pub fn try_read(&mut self) -> Option<Message> {
        if !self.rb.readable() {
            return None;
        }

        match &self.pending_opcode {
            None => {
                self.rb.try_read(&mut self.pending_opcode_buf);
                self.pending_opcode = Some(
                    OpCode::try_from(self.pending_opcode_buf[0]).expect("Failed to parse OpCode"),
                );
                self.expected_payload_size =
                    Message::msg_size(&self.pending_opcode.as_ref().unwrap());
                None
            }
            Some(op) => {
                // Read into pending_payload_offset
                self.pending_payload_offset += self.rb.try_read(
                    &mut self.pending_payload
                        [self.pending_payload_offset..self.expected_payload_size],
                );

                // Check whether end of message
                if self.pending_payload_offset == self.expected_payload_size {
                    let result = Some(Message::parse(op, &self.pending_payload));

                    // Reset variables
                    self.pending_payload_offset = 0;
                    self.pending_opcode = None;

                    result
                } else {
                    None
                }
            }
        }
    }
}

//fn serialize<T: Message>(msg: Box<T>) -> (u8, Box<[u8]>) {
//    (T::OPCODE as u8, unsafe { to_boxed_u8_slice(msg) })
//}

pub struct MessageSerializer<'a> {
    opcode_written: bool,
    pending_opcode: [u8; 1],
    pending_payload: [u8; 16],
    pending_payload_offset: usize,
    pending_payload_size: usize,
    rb: RefCnt<'a, RingBuffer>,
}

impl<'a> MessageSerializer<'a> {
    pub fn new(rb: RefCnt<'a, RingBuffer>) -> MessageSerializer<'a> {
        Self {
            opcode_written: true,
            pending_opcode: [0; 1],
            pending_payload: [0; 16],
            pending_payload_offset: 0,
            pending_payload_size: 0,
            rb,
        }
    }

    pub fn loadable(&self) -> bool {
        self.opcode_written && self.pending_payload_offset == self.pending_payload_size
    }

    fn serialize(&mut self, msg: Message) {
        let (left, right) = self.pending_payload.split_at_mut(8);
        let left: &mut [u8; 8] = left.try_into().expect("split slice with wrong length");
        let right: &mut [u8; 8] = right.try_into().expect("split slice with wrong length");

        match msg {
            Message::CreateBlobMessage { ptr: l, size: r }
            | Message::CreateTreeMessage { ptr: l, size: r }
            | Message::ApplyMessage {
                lambda_handle: ArcaHandle(l),
                arg_handle: ArcaHandle(r),
            } => {
                *left = usize::to_ne_bytes(l);
                *right = usize::to_ne_bytes(r);
            }
            Message::CreateThunkMessage {
                handle: ArcaHandle(l),
            }
            | Message::ReplyMessage {
                handle: ArcaHandle(l),
            }
            | Message::RunThunkMessage {
                handle: ArcaHandle(l),
            }
            | Message::DropMessage {
                handle: ArcaHandle(l),
            } => {
                *left = usize::to_ne_bytes(l);
            }
        }
    }

    pub fn load(&mut self, msg: Message) -> Result<(), &'static str> {
        if !self.loadable() {
            return Err("Not loadable");
        }

        self.opcode_written = false;
        self.pending_opcode[0] = msg.opcode() as u8;
        self.pending_payload_size = Message::msg_size(&msg.opcode());
        self.serialize(msg);

        return Ok(());
    }

    pub fn try_write(&mut self) -> () {
        if !self.rb.writable() {
            return;
        }

        if !self.opcode_written {
            self.rb.write(&self.pending_opcode);
            self.opcode_written = true;
        } else {
            self.pending_payload_offset += self.rb.try_write(
                &self.pending_payload[self.pending_payload_offset..self.pending_payload_size],
            );
        }
    }
}
