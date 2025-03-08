use crate::initcell::OnceLock;
use crate::prelude::*;
use crate::spinlock::SpinLock;
use crate::types::Value;
use common::message::{
    BlobHandle, Handle, LambdaHandle, Message, Messenger, ThunkHandle, TreeHandle,
};

extern crate alloc;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

pub static MESSENGER: OnceLock<SpinLock<Messenger>> = OnceLock::new();

fn reply(value: Box<Value>) {
    let msg = match value.as_ref() {
        Value::Blob(_) => {
            let ptr = Box::into_raw(value);
            Message::ReplyMessage {
                handle: Handle::Blob(BlobHandle::new(PHYSICAL_ALLOCATOR.to_offset(ptr))),
            }
        }
        Value::Tree(_) => {
            let ptr = Box::into_raw(value);
            Message::ReplyMessage {
                handle: Handle::Tree(TreeHandle::new(PHYSICAL_ALLOCATOR.to_offset(ptr))),
            }
        }
        Value::Lambda(_) => {
            let ptr = Box::into_raw(value);
            Message::ReplyMessage {
                handle: Handle::Lambda(LambdaHandle::new(PHYSICAL_ALLOCATOR.to_offset(ptr))),
            }
        }
        Value::Thunk(_) => {
            let ptr = Box::into_raw(value);
            Message::ReplyMessage {
                handle: Handle::Thunk(ThunkHandle::new(PHYSICAL_ALLOCATOR.to_offset(ptr))),
            }
        }
        _ => todo!(),
    };

    MESSENGER.lock().send(msg).unwrap();
}

fn reconstruct<T: Into<Handle>>(handle: T) -> Box<Value> {
    let ptr = PHYSICAL_ALLOCATOR.from_offset::<Value>(Handle::to_offset(handle));
    unsafe { Box::from_raw(ptr) }
}

pub fn process_incoming_message(msg: Message, cpu: &mut Cpu) -> bool {
    log::debug!("message: {msg:x?}");
    match msg {
        Message::CreateBlobMessage { ptr, size } => {
            let blob: Blob = unsafe {
                Arc::from_raw(core::ptr::slice_from_raw_parts(
                    PHYSICAL_ALLOCATOR.from_offset::<u8>(ptr),
                    size,
                ))
            };
            reply(Box::new(Value::Blob(blob)));
            true
        }
        Message::CreateTreeMessage { ptr, size } => {
            let vals: Box<[Handle]> = unsafe {
                Box::from_raw(core::ptr::slice_from_raw_parts_mut(
                    PHYSICAL_ALLOCATOR.from_offset::<Handle>(ptr),
                    size,
                ))
            };
            let mut vec = Vec::new();
            for v in vals {
                vec.push(Box::into_inner(reconstruct(v)));
            }
            reply(Box::new(Value::Tree(vec.into())));
            true
        }
        Message::CreateThunkMessage { handle } => {
            let v = reconstruct(handle);
            match Box::into_inner(v) {
                Value::Blob(b) => reply(Box::new(Value::Thunk(Thunk::from_elf(&b)))),
                _ => todo!(),
            };
            true
        }
        Message::RunThunkMessage { handle } => {
            let v = reconstruct(handle);
            match Box::into_inner(v) {
                Value::Thunk(thunk) => reply(Box::new(thunk.run(cpu))),
                _ => todo!(),
            };
            true
        }
        Message::ApplyMessage {
            lambda_handle,
            arg_handle,
        } => {
            let v = Box::into_inner(reconstruct(lambda_handle));
            let arg = Box::into_inner(reconstruct(arg_handle));
            match v {
                Value::Lambda(lambda) => reply(Box::new(Value::Thunk(lambda.apply(arg)))),
                _ => todo!(),
            };
            true
        }
        Message::DropMessage { handle } => {
            let p = reconstruct(handle);
            core::mem::drop(p);
            false
        }
        Message::ReadBlobMessage { handle } => {
            let v = reconstruct(handle);
            match v.as_ref() {
                Value::Blob(blob) => {
                    let (ptr, size) = (&raw const blob[0], blob.len());
                    let ptr = PHYSICAL_ALLOCATOR.to_offset(ptr);
                    let msg = Message::BlobContentsMessage { ptr, size };
                    MESSENGER.lock().send(msg).unwrap();
                }
                _ => todo!(),
            };
            core::mem::forget(v);
            false
        }
        _ => todo!(),
    }
}

pub fn run(cpu: &mut Cpu) {
    loop {
        let msg = MESSENGER.lock().receive().expect("Failed to read msg");
        let cont = process_incoming_message(msg, cpu);
        if !cont {
            return;
        }
    }
}
