use alloc::collections::btree_map::BTreeMap;
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
    ports: SpinLock<BTreeMap<usize, Port<'a>>>,
    sender: SpinLock<Sender<'a, MetaResponse>>,
    receiver: SpinLock<Receiver<'a, MetaRequest>>,
    _allocator: &'a BuddyAllocator<'a>,
}

impl<'a> Server<'a> {
    pub fn new(endpoint: Endpoint<'a, MetaResponse, MetaRequest>) -> Self {
        let allocator: &'a BuddyAllocator<'a> = endpoint.allocator();
        let (sender, receiver) = endpoint.into_sender_receiver();
        Server {
            ports: SpinLock::new(BTreeMap::new()),
            sender: SpinLock::new(sender),
            receiver: SpinLock::new(receiver),
            _allocator: allocator,
        }
    }

    pub fn run(&'a self, _cpu: &mut Cpu) {
        log::info!("serving requests");
        loop {
            let request = {
                let mut rx = self.receiver.lock();
                rx.recv().unwrap()
            };
            log::debug!("got request {request:?}");
            match request {
                MetaRequest::OpenPort { port, size } => {
                    let mut descriptors = Vec::with_capacity(size);
                    descriptors.resize(size, Value::Null);
                    let mut ports = self.ports.lock();
                    ports.insert(
                        port,
                        Port {
                            _server: self,
                            port,
                            descriptors,
                        },
                    );
                }
                MetaRequest::Request { port, body } => {
                    // TODO: don't block everything while this is happening
                    let mut ports = self.ports.lock();
                    let mut current = ports.remove(&port).unwrap();
                    let response = current.handle(body);
                    ports.insert(port, current);
                    if let Some(response) = response {
                        let mut tx = self.sender.lock();
                        tx.send(MetaResponse::Response {
                            port,
                            body: response,
                        })
                        .unwrap();
                    }
                }
                MetaRequest::ClosePort { port } => {
                    let mut ports = self.ports.lock();
                    ports.remove(&port);
                }
                MetaRequest::Exit => return,
            }
        }
    }
}

impl Drop for Server<'_> {
    fn drop(&mut self) {
        let mut tx = self.sender.lock();
        tx.send(MetaResponse::Exit).unwrap();
    }
}

pub struct Port<'a> {
    _server: &'a Server<'a>,
    port: usize,
    descriptors: Vec<Value>,
}

impl Port<'_> {
    fn handle(&mut self, message: Request) -> Option<Response> {
        log::debug!("got message {message:?} on port {}", self.port);
        #[allow(unused)]
        match message {
            Request::Nop => todo!(),
            Request::Resize { size } => {
                self.descriptors.resize(size, Value::Null);
                None
            }
            Request::Null { dst } => {
                self.descriptors[dst] = Value::Null;
                None
            }
            Request::CreateWord { dst, value } => {
                self.descriptors[dst] = Value::Word(value);
                None
            }
            Request::CreateAtom { dst, ptr, len } => todo!(),
            Request::CreateBlob { dst, ptr, len } => todo!(),
            Request::CreateTree { dst, ptr, len } => todo!(),
            Request::CreateThunk { dst, src } => todo!(),
            Request::Read { src } => Some(match &self.descriptors[src] {
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
            }),
            Request::Apply { dst, src } => todo!(),
            Request::Run { dst, src } => todo!(),
            Request::Clone { dst, src } => todo!(),
            Request::Drop { dst } => {
                core::mem::take(&mut self.descriptors[dst]);
                None
            }
        }
    }
}
