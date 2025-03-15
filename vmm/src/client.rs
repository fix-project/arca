use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use common::message::{MetaRequest, MetaResponse, Request, Response};
use common::ringbuffer::{Endpoint, Receiver, Result, Sender};
use common::BuddyAllocator;
extern crate alloc;

pub struct Client<'a> {
    seqno: AtomicUsize,
    sender: Mutex<Sender<'a, MetaRequest>>,
    receiver: Mutex<Receiver<'a, MetaResponse>>,
    allocator: &'a BuddyAllocator<'a>,
}

impl<'a> Client<'a> {
    fn seqno(&self) -> usize {
        self.seqno.fetch_add(1, Ordering::AcqRel)
    }

    pub fn new(endpoint: Endpoint<'a, MetaRequest, MetaResponse>) -> Self {
        let allocator: &'a BuddyAllocator<'a> = endpoint.allocator();
        let (sender, receiver) = endpoint.into_sender_receiver();
        Client {
            seqno: AtomicUsize::new(0),
            sender: Mutex::new(sender),
            receiver: Mutex::new(receiver),
            allocator,
        }
    }

    fn send(&self, message: Request) -> Result<usize> {
        let seqno = self.seqno();
        let mut tx = self.sender.lock().unwrap();
        tx.send(MetaRequest {
            seqno,
            body: message,
        })?;
        Ok(seqno)
    }

    fn recv(&self, seqno: usize) -> Result<Response> {
        let mut rx = self.receiver.lock().unwrap();
        let expected = seqno;
        let MetaResponse { seqno, body } = rx.recv()?;
        assert_eq!(expected, seqno);
        Ok(body)
    }

    fn fullsend(&self, message: Request) -> Result<Response> {
        let seqno = self.send(message)?;
        self.recv(seqno)
    }

    pub fn null<'client>(&'client self) -> Result<Handle<'a, 'client, Null>> {
        let Response::Handle(index) = self.fullsend(Request::CreateNull)? else {
            unreachable!();
        };
        Ok(Handle {
            index,
            client: self,
            _phantom: PhantomData,
        })
    }

    pub fn word<'client>(&'client self, value: u64) -> Result<Handle<'a, 'client, Word>> {
        let Response::Handle(index) = self.fullsend(Request::CreateWord { value })? else {
            unreachable!();
        };
        Ok(Handle {
            index,
            client: self,
            _phantom: PhantomData,
        })
    }

    pub fn blob<'client, T: AsRef<[u8]>>(
        &'client self,
        data: T,
    ) -> Result<Handle<'a, 'client, Blob>> {
        let data = data.as_ref();
        let mut blob = Box::new_uninit_slice_in(data.len(), self.allocator);
        blob.write_copy_of_slice(data);
        let blob = unsafe { blob.assume_init() };
        let blob = Arc::from(blob);
        let ptr: *const [u8] = Arc::into_raw(blob);
        let (ptr, len) = ptr.to_raw_parts();
        let ptr = self.allocator.to_offset(ptr);
        let Response::Handle(index) = self.fullsend(Request::CreateBlob { ptr, len })? else {
            unreachable!();
        };
        Ok(Handle {
            index,
            client: self,
            _phantom: PhantomData,
        })
    }
}

pub struct Null;
pub struct Word;
pub struct Blob;
pub struct Thunk;
pub struct Opaque;

pub struct Handle<'a, 'client, T> {
    client: &'client Client<'a>,
    index: usize,
    _phantom: PhantomData<T>,
}

impl<T> Handle<'_, '_, T> {
    pub fn index(&self) -> usize {
        self.index
    }
}

impl Handle<'_, '_, Word> {
    pub fn read(&self) -> Result<u64> {
        let Response::Word(x) = self.client.fullsend(Request::Read { src: self.index })? else {
            panic!();
        };
        Ok(x)
    }
}

impl<'a, 'client> Handle<'a, 'client, Blob> {
    pub fn create_thunk(self) -> Result<Handle<'a, 'client, Thunk>> {
        let Response::Handle(index) = self
            .client
            .fullsend(Request::CreateThunk { src: self.index })?
        else {
            unreachable!();
        };
        let new = Handle {
            client: self.client,
            index,
            _phantom: PhantomData,
        };
        core::mem::forget(self);
        Ok(new)
    }
}

impl<'a, 'client> Handle<'a, 'client, Thunk> {
    pub fn run(self) -> Result<Handle<'a, 'client, Opaque>> {
        let Response::Handle(index) = self.client.fullsend(Request::Run { src: self.index })?
        else {
            unreachable!();
        };
        let new = Handle {
            client: self.client,
            index,
            _phantom: PhantomData,
        };
        core::mem::forget(self);
        Ok(new)
    }
}

impl<T> Clone for Handle<'_, '_, T> {
    fn clone(&self) -> Self {
        let Ok(Response::Handle(index)) = self.client.fullsend(Request::Clone { src: self.index })
        else {
            unreachable!();
        };
        Handle {
            client: self.client,
            index,
            _phantom: PhantomData,
        }
    }
}

impl<T> Drop for Handle<'_, '_, T> {
    fn drop(&mut self) {
        let _ = self.client.fullsend(Request::Drop { src: self.index });
    }
}
