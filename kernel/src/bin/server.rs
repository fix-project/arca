#![no_main]
#![no_std]
#![feature(ptr_metadata)]

use alloc::vec::Vec;
use alloc::{boxed::Box, sync::Arc};
use common::message::{Handle, MetaRequest, MetaResponse, Request, Response};
use common::ringbuffer::{Endpoint, EndpointRawData, Error, Receiver, Sender};
use macros::kmain;

use kernel::prelude::*;
use kernel::rt;
use kernel::rt::profile;

extern crate alloc;

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

    #[inline(never)]
    pub async fn run(&'static self) {
        let mut rx = self.receiver.lock();
        loop {
            let attempt = rx.try_recv();
            let request = match attempt {
                Ok(result) => result,
                Err(Error::WouldBlock) => {
                    kernel::rt::yield_now().await;
                    continue;
                }
                Err(_) => {
                    break;
                }
            };
            // kernel::rt::spawn(async move {
            unsafe {
                self.handle(request);
            }
            // });
        }
    }

    fn reply(&self, function: usize, context: usize, body: Response) {
        let mut tx = self.sender.lock();
        let _ = tx.send(MetaResponse {
            function,
            context,
            body,
        });
    }

    unsafe fn handle(&'static self, request: MetaRequest) {
        // log::debug!("got message {message:?}");
        let allocator = &*PHYSICAL_ALLOCATOR;
        let MetaRequest {
            function,
            context,
            body,
        } = request;
        let reply = |response| {
            self.reply(function, context, response);
        };
        match body {
            Request::Nop => todo!(),
            Request::CreateBlob { ptr, len } => {
                let ptr: *const u8 = allocator.from_offset(ptr);
                let blob: Arc<[u8]> = Arc::from_raw(core::ptr::from_raw_parts(ptr, len));
                reply(Response::Handle(Value::Blob(blob).into()));
            }
            Request::CreateTree { ptr, len } => {
                let allocator = &*PHYSICAL_ALLOCATOR;
                let ptr: *mut usize = allocator.from_offset(ptr);
                let elements: Box<[Handle]> =
                    Box::from_raw(core::ptr::from_raw_parts_mut(ptr, len));
                let mut v = Vec::with_capacity(elements.len());
                let elements = Vec::from(elements);
                for handle in elements.into_iter() {
                    v.push(handle.into());
                }
                let value = Value::Tree(v.into());
                reply(Response::Handle(value.into()));
            }
            Request::LoadElf(handle) => {
                let Value::Blob(blob) = handle.into() else {
                    unreachable!();
                };
                let thunk = Thunk::from_elf(&blob);
                reply(Response::Handle(Value::Thunk(thunk).into()));
            }
            Request::Run(handle) => {
                let Value::Thunk(thunk) = handle.into() else {
                    unreachable!();
                };
                let result = thunk.run();
                reply(Response::Handle(result.into()));
            }
            Request::Apply(lambda, argument) => {
                let Value::Lambda(lambda) = lambda.into() else {
                    unreachable!();
                };
                let argument: Value = argument.into();
                let thunk = lambda.apply(argument);
                reply(Response::Handle(Value::Thunk(thunk).into()));
            }
            Request::Drop(handle) => {
                let _: Value = handle.into();
                reply(Response::Ack);
            }
            _ => todo!("handling {body:?}"),
        }
        // Request::CreateNull => {
        //     let dst = self.encode(Value::Null.into());
        //     self.reply(seqno, Response::Handle(dst));
        // }
        // Request::CreateWord { value } => {
        //     let dst = self.encode(Value::Word(value).into());
        //     self.reply(seqno, Response::Handle(dst));
        // }
        // Request::CreateAtom { ptr, len } => todo!(),
        // Request::CreateBlob { ptr, len } => {
        //     let allocator = &*PHYSICAL_ALLOCATOR;
        //     let dst = unsafe {
        //         let ptr: *const u8 = allocator.from_offset(ptr);
        //         let blob = Arc::from_raw(core::ptr::from_raw_parts(ptr, len));
        //         let value = Value::Blob(blob);
        //         self.encode(value.into())
        //     };
        //     self.reply(seqno, Response::Handle(dst));
        // }
        // Request::CreateTree { ptr, len } => {
        //     // crate::rt::spawn(async move {
        //     let allocator = &*PHYSICAL_ALLOCATOR;
        //     let dst = unsafe {
        //         let ptr: *const usize = allocator.from_offset(ptr);
        //         let elements: Arc<[usize]> = Arc::from_raw(core::ptr::from_raw_parts(ptr, len));
        //         let mut v = Vec::with_capacity(elements.len());
        //         for index in &*elements {
        //             let element = *self.decode(*index);
        //             v.push(element);
        //         }
        //         let value = Value::Tree(v.into());
        //         self.encode(value.into())
        //     };
        //     self.reply(seqno, Response::Handle(dst));
        //     // });
        // }
        // Request::CreateThunk { src } => {
        //     let Value::Blob(blob) = *self.decode(src) else {
        //         todo!();
        //     };
        //     // crate::rt::spawn(async move {
        //     let thunk = Thunk::from_elf(&blob);
        //     let src = self.encode(Value::Thunk(thunk).into());
        //     self.reply(seqno, Response::Handle(src));
        //     // });
        // }
        // Request::GetType { src } => self.reply(
        //     seqno,
        //     Response::Type(match &self.peek(src) {
        //         Value::Null => Type::Null,
        //         Value::Error(value) => todo!(),
        //         Value::Word(_) => Type::Word,
        //         Value::Atom(_) => todo!(),
        //         Value::Blob(items) => Type::Blob,
        //         Value::Tree(values) => Type::Tree,
        //         Value::Page(page) => todo!(),
        //         Value::PageTable(page_table) => todo!(),
        //         Value::Lambda(lambda) => Type::Lambda,
        //         Value::Thunk(thunk) => Type::Thunk,
        //     }),
        // ),
        // Request::Read { src } => match &self.peek(src) {
        //     Value::Null => self.reply(seqno, Response::Null),
        //     Value::Error(value) => todo!(),
        //     Value::Word(word) => self.reply(seqno, Response::Word(*word)),
        //     Value::Atom(_) => todo!(),
        //     Value::Blob(items) => todo!(),
        //     Value::Tree(values) => todo!(),
        //     Value::Page(page) => todo!(),
        //     Value::PageTable(page_table) => todo!(),
        //     Value::Lambda(lambda) => todo!(),
        //     Value::Thunk(thunk) => todo!(),
        // },
        // Request::Apply { src, arg } => {
        //     let f = *self.decode(src);
        //     let Value::Lambda(lambda) = f else {
        //         todo!("using {f:?} as a function");
        //     };
        //     let x = self.decode(arg);
        //     let thunk = lambda.apply(*x);
        //     let dst = self.encode(Value::Thunk(thunk).into());
        //     self.reply(seqno, Response::Handle(dst));
        // }
        // Request::Run { src } => {
        //     let Value::Thunk(thunk) = *self.decode(src) else {
        //         todo!();
        //     };
        //     // crate::rt::spawn(async move {
        //     let y = thunk.run_for(Duration::from_millis(1000));
        //     log::info!("ran thunk, got {y:#x?}");
        //     let dst = self.encode(y.into());
        //     self.reply(seqno, Response::Handle(dst));
        //     // });
        // }
        // Request::Clone { src } => {
        //     // crate::rt::spawn(async move {
        //     let original = self.peek(src);
        //     let new = original.clone();
        //     let dst = self.encode(new.into());
        //     self.reply(seqno, Response::Handle(dst));
        //     // });
        // }
        // Request::Drop { src } => {
        //     let current = self.decode(src);
        //     // crate::rt::spawn(async move {
        //     core::mem::drop(current);
        //     self.reply(seqno, Response::Ack);
        //     // });
        // }
        // }
    }
}

#[kmain]
async fn kmain(argv: &[usize]) {
    let ring_buffer_data_ptr = argv[0];
    let server = unsafe {
        let raw_rb_data =
            Box::from_raw(PHYSICAL_ALLOCATOR.from_offset::<EndpointRawData>(ring_buffer_data_ptr));
        let endpoint = Endpoint::from_raw_parts(&raw_rb_data, &PHYSICAL_ALLOCATOR);
        core::mem::forget(raw_rb_data);
        Box::leak(Box::new(Server::new(endpoint)))
    };
    kernel::profile::begin();
    server.run().await;
    kernel::profile::end();
    profile();
}
