use common::message::RawMessage;
use common::ringbuffer::RingBuffer;
use crate::prelude::RefCnt;
use crate::types::Value;
use alloc::collections::HashMap;

pub struct MessageParser<'a> {
    rb_in : RefCnt<'a, RingBuffer<RawMessage>>,
    rb_out : RefCnt<'a, RingBuffer<RawMessage>>,
    state : HashMap<usize, Value>
}

impl MessageParser <'_> {
    pub fn new( rb_in: RefCnt<RingBuffer<RawMessage>>,
                rb_out: RefCnt<RingBuffer<RawMessage>> ) -> Self {
        Self {
            rb_in,
            rb_out,
            state: HashMap::new()
        }
    }
}
