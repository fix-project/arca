#![no_main]
#![no_std]
#![feature(ptr_metadata)]
#![feature(allocator_api)]
#![allow(unused)]

use alloc::vec::Vec;
use alloc::{boxed::Box, sync::Arc};
use common::message::{Handle, MetaRequest, MetaResponse, PageTableEntry, Request, Response};
use common::ringbuffer::{Endpoint, EndpointRawData, Error, Receiver, Sender};
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
        todo!();
        // let allocator = &*PHYSICAL_ALLOCATOR;
        // let MetaRequest {
        //     function,
        //     context,
        //     body,
        // } = request;
        // let reply = |response| {
        //     self.reply(function, context, response);
        // };
        // reply(match body {
        //     Request::Nop => todo!(),
        //     Request::CreateError(handle) => {
        //         Response::Handle(Value::Error(Arc::new(handle.into())).into())
        //     }
        //     Request::CreateAtom { ptr, len } => {
        //         let ptr: *const u8 = allocator.from_offset(ptr);
        //         let blob: Arc<[u8]> = Arc::from_raw(core::ptr::from_raw_parts(ptr, len));
        //         Response::Handle(Value::Atom(Atom::new(blob).into()).into())
        //     }
        //     Request::CreateBlob { ptr, len } => {
        //         let ptr: *const u8 = allocator.from_offset(ptr);
        //         let blob: Arc<[u8]> = Arc::from_raw(core::ptr::from_raw_parts(ptr, len));
        //         Response::Handle(Value::Blob(blob).into())
        //     }
        //     Request::CreateTree { ptr, len } => {
        //         let ptr: *mut usize = allocator.from_offset(ptr);
        //         let elements: Box<[Handle]> =
        //             Box::from_raw(core::ptr::from_raw_parts_mut(ptr, len));
        //         let mut v = Vec::with_capacity(elements.len());
        //         let elements = Vec::from(elements);
        //         for handle in elements.into_iter() {
        //             v.push(handle.into());
        //         }
        //         let value = Value::Tree(v.into());
        //         Response::Handle(value.into())
        //     }
        //     Request::CreatePage { size } => Response::Handle(
        //         match size {
        //             val if val == 1 << 12 => Value::Page(DynPage::Page4KB(CowPage::new()).into()),
        //             val if val == 1 << 21 => Value::Page(DynPage::Page2MB(CowPage::new()).into()),
        //             val if val == 1 << 30 => Value::Page(DynPage::Page1GB(CowPage::new()).into()),
        //             _ => unreachable!(),
        //         }
        //         .into(),
        //     ),
        //     Request::CreatePageTable { ptr, size } => {
        //         let ptr: *mut [PageTableEntry; 512] = allocator.from_offset(ptr);
        //         let entries: Box<[PageTableEntry; 512]> = Box::from_raw(ptr);
        //         Response::Handle(
        //             Value::PageTable(match size {
        //                 val if val == 1 << 21 => {
        //                     let mut pt = AugmentedPageTable::new();
        //                     for (i, entry) in entries.into_iter().enumerate() {
        //                         let value: Option<Value> = entry.handle.map(|handle| handle.into());
        //                         match value {
        //                             Some(Value::Page(page)) => match Arc::unwrap_or_clone(page) {
        //                                 DynPage::Page4KB(page) => {
        //                                     if entry.unique {
        //                                         pt.entry_mut(i).map_unique(page.unique());
        //                                     } else {
        //                                         pt.entry_mut(i).map_shared(page.shared());
        //                                     }
        //                                 }
        //                                 _ => unreachable!(),
        //                             },
        //                             _ => unreachable!(),
        //                         }
        //                     }
        //                     PageTable::PageTable2MB(CowPage::Unique(pt)).into()
        //                 }
        //                 val if val == 1 << 30 => {
        //                     let mut pt = AugmentedPageTable::new();
        //                     for (i, entry) in entries.into_iter().enumerate() {
        //                         let value: Option<Value> = entry.handle.map(|handle| handle.into());
        //                         match value {
        //                             Some(Value::Page(page)) => match Arc::unwrap_or_clone(page) {
        //                                 DynPage::Page2MB(page) => {
        //                                     if entry.unique {
        //                                         pt.entry_mut(i).map_unique(page.unique());
        //                                     } else {
        //                                         pt.entry_mut(i).map_shared(page.shared());
        //                                     }
        //                                 }
        //                                 _ => unreachable!(),
        //                             },
        //                             Some(Value::PageTable(page)) => {
        //                                 match Arc::unwrap_or_clone(page) {
        //                                     PageTable::PageTable2MB(page) => {
        //                                         if entry.unique {
        //                                             pt.entry_mut(i).chain_unique(page.unique());
        //                                         } else {
        //                                             pt.entry_mut(i).chain_shared(page.shared());
        //                                         }
        //                                     }
        //                                     _ => unreachable!(),
        //                                 }
        //                             }
        //                             _ => unreachable!(),
        //                         }
        //                     }
        //                     PageTable::PageTable1GB(CowPage::Unique(pt)).into()
        //                 }
        //                 val if val == 1 << 39 => {
        //                     let mut pt = AugmentedPageTable::new();
        //                     for (i, entry) in entries.into_iter().enumerate() {
        //                         let value: Option<Value> = entry.handle.map(|handle| handle.into());
        //                         match value {
        //                             Some(Value::Page(page)) => match Arc::unwrap_or_clone(page) {
        //                                 DynPage::Page1GB(page) => {
        //                                     if entry.unique {
        //                                         pt.entry_mut(i).map_unique(page.unique());
        //                                     } else {
        //                                         pt.entry_mut(i).map_shared(page.shared());
        //                                     }
        //                                 }
        //                                 _ => unreachable!(),
        //                             },
        //                             Some(Value::PageTable(page)) => {
        //                                 match Arc::unwrap_or_clone(page) {
        //                                     PageTable::PageTable1GB(page) => {
        //                                         if entry.unique {
        //                                             pt.entry_mut(i).chain_unique(page.unique());
        //                                         } else {
        //                                             pt.entry_mut(i).chain_shared(page.shared());
        //                                         }
        //                                     }
        //                                     _ => unreachable!(),
        //                                 }
        //                             }
        //                             _ => unreachable!(),
        //                         }
        //                     }
        //                     PageTable::PageTable512GB(CowPage::Unique(pt)).into()
        //                 }
        //                 _ => unreachable!(),
        //             })
        //             .into(),
        //         )
        //     }
        //     Request::CreateLambda {
        //         registers,
        //         memory,
        //         table,
        //         index,
        //     } => {
        //         let Value::Blob(registers) = registers.into() else {
        //             unreachable!();
        //         };
        //         let Value::PageTable(memory) = memory.into() else {
        //             unreachable!();
        //         };
        //         let Value::Tree(table) = table.into() else {
        //             unreachable!();
        //         };
        //         todo!();
        //     }
        //     Request::CreateThunk {
        //         registers,
        //         memory,
        //         table,
        //     } => {
        //         let Value::Blob(registers) = registers.into() else {
        //             unreachable!();
        //         };
        //         let Value::PageTable(memory) = memory.into() else {
        //             unreachable!();
        //         };
        //         let Value::Tree(table) = table.into() else {
        //             unreachable!();
        //         };
        //         todo!();
        //     }
        //     Request::ReadError(handle) => {
        //         let Value::Error(error) = handle.into() else {
        //             unreachable!();
        //         };
        //         Response::Handle((*error).clone().into())
        //     }
        //     Request::ReadBlob(handle) => {
        //         let Value::Blob(blob) = handle.into() else {
        //             unreachable!();
        //         };
        //         let span = Arc::into_raw(blob);
        //         let (ptr, len) = span.to_raw_parts();
        //         let ptr = allocator.to_offset(ptr);
        //         Response::Span { ptr, len }
        //     }
        //     Request::ReadTree(handle) => {
        //         let Value::Tree(tree) = handle.into() else {
        //             unreachable!();
        //         };
        //         let v: Vec<Handle> = tree.iter().cloned().map(Handle::from).collect();
        //         let v: Box<[Handle]> = v.into_boxed_slice();
        //         let ptr = Box::into_raw(v);
        //         let (ptr, len) = ptr.to_raw_parts();
        //         let ptr = allocator.to_offset(ptr);
        //         Response::Span { ptr, len }
        //     }
        //     Request::ReadPage(handle) => todo!(),
        //     Request::ReadPageTable(handle) => todo!(),
        //     Request::ReadLambda(handle) => todo!(),
        //     Request::ReadThunk(handle) => todo!(),
        //     Request::WriteBlob(handle) => todo!(),
        //     Request::WritePage(handle) => todo!(),
        //     Request::LoadElf(handle) => {
        //         let Value::Blob(blob) = handle.into() else {
        //             unreachable!();
        //         };
        //         let thunk = Thunk::from_elf(&blob);
        //         Response::Handle(Value::Thunk(thunk).into())
        //     }
        //     Request::Run(handle) => {
        //         let Value::Thunk(thunk) = handle.into() else {
        //             unreachable!();
        //         };
        //         let result = thunk.run();
        //         Response::Handle(result.into())
        //     }
        //     Request::Apply(lambda, argument) => {
        //         let Value::Lambda(lambda) = lambda.into() else {
        //             unreachable!();
        //         };
        //         let argument: Value = argument.into();
        //         let thunk = lambda.apply(argument);
        //         Response::Handle(Value::Thunk(thunk).into())
        //     }
        //     Request::Clone(handle) => {
        //         let value: Value = handle.into();
        //         let clone = value.clone();
        //         core::mem::forget(value);
        //         Response::Handle(clone.into())
        //     }
        //     Request::Drop(handle) => {
        //         let _: Value = handle.into();
        //         Response::Ack
        //     }
        // })
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
