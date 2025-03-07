use crate::initcell::OnceLock;
use crate::prelude::*;
use crate::spinlock::SpinLock;
use crate::types::Value;
use common::message::{
    ArcaHandle, BlobHandle, LambdaHandle, Message, Messenger, ThunkHandle, TreeHandle,
};

extern crate alloc;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

pub static MESSENGER: OnceLock<SpinLock<Messenger>> = OnceLock::new();

fn reply(value: Box<Value>) {
    let msg = match (&value).as_ref() {
        Value::Blob(_) => {
            let ptr = Box::into_raw(value);
            Message::ReplyMessage {
                handle: ArcaHandle::BlobHandle(BlobHandle::new(PHYSICAL_ALLOCATOR.to_offset(ptr))),
            }
        }
        Value::Tree(_) => {
            let ptr = Box::into_raw(value);
            Message::ReplyMessage {
                handle: ArcaHandle::TreeHandle(TreeHandle::new(PHYSICAL_ALLOCATOR.to_offset(ptr))),
            }
        }
        Value::Lambda(_) => {
            let ptr = Box::into_raw(value);
            Message::ReplyMessage {
                handle: ArcaHandle::LambdaHandle(LambdaHandle::new(
                    PHYSICAL_ALLOCATOR.to_offset(ptr),
                )),
            }
        }
        Value::Thunk(_) => {
            let ptr = Box::into_raw(value);
            Message::ReplyMessage {
                handle: ArcaHandle::ThunkHandle(ThunkHandle::new(
                    PHYSICAL_ALLOCATOR.to_offset(ptr),
                )),
            }
        }
        _ => todo!(),
    };

    let _ = MESSENGER.lock().send(msg);
}

fn reconstruct<T: Into<ArcaHandle>>(handle: T) -> Box<Value> {
    let ptr = PHYSICAL_ALLOCATOR.from_offset::<Value>(ArcaHandle::to_offset(handle));
    unsafe { Box::from_raw(ptr as *mut Value) }
}

pub fn process_incoming_message(msg: Message, cpu: &mut Cpu) -> bool {
    match msg {
        Message::CreateBlobMessage { ptr, size } => {
            let blob: Blob = unsafe {
                Arc::from_raw(core::ptr::slice_from_raw_parts(
                    PHYSICAL_ALLOCATOR.from_offset::<u8>(ptr),
                    size,
                ))
            };
            reply(Box::new(Value::Blob(blob)));
            log::info!("processed create blob");
            true
        }
        Message::CreateTreeMessage { ptr, size } => {
            let vals: Box<[ArcaHandle]> = unsafe {
                Box::from_raw(core::ptr::slice_from_raw_parts_mut(
                    PHYSICAL_ALLOCATOR.from_offset::<ArcaHandle>(ptr) as *mut ArcaHandle,
                    size,
                ))
            };
            let mut vec = Vec::new();
            for v in vals {
                vec.push(Box::into_inner(reconstruct(v)));
            }
            reply(Box::new(Value::Tree(vec.into())));
            log::info!("processed create tree");
            true
        }
        Message::CreateThunkMessage { handle } => {
            let v = reconstruct(handle);
            match Box::into_inner(v) {
                Value::Blob(b) => reply(Box::new(Value::Thunk(Thunk::from_elf(&*b)))),
                _ => todo!(),
            };
            log::info!("processed create thunk");
            true
        }
        Message::RunThunkMessage { handle } => {
            let v = reconstruct(handle);
            match Box::into_inner(v) {
                Value::Thunk(thunk) => reply(Box::new(thunk.run(cpu))),
                _ => todo!(),
            };
            log::info!("processed run thunk");
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
            log::info!("processed apply lambda");
            true
        }
        Message::DropMessage { handle } => {
            reconstruct(handle);
            log::info!("processed drop");
            false
        }
        _ => todo!(),
    }
}

pub fn run(cpu: &mut Cpu) -> () {
    loop {
        let msg = MESSENGER.lock().get_one().ok().expect("Failed to read msg");
        let cont = process_incoming_message(msg, cpu);
        if !cont {
            return;
        }
    }
}
