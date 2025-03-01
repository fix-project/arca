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

fn get_reply<'a>(
    msg: Message,
    msger: Arc<RefCell<Messenger<'a>>>,
) -> Result<ArcaRef<'a>, RingBufferError> {
    match msg {
        Message::ReplyMessage { handle } => Ok(ArcaRef::new(handle, msger)),
        _ => panic!("Invalid message return type"),
    }
}

fn create_blob_internal<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    blob: Arc<[u8], &BuddyAllocator>,
) -> Result<ArcaRef<'a>, RingBufferError> {
    let (slice_ptr, a) = Arc::into_raw_with_allocator(blob);
    let (ptr, size) = slice_ptr.to_raw_parts();
    let ptr = a.to_offset(ptr);
    let reply = msger
        .borrow_mut()
        .get_reply(Message::CreateBlobMessage { ptr, size })?;
    get_reply(reply, msger.clone())
}

pub fn create_blob<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    blob: &[u8],
    allocator: &BuddyAllocator,
) -> Result<ArcaRef<'a>, RingBufferError> {
    let mut blobbuf = unsafe { Arc::new_uninit_slice_in(blob.len(), allocator).assume_init() };
    Arc::get_mut(&mut blobbuf).unwrap().copy_from_slice(blob);
    create_blob_internal(msger, blobbuf)
}

fn create_tree_internal<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    tree: Box<[ArcaHandle], &BuddyAllocator>,
) -> Result<ArcaRef<'a>, RingBufferError> {
    let (slice_ptr, a) = Box::into_raw_with_allocator(tree);
    let (ptr, size) = slice_ptr.to_raw_parts();
    let ptr = a.to_offset(ptr);
    let reply = msger
        .borrow_mut()
        .get_reply(Message::CreateTreeMessage { ptr, size })?;
    get_reply(reply, msger.clone())
}

pub fn create_tree<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    tree: Vec<ArcaRef>,
    allocator: &BuddyAllocator,
) -> Result<ArcaRef<'a>, RingBufferError> {
    let mut treebuf = Vec::new_in(allocator);
    for mut a in tree {
        treebuf.push(a.handle.take().unwrap());
    }
    create_tree_internal(msger, treebuf.into_boxed_slice())
}

pub fn create_thunk<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    mut blobref: ArcaRef,
) -> Result<ArcaRef<'a>, RingBufferError> {
    let reply = msger.borrow_mut().get_reply(Message::CreateThunkMessage {
        handle: blobref.handle.take().unwrap(),
    })?;
    get_reply(reply, msger.clone())
}

pub fn run_thunk<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    mut thunkref: ArcaRef,
) -> Result<ArcaRef<'a>, RingBufferError> {
    let reply = msger.borrow_mut().get_reply(Message::RunThunkMessage {
        handle: thunkref.handle.take().unwrap(),
    })?;
    get_reply(reply, msger.clone())
}

pub fn apply_lambda<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    mut lambdaref: ArcaRef,
    mut argref: ArcaRef,
) -> Result<ArcaRef<'a>, RingBufferError> {
    let reply = msger.borrow_mut().get_reply(Message::ApplyMessage {
        lambda_handle: lambdaref.handle.take().unwrap(),
        arg_handle: argref.handle.take().unwrap(),
    })?;
    get_reply(reply, msger.clone())
}

pub fn drop_handle(msger: &mut Messenger, handle: ArcaHandle) -> Result<(), RingBufferError> {
    let msg = Message::DropMessage { handle };
    msger.send(msg)
}
