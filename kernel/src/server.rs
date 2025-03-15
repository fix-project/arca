use common::{
    message::{MetaRequest, MetaResponse, Request, Response},
    ringbuffer::{Endpoint, Receiver, Sender},
    util::spinlock::SpinLock,
    BuddyAllocator,
};

use crate::prelude::*;

extern crate alloc;

pub static SERVER: OnceLock<Server> = OnceLock::new();

pub struct Server<'a> {
    assigned: SpinLock<Vec<bool>>,
    descriptors: SpinLock<Vec<Value>>,
    sender: SpinLock<Sender<'a, MetaResponse>>,
    receiver: SpinLock<Receiver<'a, MetaRequest>>,
    _allocator: &'a BuddyAllocator<'a>,
}

impl<'a> Server<'a> {
    pub fn new(endpoint: Endpoint<'a, MetaResponse, MetaRequest>) -> Self {
        let allocator: &'a BuddyAllocator<'a> = endpoint.allocator();
        let (sender, receiver) = endpoint.into_sender_receiver();
        Server {
            assigned: SpinLock::new(Vec::new()),
            descriptors: SpinLock::new(Vec::new()),
            sender: SpinLock::new(sender),
            receiver: SpinLock::new(receiver),
            _allocator: allocator,
        }
    }

    pub fn run(&'a self, cpu: &mut Cpu) {
        loop {
            let result = {
                let mut rx = self.receiver.lock();
                rx.recv()
            };
            let Ok(MetaRequest { seqno, body }) = result else {
                return;
            };
            log::debug!("got request {seqno}: {body:?}");
            let body = self.handle(body, cpu);
            let mut tx = self.sender.lock();
            let result = tx.send(MetaResponse { seqno, body });
            if result.is_err() {
                return;
            }
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

    fn unassign(&self, i: usize) {
        self.descriptors.lock()[i] = Value::Null;
        self.assigned.lock()[i] = false;
    }

    fn handle(&self, message: Request, cpu: &mut Cpu) -> Response {
        log::debug!("got message {message:?}");
        #[allow(unused)]
        match message {
            Request::Nop => todo!(),
            Request::CreateNull => {
                let dst = self.assign();
                self.descriptors.lock()[dst] = Value::Null;
                Response::Handle(dst)
            }
            Request::CreateWord { value } => {
                let dst = self.assign();
                self.descriptors.lock()[dst] = Value::Word(value);
                Response::Handle(dst)
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
                Response::Handle(dst)
            }
            Request::CreateTree { ptr, len } => todo!(),
            Request::CreateThunk { src } => {
                let Value::Blob(blob) = core::mem::take(&mut self.descriptors.lock()[src]) else {
                    todo!();
                };
                let thunk = Thunk::from_elf(&blob);
                self.descriptors.lock()[src] = Value::Thunk(thunk);
                Response::Handle(src)
            }
            Request::Read { src } => match &self.descriptors.lock()[src] {
                Value::Null => Response::Null,
                Value::Error(value) => todo!(),
                Value::Word(word) => Response::Word(*word),
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
                Response::Handle(src)
            }
            Request::Run { src } => {
                let Value::Thunk(thunk) = core::mem::take(&mut self.descriptors.lock()[src]) else {
                    todo!();
                };
                let y = thunk.run(cpu);
                self.descriptors.lock()[src] = y;
                Response::Handle(src)
            }
            Request::Clone { src } => {
                let dst = self.assign();
                let mut descriptors = self.descriptors.lock();
                descriptors[dst] = descriptors[src].clone();
                Response::Handle(dst)
            }
            Request::Drop { src } => {
                core::mem::take(&mut self.descriptors.lock()[src]);
                self.unassign(src);
                Response::Ack
            }
        }
    }
}
