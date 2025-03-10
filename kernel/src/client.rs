use core::ops::ControlFlow;

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

fn reply(m: &mut Messenger, value: Value) {
    let msg = match value {
        Value::Null => Message::Created(Handle::Null),
        Value::Blob(blob) => {
            let ptr = Arc::into_raw(blob);
            Message::Created(Handle::Blob(BlobHandle {
                ptr: PHYSICAL_ALLOCATOR.to_offset(ptr),
                len: ptr.len(),
            }))
        }
        Value::Tree(tree) => {
            let ptr = Arc::into_raw(tree);
            Message::Created(Handle::Tree(TreeHandle {
                ptr: PHYSICAL_ALLOCATOR.to_offset(ptr),
                len: ptr.len(),
            }))
        }
        Value::Lambda(lambda) => {
            let ptr = Box::into_raw(lambda.into());
            Message::Created(Handle::Lambda(LambdaHandle(
                PHYSICAL_ALLOCATOR.to_offset(ptr),
            )))
        }
        Value::Thunk(thunk) => {
            let ptr = Box::into_raw(thunk.into());
            Message::Created(Handle::Thunk(ThunkHandle(
                PHYSICAL_ALLOCATOR.to_offset(ptr),
            )))
        }
        _ => todo!(),
    };

    m.send(msg).unwrap();
}

fn reconstruct(handle: Handle) -> Value {
    let allocator = &*PHYSICAL_ALLOCATOR;
    unsafe {
        match handle {
            Handle::Null => Value::Null,
            Handle::Blob(handle) => Value::Blob(Arc::from_raw(core::ptr::from_raw_parts(
                allocator.from_offset::<u8>(handle.ptr),
                handle.len,
            ))),
            Handle::Tree(handle) => Value::Tree(Arc::from_raw(core::ptr::from_raw_parts(
                allocator.from_offset::<Value>(handle.ptr),
                handle.len,
            ))),
            Handle::Lambda(lambda) => {
                Value::Lambda(*Box::from_raw(allocator.from_offset(lambda.0)))
            }
            Handle::Thunk(thunk) => Value::Thunk(*Box::from_raw(allocator.from_offset(thunk.0))),
        }
    }
}

pub fn process_incoming_message(m: &mut Messenger, msg: Message, cpu: &mut Cpu) -> ControlFlow<()> {
    log::debug!("message: {msg:x?}");
    match msg {
        Message::CreateBlob { ptr, len } => {
            let blob: Blob = unsafe {
                Arc::from_raw(core::ptr::slice_from_raw_parts(
                    PHYSICAL_ALLOCATOR.from_offset::<u8>(ptr),
                    len,
                ))
            };
            reply(m, Value::Blob(blob));
        }
        Message::CreateTree { ptr, len } => {
            let vals: Box<[Handle]> = unsafe {
                Box::from_raw(core::ptr::slice_from_raw_parts_mut(
                    PHYSICAL_ALLOCATOR.from_offset::<Handle>(ptr),
                    len,
                ))
            };
            let mut vec = Vec::new();
            for v in vals {
                vec.push(reconstruct(v));
            }
            reply(m, Value::Tree(vec.into()));
        }
        Message::CreateThunk(handle) => {
            let v = reconstruct(handle.into());
            match v {
                Value::Blob(b) => reply(m, Value::Thunk(Thunk::from_elf(&b))),
                _ => todo!(),
            };
        }
        Message::Run(handle) => {
            let v = reconstruct(handle.into());
            match v {
                Value::Thunk(thunk) => reply(m, thunk.run(cpu)),
                _ => todo!(),
            };
        }
        Message::Apply(lambda, arg) => {
            let v = reconstruct(lambda.into());
            let arg = reconstruct(arg);
            match v {
                Value::Lambda(lambda) => reply(m, Value::Thunk(lambda.apply(arg))),
                _ => todo!(),
            };
        }
        Message::ApplyAndRun(lambda, arg) => {
            let v = reconstruct(lambda.into());
            let arg = reconstruct(arg);
            match v {
                Value::Lambda(lambda) => reply(m, lambda.apply(arg).run(cpu)),
                _ => todo!(),
            };
        }
        Message::Drop(handle) => {
            let p = reconstruct(handle);
            core::mem::drop(p);
        }
        Message::Exit => {
            return ControlFlow::Break(());
        }
        x => todo!("handling {x:?}"),
    }
    ControlFlow::Continue(())
}

pub fn run(cpu: &mut Cpu) {
    loop {
        let mut m = MESSENGER.lock();
        if !m.is_empty() {
            let msg = m.receive().expect("Failed to read msg");
            if process_incoming_message(&mut m, msg, cpu) == ControlFlow::Break(()) {
                return;
            }
        }
        core::hint::spin_loop();
    }
}
