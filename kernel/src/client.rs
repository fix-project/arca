use crate::prelude::*;
use crate::types::Value;
use common::message::{serialize, MessageParser, MessageSerializer, Messages, ReplyMessage};
use common::ringbuffer::RingBuffer;

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;

pub struct Client<'a> {
    parser: MessageParser<'a>,
    serializer: MessageSerializer<'a>,
    pending_messages: VecDeque<(u8, Box<[u8]>)>,
}

impl<'a> Client<'a> {
    pub fn new(rb_incoming: RefCnt<'a, RingBuffer>, rb_outgoing: RefCnt<'a, RingBuffer>) -> Self {
        Self {
            parser: MessageParser::new(rb_incoming),
            serializer: MessageSerializer::new(rb_outgoing),
            pending_messages: VecDeque::new(),
        }
    }

    pub fn send_all(&mut self) -> () {
        while !self.pending_messages.is_empty() {
            let msg = self.pending_messages.pop_front().unwrap();

            // Load the message
            while !self.serializer.loadable() {
                self.serializer.try_write();
            }
            self.serializer.load(msg.0, msg.1);
        }
        while !self.serializer.loadable() {
            self.serializer.try_write();
        }
    }

    fn reply(&mut self, value: Box<Value>) -> () {
        let ptr = Box::into_raw(value);
        let msg = Box::new(ReplyMessage::new(PHYSICAL_ALLOCATOR.to_offset(ptr)));
        self.pending_messages.push_back(serialize(msg));
    }

    fn reconstruct(offset: usize) -> Box<Value> {
        let ptr = PHYSICAL_ALLOCATOR.from_offset::<Value>(offset);
        unsafe { Box::from_raw(ptr as *mut Value) }
    }

    pub fn process_incoming_message(&mut self, msg: Messages, cpu: &mut Cpu) -> bool {
        match msg {
            Messages::CreateBlobMessage(m) => {
                let blob: Blob = unsafe {
                    Arc::from_raw(core::ptr::slice_from_raw_parts(
                        PHYSICAL_ALLOCATOR.from_offset::<u8>(m.ptr),
                        m.size,
                    ))
                };
                self.reply(Box::new(Value::Blob(blob)));
                log::info!("processed create blob");
                true
            }
            Messages::CreateTreeMessage(m) => {
                let vals: Box<[usize]> = unsafe {
                    Box::from_raw(core::ptr::slice_from_raw_parts_mut(
                        PHYSICAL_ALLOCATOR.from_offset::<usize>(m.ptr) as *mut usize,
                        m.size,
                    ))
                };
                let mut vec = Vec::new();
                for v in vals {
                    vec.push(Box::into_inner(Self::reconstruct(v)));
                }
                self.reply(Box::new(Value::Tree(vec.into())));
                log::info!("processed create tree");
                true
            }
            Messages::CreateThunkMessage(m) => {
                let v = Self::reconstruct(m.handle);
                match Box::into_inner(v) {
                    Value::Blob(b) => self.reply(Box::new(Value::Thunk(Thunk::from_elf(&*b)))),
                    _ => todo!(),
                };
                log::info!("processed create thunk");
                true
            }
            Messages::RunThunkMessage(m) => {
                let v = Self::reconstruct(m.handle);
                match Box::into_inner(v) {
                    Value::Thunk(thunk) => self.reply(Box::new(thunk.run(cpu))),
                    _ => todo!(),
                };
                log::info!("processed run thunk");
                true
            }
            Messages::ApplyMessage(m) => {
                let v = Box::into_inner(Self::reconstruct(m.lambda_handle));
                let arg = Box::into_inner(Self::reconstruct(m.arg_handle));
                match v {
                    Value::Lambda(lambda) => self.reply(Box::new(Value::Thunk(lambda.apply(arg)))),
                    _ => todo!(),
                };
                log::info!("processed apply lambda");
                true
            }
            Messages::DropMessage(m) => {
                Self::reconstruct(m.handle);
                log::info!("processed drop");
                false
            }
            _ => todo!(),
        }
    }

    pub fn read_one_message(&mut self) -> Messages {
        loop {
            match self.parser.try_read() {
                Some(m) => {
                    return m;
                }
                None => continue,
            }
        }
    }
}
