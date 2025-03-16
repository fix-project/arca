use common::{
    message::{MetaRequest, MetaResponse, Request, Response},
    ringbuffer::{Endpoint, Error, Receiver, Sender},
};

use crate::prelude::*;

extern crate alloc;

pub static SERVER: OnceLock<Server> = OnceLock::new();

pub struct Server {
    assigned: SpinLock<Vec<bool>>,
    descriptors: SpinLock<Vec<Value>>,
    sender: SpinLock<Sender<'static, MetaResponse>>,
    receiver: SpinLock<Receiver<'static, MetaRequest>>,
}

impl Server {
    pub fn new(endpoint: Endpoint<'static, MetaResponse, MetaRequest>) -> Self {
        let (sender, receiver) = endpoint.into_sender_receiver();
        Server {
            assigned: SpinLock::new(Vec::new()),
            descriptors: SpinLock::new(Vec::new()),
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
                    core::hint::spin_loop();
                    crate::rt::yield_now().await;
                    continue;
                }
                Err(_) => {
                    return;
                }
            };
            log::debug!("got request {seqno}: {body:?}");
            crate::rt::spawn(async move {
                self.handle(seqno, body);
            });
        }
    }

    fn assign(&self) -> usize {
        let mut assigned = self.assigned.lock();
        if let Some(i) = assigned.iter().position(|x| !x) {
            assigned[i] = true;
            i
        } else {
            let i = assigned.len();
            let size = 2 * i;
            let size = core::cmp::max(size, 1024);
            let mut descriptors = self.descriptors.lock();
            descriptors.resize(size, Value::Null);
            assigned.resize(size, false);
            assigned[i] = true;
            i
        }
    }

    fn reply(&self, seqno: usize, body: Response) {
        let mut tx = self.sender.lock();
        let _ = tx.send(MetaResponse { seqno, body });
    }

    fn unassign(&self, i: usize) {
        self.descriptors.lock()[i] = Value::Null;
        self.assigned.lock()[i] = false;
    }

    fn handle(&'static self, seqno: usize, message: Request) {
        log::debug!("got message {message:?}");
        #[allow(unused)]
        match message {
            Request::Nop => todo!(),
            Request::CreateNull => {
                let dst = self.assign();
                self.descriptors.lock()[dst] = Value::Null;
                self.reply(seqno, Response::Handle(dst));
            }
            Request::CreateWord { value } => {
                let dst = self.assign();
                self.descriptors.lock()[dst] = Value::Word(value);
                self.reply(seqno, Response::Handle(dst));
            }
            Request::CreateAtom { ptr, len } => todo!(),
            Request::CreateBlob { ptr, len } => {
                let dst = self.assign();
                let allocator = &*PHYSICAL_ALLOCATOR;
                unsafe {
                    let ptr: *const u8 = allocator.from_offset(ptr);
                    let blob = Arc::from_raw(core::ptr::from_raw_parts(ptr, len));
                    self.descriptors.lock()[dst] = Value::Blob(blob);
                }
                self.reply(seqno, Response::Handle(dst));
            }
            Request::CreateTree { ptr, len } => todo!(),
            Request::CreateThunk { src } => {
                let Value::Blob(blob) = core::mem::take(&mut self.descriptors.lock()[src]) else {
                    todo!();
                };
                // crate::rt::spawn(async move {
                    let thunk = Thunk::from_elf(&blob);
                    self.descriptors.lock()[src] = Value::Thunk(thunk);
                    self.reply(seqno, Response::Handle(src));
                // });
            }
            Request::Read { src } => match &self.descriptors.lock()[src] {
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
                let Value::Lambda(lambda) = core::mem::take(&mut self.descriptors.lock()[src])
                else {
                    todo!();
                };
                let x = core::mem::take(&mut self.descriptors.lock()[arg]);
                let thunk = lambda.apply(x);
                self.descriptors.lock()[src] = Value::Thunk(thunk);
                self.reply(seqno, Response::Handle(src));
            }
            Request::Run { src } => {
                let Value::Thunk(thunk) = core::mem::take(&mut self.descriptors.lock()[src]) else {
                    todo!();
                };
                // crate::rt::spawn(async move {
                    let y = thunk.run_on_this_cpu();
                    self.descriptors.lock()[src] = y;
                    self.reply(seqno, Response::Handle(src));
                // });
            }
            Request::Clone { src } => {
                let dst = self.assign();
                // crate::rt::spawn(async move {
                    let mut descriptors = self.descriptors.lock();
                    let mut original = &descriptors[src];
                    let new = original.clone();
                    descriptors[dst] = new;
                    self.reply(seqno, Response::Handle(dst));
                // });
            }
            Request::Drop { src } => {
                let current = core::mem::take(&mut self.descriptors.lock()[src]);
                // crate::rt::spawn(async move {
                    core::mem::drop(current);
                    self.unassign(src);
                    self.reply(seqno, Response::Ack);
                // });
            }
        }
    }
}
