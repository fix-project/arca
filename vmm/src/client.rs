use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use common::message::{MetaRequest, MetaResponse, Request, Response};
use common::ringbuffer::{Endpoint, Receiver, Sender};
use common::BuddyAllocator;
extern crate alloc;

pub struct Client<'a> {
    port: AtomicUsize,
    buffer: Mutex<HashMap<usize, VecDeque<Response>>>,
    sender: Mutex<Sender<'a, MetaRequest>>,
    receiver: Mutex<Receiver<'a, MetaResponse>>,
    _allocator: &'a BuddyAllocator<'a>,
}

impl<'a> Client<'a> {
    pub fn new(endpoint: Endpoint<'a, MetaRequest, MetaResponse>) -> Self {
        let allocator: &'a BuddyAllocator<'a> = endpoint.allocator();
        let (sender, receiver) = endpoint.into_sender_receiver();
        Client {
            port: AtomicUsize::new(0),
            buffer: Mutex::new(HashMap::new()),
            sender: Mutex::new(sender),
            receiver: Mutex::new(receiver),
            _allocator: allocator,
        }
    }

    pub fn port<'client>(&'client self) -> Port<'a, 'client> {
        let port = self.port.fetch_add(1, Ordering::SeqCst);
        let mut tx = self.sender.lock().unwrap();
        let size = 1024;
        let mut assigned = Vec::with_capacity(size);
        assigned.resize(size, false);
        tx.send(MetaRequest::OpenPort { port, size }).unwrap();
        Port {
            client: self,
            assigned: Mutex::new(assigned),
            port,
        }
    }

    fn send(&self, port: usize, message: Request) {
        let mut tx = self.sender.lock().unwrap();
        tx.send(MetaRequest::Request {
            port,
            body: message,
        })
        .unwrap();
    }

    fn recv(&self, port: usize) -> Response {
        loop {
            if let Ok(mut rx) = self.receiver.try_lock() {
                let result = rx.recv().unwrap();
                match result {
                    MetaResponse::Response { port, body } => {
                        let mut buffer = self.buffer.lock().unwrap();
                        buffer.entry(port).or_default().push_back(body);
                    }
                    MetaResponse::Exit => todo!(),
                }
            }
            if let Ok(mut buffer) = self.buffer.try_lock() {
                let port = buffer.entry(port).or_default();
                if let Some(front) = port.pop_front() {
                    return front;
                }
            }
            core::hint::spin_loop();
        }
    }
}

impl Drop for Client<'_> {
    fn drop(&mut self) {
        let mut tx = self.sender.lock().unwrap();
        tx.send(MetaRequest::Exit).unwrap();
    }
}

pub struct Port<'a, 'client> {
    client: &'client Client<'a>,
    assigned: Mutex<Vec<bool>>,
    port: usize,
}

impl<'a, 'client> Port<'a, 'client> {
    fn send(&self, message: Request) {
        self.client.send(self.port, message);
    }

    fn recv(&self) -> Response {
        self.client.recv(self.port)
    }

    fn assign(&self) -> usize {
        let mut assigned = self.assigned.lock().unwrap();
        if let Some(i) = assigned.iter().position(|x| !x) {
            assigned[i] = true;
            i
        } else {
            let i = assigned.len();
            let size = 2 * i;
            self.send(Request::Resize { size });
            assigned.resize(size, false);
            assigned[i] = true;
            i
        }
    }

    pub fn null<'port>(&'port self) -> Handle<'a, 'port, 'client, Null> {
        let i = self.assign();
        self.send(Request::Null { dst: i });
        Handle {
            idx: i,
            port: self,
            _phantom: PhantomData,
        }
    }

    pub fn word<'port>(&'port self, value: u64) -> Handle<'a, 'port, 'client, Word> {
        let i = self.assign();
        self.send(Request::CreateWord { dst: i, value });
        Handle {
            idx: i,
            port: self,
            _phantom: PhantomData,
        }
    }
}

impl Drop for Port<'_, '_> {
    fn drop(&mut self) {
        let mut tx = self.client.sender.lock().unwrap();
        tx.send(MetaRequest::ClosePort { port: self.port }).unwrap();
    }
}

pub struct Null;
pub struct Word;

pub struct Handle<'a, 'port, 'client, T> {
    port: &'port Port<'a, 'client>,
    idx: usize,
    _phantom: PhantomData<T>,
}

impl<T> Handle<'_, '_, '_, T> {
    pub fn index(&self) -> usize {
        self.idx
    }
}

impl Handle<'_, '_, '_, Word> {
    pub fn read(&self) -> u64 {
        self.port.send(Request::Read { src: self.idx });
        let Response::Word(x) = self.port.recv() else {
            panic!();
        };
        x
    }
}

impl<T> Drop for Handle<'_, '_, '_, T> {
    fn drop(&mut self) {
        self.port.send(Request::Drop { dst: self.idx });
        self.port.assigned.lock().unwrap()[self.idx] = false;
    }
}
