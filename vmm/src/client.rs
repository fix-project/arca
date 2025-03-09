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
        let msg = Message::Drop(self.handle.into());
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

impl ArcaRef<'_, '_> {
    pub fn handle(&self) -> Handle {
        match self {
            ArcaRef::Blob(h) => Handle::Blob(h.handle),
            ArcaRef::Tree(h) => Handle::Tree(h.handle),
            ArcaRef::Lambda(h) => Handle::Lambda(h.handle),
            ArcaRef::Thunk(h) => Handle::Thunk(h.handle),
        }
    }
}

impl<'a, 'b> From<ArcaRef<'a, 'b>> for Handle
where
    'b: 'a,
{
    fn from(value: ArcaRef<'a, 'b>) -> Self {
        value.handle()
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
        let (ptr, len) = slice.to_raw_parts();
        let ptr = a.to_offset(ptr);
        let mut m = self.messenger.lock();
        if let ArcaRef::Blob(b) =
            self.make_ref(m.send_and_receive_handle(Message::CreateBlob { ptr, len })?)
        {
            Ok(b)
        } else {
            Err(RingBufferError::TypeError)
        }
    }

    pub fn create_tree<'a>(&'a self, tree: Vec<ArcaRef>) -> Result<TreeRef<'a, 'b>, RingBufferError>
    where
        'b: 'a,
    {
        let mut new: Vec<Handle, &BuddyAllocator> =
            Vec::with_capacity_in(tree.len(), self.allocator);
        for r in tree {
            new.push(r.handle());
            core::mem::forget(r);
        }
        let tree = new.into_boxed_slice();

        let (slice, a) = Box::into_raw_with_allocator(tree);
        let (ptr, len) = slice.to_raw_parts();

        let ptr = a.to_offset(ptr);
        let mut m = self.messenger.lock();
        if let ArcaRef::Tree(t) =
            self.make_ref(m.send_and_receive_handle(Message::CreateTree { ptr, len })?)
        {
            Ok(t)
        } else {
            Err(RingBufferError::TypeError)
        }
    }
}

impl<'a, 'b> Ref<'a, 'b, BlobHandle>
where
    'b: 'a,
{
    pub fn read(&self) -> Result<Arc<[u8], &BuddyAllocator>, RingBufferError> {
        let handle = self.handle;
        let mut m = self.client.messenger.lock();
        let Message::BlobContents { ptr, len } = m.send_and_receive(Message::ReadBlob(handle))?
        else {
            return Err(RingBufferError::TypeError);
        };
        let allocator = self.client.allocator;
        unsafe {
            let ptr: *const u8 = allocator.from_offset(ptr);
            let ptr = core::ptr::from_raw_parts(ptr, len);
            let arc: Arc<[u8], &BuddyAllocator> = Arc::from_raw_in(ptr, allocator);
            Ok(arc)
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
            client.make_ref(m.send_and_receive_handle(Message::CreateThunk(handle))?)
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
        Ok(client.make_ref(m.send_and_receive_handle(Message::Run(handle))?))
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
        let arg_handle = value.handle();
        core::mem::forget(value);
        let mut m = client.messenger.lock();
        if let ArcaRef::Thunk(t) =
            client.make_ref(m.send_and_receive_handle(Message::Apply(handle, arg_handle))?)
        {
            Ok(t)
        } else {
            Err(RingBufferError::TypeError)
        }
    }
}
