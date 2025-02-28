use crate::initcell::InitCell;
use crate::prelude::*;
use crate::spinlock::SpinLock;
use crate::types::Value;
use common::message::{ArcaHandle, Message, Messenger};

extern crate alloc;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

pub static MESSENGER: InitCell<SpinLock<Messenger>> = InitCell::empty();

fn reply(value: Box<Value>) -> () {
    let ptr = Box::into_raw(value);
    let msg = Message::ReplyMessage {
        handle: ArcaHandle::new(PHYSICAL_ALLOCATOR.to_offset(ptr)),
    };
    let _ = MESSENGER.lock().push_outgoing_message(msg);
}

fn reconstruct(handle: ArcaHandle) -> Box<Value> {
    let ptr = PHYSICAL_ALLOCATOR.from_offset::<Value>(handle.to_offset());
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
        let _ = MESSENGER.lock().read_exact(1);
        let msg = MESSENGER.lock().pop_incoming_message().unwrap();
        let cont = process_incoming_message(msg, cpu);
        if !cont {
            return;
        }
        let _ = MESSENGER.lock().write_all();
    }
}
