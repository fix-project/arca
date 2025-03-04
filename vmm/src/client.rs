use common::message::{
    ArcaHandle, BlobHandle, LambdaHandle, Message, Messenger, ThunkHandle, TreeHandle,
};
use common::ringbuffer::RingBufferError;
use common::BuddyAllocator;
extern crate alloc;
use alloc::sync::Arc;
use core::cell::RefCell;

pub struct Ref<'a, T: Into<ArcaHandle>> {
    handle: Option<T>,
    msger: Arc<RefCell<Messenger<'a>>>,
}

impl<'a, T: Into<ArcaHandle>> Ref<'a, T> {
    fn take(&mut self) -> Option<T> {
        self.handle.take()
    }
}

impl<T: Into<ArcaHandle>> Drop for Ref<'_, T> {
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

pub type BlobRef<'a> = Ref<'a, BlobHandle>;
pub type TreeRef<'a> = Ref<'a, TreeHandle>;
pub type LambdaRef<'a> = Ref<'a, LambdaHandle>;
pub type ThunkRef<'a> = Ref<'a, ThunkHandle>;

pub enum ArcaRef<'a> {
    BlobRef(BlobRef<'a>),
    TreeRef(TreeRef<'a>),
    LambdaRef(LambdaRef<'a>),
    ThunkRef(ThunkRef<'a>),
}

impl<'a> From<BlobRef<'a>> for ArcaRef<'a> {
    fn from(value: BlobRef<'a>) -> ArcaRef<'a> {
        ArcaRef::BlobRef(value)
    }
}

impl<'a> From<TreeRef<'a>> for ArcaRef<'a> {
    fn from(value: TreeRef<'a>) -> ArcaRef<'a> {
        ArcaRef::TreeRef(value)
    }
}

impl<'a> From<LambdaRef<'a>> for ArcaRef<'a> {
    fn from(value: LambdaRef<'a>) -> ArcaRef<'a> {
        ArcaRef::LambdaRef(value)
    }
}

impl<'a> From<ThunkRef<'a>> for ArcaRef<'a> {
    fn from(value: ThunkRef<'a>) -> ArcaRef<'a> {
        ArcaRef::ThunkRef(value)
    }
}

impl<'a> ArcaRef<'a> {
    fn new(handle: ArcaHandle, msger: Arc<RefCell<Messenger<'a>>>) -> Self {
        match handle {
            ArcaHandle::BlobHandle(h) => ArcaRef::BlobRef(BlobRef {
                handle: Some(h),
                msger,
            }),
            ArcaHandle::TreeHandle(h) => ArcaRef::TreeRef(TreeRef {
                handle: Some(h),
                msger,
            }),
            ArcaHandle::LambdaHandle(h) => ArcaRef::LambdaRef(LambdaRef {
                handle: Some(h),
                msger,
            }),
            ArcaHandle::ThunkHandle(h) => ArcaRef::ThunkRef(ThunkRef {
                handle: Some(h),
                msger,
            }),
        }
    }

    fn take(&mut self) -> Option<ArcaHandle> {
        match self {
            ArcaRef::BlobRef(h) => h.take().map(|h| h.into()),
            ArcaRef::TreeRef(h) => h.take().map(|h| h.into()),
            ArcaRef::LambdaRef(h) => h.take().map(|h| h.into()),
            ArcaRef::ThunkRef(h) => h.take().map(|h| h.into()),
        }
    }
}

fn get_reply<'a>(
    msg: Message,
    msger: Arc<RefCell<Messenger<'a>>>,
) -> Result<ArcaRef<'a>, RingBufferError> {
    match msg {
        Message::ReplyMessage { handle } => Ok(ArcaRef::new(handle, msger)),
        _ => Err(RingBufferError::TypeError),
    }
}

fn create_blob_internal<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    blob: Arc<[u8], &BuddyAllocator>,
) -> Result<BlobRef<'a>, RingBufferError> {
    let (slice_ptr, a) = Arc::into_raw_with_allocator(blob);
    let (ptr, size) = slice_ptr.to_raw_parts();
    let ptr = a.to_offset(ptr);
    let reply = msger
        .borrow_mut()
        .get_reply(Message::CreateBlobMessage { ptr, size })?;
    let reply = get_reply(reply, msger.clone())?;
    match reply {
        ArcaRef::BlobRef(b) => Ok(b),
        _ => Err(RingBufferError::TypeError),
    }
}

pub fn create_blob<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    blob: &[u8],
    allocator: &BuddyAllocator,
) -> Result<BlobRef<'a>, RingBufferError> {
    let mut blobbuf = unsafe { Arc::new_uninit_slice_in(blob.len(), allocator).assume_init() };
    Arc::get_mut(&mut blobbuf).unwrap().copy_from_slice(blob);
    create_blob_internal(msger, blobbuf)
}

fn create_tree_internal<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    tree: Box<[ArcaHandle], &BuddyAllocator>,
) -> Result<TreeRef<'a>, RingBufferError> {
    let (slice_ptr, a) = Box::into_raw_with_allocator(tree);
    let (ptr, size) = slice_ptr.to_raw_parts();
    let ptr = a.to_offset(ptr);
    let reply = msger
        .borrow_mut()
        .get_reply(Message::CreateTreeMessage { ptr, size })?;
    let reply = get_reply(reply, msger.clone())?;
    match reply {
        ArcaRef::TreeRef(t) => Ok(t),
        _ => Err(RingBufferError::TypeError),
    }
}

pub fn create_tree<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    tree: Vec<ArcaRef>,
    allocator: &BuddyAllocator,
) -> Result<TreeRef<'a>, RingBufferError> {
    let mut treebuf = Vec::new_in(allocator);
    for mut a in tree {
        treebuf.push(a.take().unwrap());
    }
    create_tree_internal(msger, treebuf.into_boxed_slice())
}

pub fn create_thunk<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    mut blobref: BlobRef,
) -> Result<ThunkRef<'a>, RingBufferError> {
    let reply = msger.borrow_mut().get_reply(Message::CreateThunkMessage {
        handle: blobref.handle.take().unwrap(),
    })?;
    let reply = get_reply(reply, msger.clone())?;
    match reply {
        ArcaRef::ThunkRef(t) => Ok(t),
        _ => Err(RingBufferError::TypeError),
    }
}

pub fn run_thunk<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    mut thunkref: ThunkRef,
) -> Result<ArcaRef<'a>, RingBufferError> {
    let reply = msger.borrow_mut().get_reply(Message::RunThunkMessage {
        handle: thunkref.handle.take().unwrap(),
    })?;
    get_reply(reply, msger.clone())
}

pub fn apply_lambda<'a>(
    msger: &Arc<RefCell<Messenger<'a>>>,
    mut lambdaref: LambdaRef,
    mut argref: ArcaRef,
) -> Result<ArcaRef<'a>, RingBufferError> {
    let reply = msger.borrow_mut().get_reply(Message::ApplyMessage {
        lambda_handle: lambdaref.handle.take().unwrap(),
        arg_handle: argref.take().unwrap(),
    })?;
    get_reply(reply, msger.clone())
}

pub fn drop_handle<T: Into<ArcaHandle>>(
    msger: &mut Messenger,
    handle: T,
) -> Result<(), RingBufferError> {
    let msg = Message::DropMessage {
        handle: handle.into(),
    };
    msger.send(msg)
}
