use common::message::{ArcaHandle, Message, Messenger};
use common::ringbuffer::RingBufferError;
use common::BuddyAllocator;
extern crate alloc;
use alloc::sync::Arc;
use core::cell::RefCell;

pub struct ArcaRef<'a> {
    handle: Option<ArcaHandle>,
    msger: Arc<RefCell<Messenger<'a>>>,
}

impl<'a> ArcaRef<'a> {
    fn new(handle: ArcaHandle, msger: Arc<RefCell<Messenger<'a>>>) -> Self {
        Self {
            handle: Some(handle),
            msger,
        }
    }
}

impl Drop for ArcaRef<'_> {
    fn drop(&mut self) {
        let h = self.handle.take();
        match h {
            Some(h) => {
                let _ = drop_handle(&mut self.msger.borrow_mut(), h);
            }
            None => {}
        }
    }
}

fn create_blob_internal(
    msger: &mut Messenger,
    blob: Arc<[u8], &BuddyAllocator>,
) -> Result<(), RingBufferError> {
    let (slice_ptr, a) = Arc::into_raw_with_allocator(blob);
    let (ptr, size) = slice_ptr.to_raw_parts();
    let ptr = a.to_offset(ptr);
    msger.push_outgoing_message(Message::CreateBlobMessage { ptr, size })
}

pub fn create_blob(
    msger: &mut Messenger,
    blob: &[u8],
    allocator: &BuddyAllocator,
) -> Result<(), RingBufferError> {
    let mut blobbuf = unsafe { Arc::new_uninit_slice_in(blob.len(), allocator).assume_init() };
    Arc::get_mut(&mut blobbuf).unwrap().copy_from_slice(blob);
    create_blob_internal(msger, blobbuf)
}

fn create_tree_internal(
    msger: &mut Messenger,
    tree: Box<[ArcaHandle], &BuddyAllocator>,
) -> Result<(), RingBufferError> {
    let (slice_ptr, a) = Box::into_raw_with_allocator(tree);
    let (ptr, size) = slice_ptr.to_raw_parts();
    let ptr = a.to_offset(ptr);
    msger.push_outgoing_message(Message::CreateTreeMessage { ptr, size })
}

pub fn create_tree(
    msger: &mut Messenger,
    tree: Vec<ArcaRef>,
    allocator: &BuddyAllocator,
) -> Result<(), RingBufferError> {
    let mut treebuf = Vec::new_in(allocator);
    for mut a in tree {
        treebuf.push(a.handle.take().unwrap());
    }
    create_tree_internal(msger, treebuf.into_boxed_slice())
}

pub fn create_thunk(msger: &mut Messenger, mut blobref: ArcaRef) -> Result<(), RingBufferError> {
    msger.push_outgoing_message(Message::CreateThunkMessage {
        handle: blobref.handle.take().unwrap(),
    })
}

pub fn run_thunk(msger: &mut Messenger, mut thunkref: ArcaRef) -> Result<(), RingBufferError> {
    msger.push_outgoing_message(Message::RunThunkMessage {
        handle: thunkref.handle.take().unwrap(),
    })
}

pub fn apply_lambda(
    msger: &mut Messenger,
    mut lambdaref: ArcaRef,
    mut argref: ArcaRef,
) -> Result<(), RingBufferError> {
    msger.push_outgoing_message(Message::ApplyMessage {
        lambda_handle: lambdaref.handle.take().unwrap(),
        arg_handle: argref.handle.take().unwrap(),
    })
}

pub fn drop_handle(msger: &mut Messenger, handle: ArcaHandle) -> Result<(), RingBufferError> {
    let msg = Message::DropMessage { handle };
    msger.push_outgoing_message(msg)
}

pub fn get_reply<'a>(
    msger_mut: &mut Messenger<'a>,
    msger: Arc<RefCell<Messenger<'a>>>,
) -> Result<ArcaRef<'a>, RingBufferError> {
    msger_mut
        .write_all()
        .and_then(|()| msger_mut.read_exact(1))
        .and_then(|()| Ok(msger_mut.pop_incoming_message().unwrap()))
        .and_then(|msg| match msg {
            Message::ReplyMessage { handle } => Ok(ArcaRef::new(handle, msger.clone())),
            _ => panic!("Invalid message return type"),
        })
}
