#![no_main]
#![no_std]
#![feature(ptr_metadata)]
#![feature(allocator_api)]
#![allow(unused)]

use alloc::vec::Vec;
use alloc::{boxed::Box, sync::Arc};
use common::message::{Handle, MetaRequest, MetaResponse, PageTableEntry, Request, Response};
use common::ringbuffer::{Endpoint, EndpointRawData, Error as RingbufferError, Receiver, Sender};
use macros::kmain;

use kernel::prelude::*;
use kernel::rt;
use kernel::rt::profile;

extern crate alloc;

pub struct Server {
    sender: SpinLock<Sender<MetaResponse>>,
    receiver: SpinLock<Receiver<MetaRequest>>,
}

impl Server {
    pub fn new(endpoint: Endpoint<MetaResponse, MetaRequest>) -> Self {
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
                Err(RingbufferError::WouldBlock) => {
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
        let MetaRequest {
            function,
            context,
            body,
        } = request;
        let reply = |response| {
            self.reply(function, context, response);
        };
        let allocator = BuddyAllocator;
        reply(match body {
            Request::Nop => todo!(),
            Request::CreateError(handle) => {
                Response::Handle(Value::Error(Error::new(handle.into())).into())
            }
            Request::CreateAtom { ptr, len } => {
                let ptr: *mut u8 = allocator.from_offset(ptr);
                todo!();
                // let blob: Box<[u8]> = Box::from_raw(core::ptr::from_raw_parts_mut(ptr, len));
                // Response::Handle(Value::Atom(Atom::new(blob).into()).into())
            }
            Request::CreateBlob { ptr, len } => {
                let ptr: *mut u8 = allocator.from_offset(ptr);
                let blob: Box<[u8]> = Box::from_raw(core::ptr::from_raw_parts_mut(ptr, len));
                Response::Handle(Value::Blob(Blob::new(blob)).into())
            }
            Request::CreateTree { size } => {
                Response::Handle(Value::Tree(Tree::new_with_len(size)).into())
            }
            Request::CreatePage { size } => Response::Handle(Value::Page(Page::new(size)).into()),
            Request::CreateTable { size } => {
                Response::Handle(Value::Table(Table::new(size)).into())
            }
            Request::CreateLambda { thunk, index } => {
                todo!();
            }
            Request::CreateThunk {
                registers,
                memory,
                descriptors,
            } => {
                let Value::Blob(registers) = registers.into() else {
                    unreachable!();
                };
                let Value::Table(memory) = memory.into() else {
                    unreachable!();
                };
                let Value::Tree(descriptors) = descriptors.into() else {
                    unreachable!();
                };
                let registers: Vec<u64> = registers
                    .chunks(8)
                    .map(|x| u64::from_ne_bytes(x.try_into().unwrap()))
                    .collect();
                let mut register_file = RegisterFile::new();
                for (i, x) in registers.iter().take(18).enumerate() {
                    register_file[i] = *x;
                }
                let arca = Arca::new_with(register_file, memory, descriptors);
                Response::Handle(Value::Thunk(Thunk::new(arca)).into())
            }
            Request::Run(handle) => {
                let Value::Thunk(thunk) = handle.into() else {
                    unreachable!();
                };
                let result = thunk.run();
                Response::Handle(result.into())
            }
            Request::Apply(lambda, argument) => {
                let Value::Lambda(lambda) = lambda.into() else {
                    unreachable!();
                };
                let argument: Value = argument.into();
                let thunk = lambda.apply(argument);
                Response::Handle(Value::Thunk(thunk).into())
            }
            Request::Clone(handle) => {
                let value: Value = handle.into();
                let clone = value.clone();
                core::mem::forget(value);
                Response::Handle(clone.into())
            }
            Request::Drop(handle) => {
                let _: Value = handle.into();
                Response::Ack
            }
            Request::Type(handle) => {
                let value: Value = handle.into();
                let dt = value.datatype();
                core::mem::forget(value);
                Response::Type(dt)
            }
            Request::TreePut(tree, index, argument) => {
                let Value::Tree(mut tree) = tree.into() else {
                    unreachable!();
                };
                let old = tree.put(index, argument.into());
                core::mem::forget(tree);
                Response::Handle(old.into())
            }
            Request::TablePut(table, index, entry) => {
                let Value::Table(mut table) = table.into() else {
                    unreachable!();
                };
                let old = match entry {
                    Some((false, value)) if value.datatype() == DataType::Page => table
                        .put(
                            index,
                            arca::Entry::ROPage(
                                value.try_into().unwrap_or_else(|_| unreachable!()),
                            ),
                        )
                        .unwrap_or_else(|_| unreachable!()),
                    Some((true, value)) if value.datatype() == DataType::Page => table
                        .put(
                            index,
                            arca::Entry::RWPage(
                                value.try_into().unwrap_or_else(|_| unreachable!()),
                            ),
                        )
                        .unwrap_or_else(|_| unreachable!()),
                    Some((false, value)) if value.datatype() == DataType::Table => table
                        .put(
                            index,
                            arca::Entry::ROTable(
                                value.try_into().unwrap_or_else(|_| unreachable!()),
                            ),
                        )
                        .unwrap_or_else(|_| unreachable!()),
                    Some((true, value)) if value.datatype() == DataType::Table => table
                        .put(
                            index,
                            arca::Entry::RWTable(
                                value.try_into().unwrap_or_else(|_| unreachable!()),
                            ),
                        )
                        .unwrap_or_else(|_| unreachable!()),
                    None => table
                        .put(index, arca::Entry::Null(Null))
                        .unwrap_or_else(|_| unreachable!()),
                    _ => unreachable!(),
                };
                let old = match old {
                    arca::Entry::Null(_) => None,
                    arca::Entry::ROPage(page) => Some((false, page.into())),
                    arca::Entry::RWPage(page) => Some((true, page.into())),
                    arca::Entry::ROTable(table) => Some((false, table.into())),
                    arca::Entry::RWTable(table) => Some((true, table.into())),
                };
                core::mem::forget(table);
                Response::Entry(old)
            }
            Request::TableTake(table, index) => {
                let Value::Table(mut table) = table.into() else {
                    unreachable!();
                };
                let old = table.take(index);
                let old = match old {
                    arca::Entry::Null(_) => None,
                    arca::Entry::ROPage(page) => Some((false, page.into())),
                    arca::Entry::RWPage(page) => Some((true, page.into())),
                    arca::Entry::ROTable(table) => Some((false, table.into())),
                    arca::Entry::RWTable(table) => Some((true, table.into())),
                };
                core::mem::forget(table);
                Response::Entry(old)
            }
            Request::ReadBlob(handle) => {
                let Value::Blob(blob) = handle.into() else {
                    unreachable!();
                };
                let ptr = blob.as_ptr();
                let len = blob.len();
                let ptr = BuddyAllocator.to_offset(ptr);
                core::mem::forget(blob);
                Response::Span { ptr, len }
            }
            Request::ReadPage(handle) => {
                let Value::Page(page) = handle.into() else {
                    unreachable!();
                };
                let ptr = page.as_ptr();
                let len = page.len();
                let ptr = BuddyAllocator.to_offset(ptr);
                core::mem::forget(page);
                Response::Span { ptr, len }
            }
            Request::WritePage {
                handle,
                offset,
                ptr,
                len,
            } => {
                let Value::Page(mut page) = handle.into() else {
                    unreachable!();
                };
                let ptr: *mut u8 = allocator.from_offset(ptr);
                let blob: Box<[u8]> = Box::from_raw(core::ptr::from_raw_parts_mut(ptr, len));
                page.write(offset, &blob);
                core::mem::forget(page);
                Response::Ack
            }
            Request::Length(handle) => {
                let value = Value::from(handle);
                let size = match &value {
                    Value::Null => unreachable!(),
                    Value::Word(word) => unreachable!(),
                    Value::Atom(atom) => unreachable!(),
                    Value::Error(error) => unreachable!(),
                    Value::Blob(blob) => blob.len(),
                    Value::Tree(tree) => tree.len(),
                    Value::Page(page) => page.size(),
                    Value::Table(table) => table.size(),
                    Value::Lambda(lambda) => unreachable!(),
                    Value::Thunk(thunk) => unreachable!(),
                };
                core::mem::forget(value);
                Response::Length(size)
            }
        })
    }
}

#[kmain]
async fn kmain(argv: &[usize]) {
    let ring_buffer_data_ptr = argv[0];
    let server = unsafe {
        let raw_rb_data =
            Box::from_raw(BuddyAllocator.from_offset::<EndpointRawData>(ring_buffer_data_ptr));
        let endpoint = Endpoint::from_raw_parts(&raw_rb_data);
        core::mem::forget(raw_rb_data);
        Box::leak(Box::new(Server::new(endpoint)))
    };
    kernel::profile::begin();
    server.run().await;
    kernel::profile::end();
    profile();
}
