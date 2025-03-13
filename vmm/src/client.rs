use std::sync::Arc;

use common::message::{
    ArcaHandle, BlobHandle, Handle, LambdaHandle, Message, Messenger, NullHandle, ThunkHandle,
    TreeHandle, WordHandle,
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

impl Drop for Client<'_> {
    fn drop(&mut self) {
        let mut m = self.messenger.lock();
        m.send(Message::Exit).unwrap();
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

pub type NullRef<'a, 'b> = Ref<'a, 'b, NullHandle>;
pub type WordRef<'a, 'b> = Ref<'a, 'b, WordHandle>;
pub type BlobRef<'a, 'b> = Ref<'a, 'b, BlobHandle>;
pub type TreeRef<'a, 'b> = Ref<'a, 'b, TreeHandle>;
pub type LambdaRef<'a, 'b> = Ref<'a, 'b, LambdaHandle>;
pub type ThunkRef<'a, 'b> = Ref<'a, 'b, ThunkHandle>;

pub enum ArcaRef<'a, 'b>
where
    'b: 'a,
{
    Null(NullRef<'a, 'b>),
    Word(WordRef<'a, 'b>),
    Blob(BlobRef<'a, 'b>),
    Tree(TreeRef<'a, 'b>),
    Lambda(LambdaRef<'a, 'b>),
    Thunk(ThunkRef<'a, 'b>),
}

impl ArcaRef<'_, '_> {
    pub fn handle(&self) -> Handle {
        match self {
            ArcaRef::Null(_) => Handle::Null,
            ArcaRef::Word(h) => Handle::Word(h.handle),
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

impl<'a, 'b> From<WordRef<'a, 'b>> for ArcaRef<'a, 'b>
where
    'b: 'a,
{
    fn from(value: WordRef<'a, 'b>) -> ArcaRef<'a, 'b> {
        ArcaRef::Word(value)
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

impl Clone for WordRef<'_, '_> {
    fn clone(&self) -> Self {
        WordRef {
            handle: self.handle,
            client: self.client,
        }
    }
}

impl Clone for BlobRef<'_, '_> {
    fn clone(&self) -> Self {
        unsafe {
            let BlobHandle { ptr, len } = self.handle;
            let this: Arc<[u8], &BuddyAllocator> = Arc::from_raw_in(
                core::ptr::from_raw_parts(self.client.allocator.from_offset::<u8>(ptr), len),
                self.client.allocator,
            );
            let new = this.clone();
            core::mem::forget(this);
            let (slice, a) = Arc::into_raw_with_allocator(new);
            let (ptr, len) = slice.to_raw_parts();
            let ptr = a.to_offset(ptr);
            let handle = BlobHandle { ptr, len };
            BlobRef {
                handle,
                client: self.client,
            }
        }
    }
}

impl Clone for TreeRef<'_, '_> {
    fn clone(&self) -> Self {
        let msg = Message::Clone(self.handle.into());
        let mut messenger = self.client.messenger.lock();
        let Handle::Tree(t) = messenger.send_and_receive_handle(msg).unwrap() else {
            panic!();
        };
        Self {
            handle: t,
            client: self.client,
        }
    }
}

impl Clone for LambdaRef<'_, '_> {
    fn clone(&self) -> Self {
        let msg = Message::Clone(self.handle.into());
        let mut messenger = self.client.messenger.lock();
        let Handle::Lambda(l) = messenger.send_and_receive_handle(msg).unwrap() else {
            panic!();
        };
        Self {
            handle: l,
            client: self.client,
        }
    }
}

impl Clone for ThunkRef<'_, '_> {
    fn clone(&self) -> Self {
        let msg = Message::Clone(self.handle.into());
        let mut messenger = self.client.messenger.lock();
        let Handle::Thunk(t) = messenger.send_and_receive_handle(msg).unwrap() else {
            panic!();
        };
        Self {
            handle: t,
            client: self.client,
        }
    }
}

impl<'b> Client<'b> {
    fn make_ref<'a>(&'a self, handle: Handle) -> ArcaRef<'a, 'b>
    where
        'b: 'a,
    {
        match handle {
            Handle::Null => ArcaRef::Null(NullRef {
                handle: NullHandle,
                client: self,
            }),
            Handle::Word(handle) => ArcaRef::Word(WordRef {
                handle,
                client: self,
            }),
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

    pub fn null<'a>(&'a self) -> Result<NullRef<'a, 'b>, RingBufferError>
    where
        'b: 'a,
    {
        Ok(NullRef {
            handle: NullHandle,
            client: self,
        })
    }

    pub fn create_word<'a>(&'a self, word: u64) -> WordRef<'a, 'b>
    where
        'b: 'a,
    {
        WordRef {
            handle: WordHandle(word),
            client: self,
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
        let handle = BlobHandle { ptr, len };
        Ok(BlobRef {
            handle,
            client: self,
        })
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

impl<'a, 'b> Ref<'a, 'b, WordHandle>
where
    'b: 'a,
{
    pub fn read(&self) -> u64 {
        self.handle.0
    }
}

impl<'a, 'b> Ref<'a, 'b, BlobHandle>
where
    'b: 'a,
{
    pub fn read(&self) -> Result<Arc<[u8], &BuddyAllocator>, RingBufferError> {
        let BlobHandle { ptr, len } = self.handle;
        unsafe {
            let this: Arc<[u8], &BuddyAllocator> = Arc::from_raw_in(
                core::ptr::from_raw_parts(self.client.allocator.from_offset::<u8>(ptr), len),
                self.client.allocator,
            );
            let new = this.clone();
            core::mem::forget(this);
            Ok(new)
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

    pub fn apply_and_run(self, value: ArcaRef<'a, 'b>) -> Result<ArcaRef<'a, 'b>, RingBufferError> {
        // assert_eq!(self.client as *const _, value.client as *const _);
        let LambdaRef { client, handle } = self;
        core::mem::forget(self);
        let arg_handle = value.handle();
        core::mem::forget(value);
        let mut m = client.messenger.lock();
        Ok(client.make_ref(m.send_and_receive_handle(Message::ApplyAndRun(handle, arg_handle))?))
    }
}
