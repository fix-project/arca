use std::mem::MaybeUninit;
use std::sync::Arc;

use common::message::{
    ArcaHandle, BlobHandle, Handle, LambdaHandle, Message, Messenger, ThunkHandle, TreeHandle,
};
use common::ringbuffer::RingBufferError;
use common::BuddyAllocator;
extern crate alloc;
use common::util::spinlock::SpinLock;

pub struct Client<'a> {
    messenger: SpinLock<Messenger<'a>>,
    allocator: &'a BuddyAllocator<'a>,
}

impl<'a> Client<'a> {
    pub fn new(messenger: Messenger<'a>) -> Self {
        let allocator: &'a BuddyAllocator<'a> = messenger.allocator();
        Client {
            messenger: SpinLock::new(messenger),
            allocator,
        }
    }
}

pub struct Ref<'a, 'b, T: ArcaHandle>
where
    'b: 'a,
{
    handle: T,
    client: &'a Client<'b>,
}

impl<T: ArcaHandle> Drop for Ref<'_, '_, T> {
    fn drop(&mut self) {
        let msg = Message::DropMessage {
            handle: self.handle.into(),
        };
        let mut messenger = self.client.messenger.lock();
        messenger.send(msg).unwrap();
    }
}

pub type BlobRef<'a, 'b> = Ref<'a, 'b, BlobHandle>;
pub type TreeRef<'a, 'b> = Ref<'a, 'b, TreeHandle>;
pub type LambdaRef<'a, 'b> = Ref<'a, 'b, LambdaHandle>;
pub type ThunkRef<'a, 'b> = Ref<'a, 'b, ThunkHandle>;

pub enum ArcaRef<'a, 'b>
where
    'b: 'a,
{
    Blob(BlobRef<'a, 'b>),
    Tree(TreeRef<'a, 'b>),
    Lambda(LambdaRef<'a, 'b>),
    Thunk(ThunkRef<'a, 'b>),
}

impl<'a, 'b> From<ArcaRef<'a, 'b>> for Handle
where
    'b: 'a,
{
    fn from(value: ArcaRef<'a, 'b>) -> Self {
        match value {
            ArcaRef::Blob(h) => Handle::Blob(h.handle),
            ArcaRef::Tree(h) => Handle::Tree(h.handle),
            ArcaRef::Lambda(h) => Handle::Lambda(h.handle),
            ArcaRef::Thunk(h) => Handle::Thunk(h.handle),
        }
    }
}
impl<'a, 'b> From<BlobRef<'a, 'b>> for ArcaRef<'a, 'b>
where
    'b: 'a,
{
    fn from(value: BlobRef<'a, 'b>) -> ArcaRef<'a, 'b> {
        ArcaRef::Blob(value)
    }
}

impl<'a, 'b> From<TreeRef<'a, 'b>> for ArcaRef<'a, 'b>
where
    'b: 'a,
{
    fn from(value: TreeRef<'a, 'b>) -> ArcaRef<'a, 'b> {
        ArcaRef::Tree(value)
    }
}

impl<'a, 'b> From<LambdaRef<'a, 'b>> for ArcaRef<'a, 'b>
where
    'b: 'a,
{
    fn from(value: LambdaRef<'a, 'b>) -> ArcaRef<'a, 'b> {
        ArcaRef::Lambda(value)
    }
}

impl<'a, 'b> From<ThunkRef<'a, 'b>> for ArcaRef<'a, 'b>
where
    'b: 'a,
{
    fn from(value: ThunkRef<'a, 'b>) -> ArcaRef<'a, 'b> {
        ArcaRef::Thunk(value)
    }
}

impl<'b> Client<'b> {
    fn make_ref<'a>(&'a self, handle: Handle) -> ArcaRef<'a, 'b>
    where
        'b: 'a,
    {
        match handle {
            Handle::Blob(handle) => ArcaRef::Blob(BlobRef {
                handle,
                client: self,
            }),
            Handle::Tree(handle) => ArcaRef::Tree(TreeRef {
                handle,
                client: self,
            }),
            Handle::Lambda(handle) => ArcaRef::Lambda(LambdaRef {
                handle,
                client: self,
            }),
            Handle::Thunk(handle) => ArcaRef::Thunk(ThunkRef {
                handle,
                client: self,
            }),
        }
    }

    pub fn create_blob<'a>(&'a self, blob: &[u8]) -> Result<BlobRef<'a, 'b>, RingBufferError>
    where
        'b: 'a,
    {
        let allocator = self.allocator;
        let mut buf = Arc::new_uninit_slice_in(blob.len(), allocator);
        Arc::make_mut(&mut buf).write_copy_of_slice(blob);
        let buf = unsafe { buf.assume_init() };
        let (slice, a) = Arc::into_raw_with_allocator(buf);
        let (ptr, size) = slice.to_raw_parts();
        let ptr = a.to_offset(ptr);
        let mut m = self.messenger.lock();
        if let ArcaRef::Blob(b) =
            self.make_ref(m.send_and_receive(Message::CreateBlobMessage { ptr, size })?)
        {
            Ok(b)
        } else {
            Err(RingBufferError::TypeError)
        }
    }

    pub fn create_tree<'a>(&'b self, tree: Vec<ArcaRef>) -> Result<TreeRef<'a, 'b>, RingBufferError>
    where
        'b: 'a,
    {
        let mut new: Vec<ArcaRef, &BuddyAllocator> =
            Vec::with_capacity_in(tree.len(), self.allocator);
        for r in tree {
            new.push(r);
        }
        let tree = new.into_boxed_slice();

        let (slice, a) = Box::into_raw_with_allocator(tree);
        let (ptr, size) = slice.to_raw_parts();
        let ptr = a.to_offset(ptr);
        let mut m = self.messenger.lock();
        if let ArcaRef::Tree(t) =
            self.make_ref(m.send_and_receive(Message::CreateTreeMessage { ptr, size })?)
        {
            Ok(t)
        } else {
            Err(RingBufferError::TypeError)
        }
    }
}

impl<'a, 'b> Ref<'a, 'b, ThunkHandle>
where
    'b: 'a,
{
    pub fn new(blob: BlobRef<'a, 'b>) -> Result<Self, RingBufferError> {
        let BlobRef { client, handle } = blob;
        core::mem::forget(blob);
        let mut m = client.messenger.lock();
        if let ArcaRef::Thunk(t) =
            client.make_ref(m.send_and_receive(Message::CreateThunkMessage { handle })?)
        {
            Ok(t)
        } else {
            Err(RingBufferError::TypeError)
        }
    }

    pub fn run(self) -> Result<ArcaRef<'a, 'b>, RingBufferError> {
        let ThunkRef { client, handle } = self;
        core::mem::forget(self);
        let mut m = client.messenger.lock();
        Ok(client.make_ref(m.send_and_receive(Message::RunThunkMessage { handle })?))
    }
}

impl<'a, 'b> Ref<'a, 'b, LambdaHandle>
where
    'b: 'a,
{
    pub fn apply(self, value: ArcaRef<'a, 'b>) -> Result<ThunkRef<'a, 'b>, RingBufferError> {
        // assert_eq!(self.client as *const _, value.client as *const _);
        let LambdaRef { client, handle } = self;
        core::mem::forget(self);
        let mut m = client.messenger.lock();
        if let ArcaRef::Thunk(t) = client.make_ref(m.send_and_receive(Message::ApplyMessage {
            lambda_handle: handle,
            arg_handle: value.into(),
        })?) {
            Ok(t)
        } else {
            Err(RingBufferError::TypeError)
        }
    }
}
