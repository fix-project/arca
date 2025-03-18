use common::{
    message::{MetaRequest, MetaResponse, Request, Response},
    ringbuffer::{Endpoint, Error, Receiver, Sender},
};

use crate::prelude::*;

extern crate alloc;

pub static SERVER: OnceLock<Server> = OnceLock::new();

pub struct Server {
    sender: SpinLock<Sender<'static, MetaResponse>>,
    receiver: SpinLock<Receiver<'static, MetaRequest>>,
}

impl Server {
    pub fn new(endpoint: Endpoint<'static, MetaResponse, MetaRequest>) -> Self {
        let (sender, receiver) = endpoint.into_sender_receiver();
        Server {
            sender: SpinLock::new(sender),
            receiver: SpinLock::new(receiver),
        }
    }

    pub async fn run(&'static self) {
        loop {
            let attempt = {
                let mut rx = self.receiver.lock();
                rx.try_recv()
            };
            let MetaRequest { seqno, body } = match attempt {
                Ok(result) => result,
                Err(Error::WouldBlock) => {
                    crate::rt::yield_now().await;
                    continue;
                }
                Err(_) => {
                    break;
                }
            };
            log::debug!("got request {seqno}: {body:?}");
            unsafe {
                self.handle(seqno, body);
            }
        }
    }

    fn reply(&self, seqno: usize, body: Response) {
        let mut tx = self.sender.lock();
        let _ = tx.send(MetaResponse { seqno, body });
    }

    unsafe fn encode(&self, value: Box<Value>) -> usize {
        let raw = Box::into_raw(value);
        PHYSICAL_ALLOCATOR.to_offset(raw)
    }

    unsafe fn peek(&self, handle: usize) -> &Value {
        &*PHYSICAL_ALLOCATOR.from_offset(handle)
    }

    unsafe fn decode(&self, handle: usize) -> Box<Value> {
        Box::from_raw(PHYSICAL_ALLOCATOR.from_offset(handle))
    }

    unsafe fn handle(&'static self, seqno: usize, message: Request) {
        log::debug!("got message {message:?}");
        #[allow(unused)]
        match message {
            Request::Nop => todo!(),
            Request::CreateNull => {
                let dst = self.encode(Value::Null.into());
                self.reply(seqno, Response::Handle(dst));
            }
            Request::CreateWord { value } => {
                let dst = self.encode(Value::Word(value).into());
                self.reply(seqno, Response::Handle(dst));
            }
            Request::CreateAtom { ptr, len } => todo!(),
            Request::CreateBlob { ptr, len } => {
                let allocator = &*PHYSICAL_ALLOCATOR;
                let dst = unsafe {
                    let ptr: *const u8 = allocator.from_offset(ptr);
                    let blob = Arc::from_raw(core::ptr::from_raw_parts(ptr, len));
                    let value = Value::Blob(blob);
                    self.encode(value.into())
                };
                self.reply(seqno, Response::Handle(dst));
            }
            Request::CreateTree { ptr, len } => todo!(),
            Request::CreateThunk { src } => {
                let Value::Blob(blob) = *self.decode(src) else {
                    todo!();
                };
                // crate::rt::spawn(async move {
                let thunk = Thunk::from_elf(&blob);
                let src = self.encode(Value::Thunk(thunk).into());
                self.reply(seqno, Response::Handle(src));
                // });
            }
            Request::Read { src } => match &self.peek(src) {
                Value::Null => self.reply(seqno, Response::Null),
                Value::Error(value) => todo!(),
                Value::Word(word) => self.reply(seqno, Response::Word(*word)),
                Value::Atom(_) => todo!(),
                Value::Blob(items) => todo!(),
                Value::Tree(values) => todo!(),
                Value::Page(page) => todo!(),
                Value::PageTable(page_table) => todo!(),
                Value::Lambda(lambda) => todo!(),
                Value::Thunk(thunk) => todo!(),
            },
            Request::Apply { src, arg } => {
                let Value::Lambda(lambda) = *self.decode(src) else {
                    todo!();
                };
                let x = self.decode(arg);
                let thunk = lambda.apply(*x);
                let dst = self.encode(Value::Thunk(thunk).into());
                self.reply(seqno, Response::Handle(dst));
            }
            Request::Run { src } => {
                let Value::Thunk(thunk) = *self.decode(src) else {
                    todo!();
                };
                // crate::rt::spawn(async move {
                let y = thunk.run_on_this_cpu();
                let dst = self.encode(y.into());
                self.reply(seqno, Response::Handle(dst));
                // });
            }
            Request::Clone { src } => {
                // crate::rt::spawn(async move {
                let original = self.peek(src);
                let new = original.clone();
                let dst = self.encode(new.into());
                self.reply(seqno, Response::Handle(dst));
                // });
            }
            Request::Drop { src } => {
                let current = self.decode(src);
                // crate::rt::spawn(async move {
                core::mem::drop(current);
                self.reply(seqno, Response::Ack);
                // });
            }
        }
    }
}
