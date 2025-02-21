use common::message::{
    serialize, ApplyMessage, ArcaHandle, CreateBlobMessage, CreateThunkMessage, CreateTreeMessage,
    DropMessage, MessageParser, MessageSerializer, Messages, ReplyMessage, RunThunkMessage,
};
use common::refcnt::RefCnt;
use common::ringbuffer::RingBuffer;
use common::BuddyAllocator;
extern crate alloc;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use std::sync::RwLock;

pub struct ArcaRef<'a> {
    offset: Option<usize>,
    client: Arc<RwLock<Client<'a>>>,
}

impl<'a> ArcaRef<'a> {
    fn new(offset: usize, client: Arc<RwLock<Client<'a>>>) -> Self {
        Self {
            offset: Some(offset),
            client,
        }
    }
}

impl Drop for ArcaRef<'_> {
    fn drop(&mut self) {
        match self.offset {
            Some(o) => {
                println!("Dropped");
                self.client.write().unwrap().drop_handle(o);
            }
            None => {}
        }
    }
}

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

    pub fn create_blob(&mut self, blob: Arc<[u8], &BuddyAllocator>) -> () {
        let (slice_ptr, a) = Arc::into_raw_with_allocator(blob);
        let (ptr, size) = slice_ptr.to_raw_parts();
        let offset = a.to_offset(ptr);
        let msg = Box::new(CreateBlobMessage::new(offset, size));
        self.pending_messages.push_back(serialize(msg));
    }

    fn create_tree_internal(&mut self, tree: Box<[usize], &BuddyAllocator>) -> () {
        let (slice_ptr, a) = Box::into_raw_with_allocator(tree);
        let (ptr, size) = slice_ptr.to_raw_parts();
        let offset = a.to_offset(ptr);
        let msg = Box::new(CreateTreeMessage::new(offset, size));
        self.pending_messages.push_back(serialize(msg));
    }

    pub fn create_tree(&mut self, tree: Vec<ArcaRef>, allocator: &BuddyAllocator) -> () {
        let mut offsets: Vec<usize> = Vec::new();
        for mut a in tree {
            offsets.push(a.offset.take().unwrap());
        }
        let mut treebuf =
            unsafe { Box::new_uninit_slice_in(offsets.len(), allocator).assume_init() };
        treebuf.copy_from_slice(offsets.as_slice());
        self.create_tree_internal(treebuf);
    }

    pub fn create_thunk(&mut self, mut blobref: ArcaRef) -> () {
        let msg = Box::new(CreateThunkMessage::new(blobref.offset.take().unwrap()));
        self.pending_messages.push_back(serialize(msg));
    }

    pub fn run_thunk(&mut self, mut thunkref: ArcaRef) -> () {
        let msg = Box::new(RunThunkMessage::new(thunkref.offset.take().unwrap()));
        self.pending_messages.push_back(serialize(msg));
    }

    pub fn apply_lambda(&mut self, mut lambdaref: ArcaRef, mut argref: ArcaRef) -> () {
        let msg = Box::new(ApplyMessage::new(
            lambdaref.offset.take().unwrap(),
            argref.offset.take().unwrap(),
        ));
        self.pending_messages.push_back(serialize(msg));
    }

    pub fn drop_handle(&mut self, handle: ArcaHandle) -> () {
        let msg = Box::new(DropMessage::new(handle));
        self.pending_messages.push_back(serialize(msg));
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

    fn reply_to_handle(reply: Box<ReplyMessage>, client: Arc<RwLock<Client>>) -> ArcaRef {
        ArcaRef::new(reply.handle, client.clone())
    }

    pub fn get_reply(client: Arc<RwLock<Client>>) -> ArcaRef {
        loop {
            match client.clone().write().unwrap().parser.try_read() {
                Some(m) => match m {
                    Messages::ReplyMessage(reply) => {
                        return Self::reply_to_handle(reply, client);
                    }
                    _ => panic!("Unexpected message type"),
                },
                None => continue,
            }
        }
    }
}
