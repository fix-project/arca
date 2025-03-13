use core::ops::{ControlFlow, Deref};

use crate::initcell::OnceLock;
use crate::prelude::*;
use crate::spinlock::SpinLock;
use crate::types::Value;
use common::message::{
    BlobHandle, Handle, LambdaHandle, Message, Messenger, ThunkHandle, TreeHandle, WordHandle,
};

extern crate alloc;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

pub static MESSENGER: OnceLock<SpinLock<Messenger>> = OnceLock::new();

#[derive(Clone, Debug)]
enum MaybeBoxed<T> {
    Unboxed(T),
    Boxed(Box<T>),
}

impl<T> From<T> for MaybeBoxed<T> {
    fn from(value: T) -> Self {
        MaybeBoxed::Unboxed(value)
    }
}

impl<T> From<Box<T>> for MaybeBoxed<T> {
    fn from(value: Box<T>) -> Self {
        MaybeBoxed::Boxed(value)
    }
}

impl<T> MaybeBoxed<T> {
    fn unboxed(self) -> T {
        match self {
            MaybeBoxed::Unboxed(x) => x,
            MaybeBoxed::Boxed(x) => *x,
        }
    }

    fn boxed(self) -> Box<T> {
        match self {
            MaybeBoxed::Unboxed(x) => x.into(),
            MaybeBoxed::Boxed(x) => x,
        }
    }
}

impl<T> Deref for MaybeBoxed<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            MaybeBoxed::Unboxed(x) => x,
            MaybeBoxed::Boxed(x) => x,
        }
    }
}

impl<T> AsRef<T> for MaybeBoxed<T> {
    fn as_ref(&self) -> &T {
        match self {
            MaybeBoxed::Unboxed(x) => x,
            MaybeBoxed::Boxed(x) => x,
        }
    }
}

fn reply(m: &mut Messenger, value: MaybeBoxed<Value>) {
    let msg = match value.as_ref() {
        Value::Null => Message::Reply(Handle::Null),
        Value::Word(word) => Message::Reply(Handle::Word(WordHandle(*word))),
        Value::Blob(blob) => {
            let ptr = Arc::into_raw(blob.clone());
            Message::Reply(Handle::Blob(BlobHandle {
                ptr: PHYSICAL_ALLOCATOR.to_offset(ptr),
                len: ptr.len(),
            }))
        }
        Value::Tree(tree) => {
            let ptr = Arc::into_raw(tree.clone());
            Message::Reply(Handle::Tree(TreeHandle {
                ptr: PHYSICAL_ALLOCATOR.to_offset(ptr),
                len: ptr.len(),
            }))
        }
        Value::Lambda(_) => {
            let ptr = Box::into_raw(value.boxed());
            Message::Reply(Handle::Lambda(LambdaHandle(
                PHYSICAL_ALLOCATOR.to_offset(ptr),
            )))
        }
        Value::Thunk(_) => {
            let ptr = Box::into_raw(value.boxed());
            Message::Reply(Handle::Thunk(ThunkHandle(
                PHYSICAL_ALLOCATOR.to_offset(ptr),
            )))
        }
        _ => todo!("replying with {value:?}"),
    };
    log::debug!("replying {msg:?}");

    m.send(msg).unwrap();
}

fn reconstruct(handle: Handle) -> MaybeBoxed<Value> {
    let allocator = &*PHYSICAL_ALLOCATOR;
    unsafe {
        match handle {
            Handle::Null => Value::Null.into(),
            Handle::Word(x) => Value::Word(x.0).into(),
            Handle::Blob(handle) => Value::Blob(Arc::from_raw(core::ptr::from_raw_parts(
                allocator.from_offset::<u8>(handle.ptr),
                handle.len,
            )))
            .into(),
            Handle::Tree(handle) => Value::Tree(Arc::from_raw(core::ptr::from_raw_parts(
                allocator.from_offset::<Value>(handle.ptr),
                handle.len,
            )))
            .into(),
            Handle::Lambda(lambda) => {
                Box::<Value>::from_raw(allocator.from_offset(lambda.0)).into()
            }
            Handle::Thunk(thunk) => Box::<Value>::from_raw(allocator.from_offset(thunk.0)).into(),
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
            reply(m, Value::Blob(blob).into());
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
                vec.push(reconstruct(v).unboxed());
            }
            reply(m, Value::Tree(vec.into()).into());
        }
        Message::CreateThunk(handle) => {
            let v = reconstruct(handle.into()).unboxed();
            match v {
                Value::Blob(b) => reply(m, Value::Thunk(Thunk::from_elf(&b)).into()),
                _ => todo!(),
            };
        }
        Message::Run(handle) => {
            let v = reconstruct(handle.into()).unboxed();
            match v {
                Value::Thunk(thunk) => reply(m, thunk.run(cpu).into()),
                _ => todo!(),
            };
        }
        Message::Apply(lambda, arg) => {
            let v = reconstruct(lambda.into()).unboxed();
            let arg = reconstruct(arg).unboxed();
            match v {
                Value::Lambda(lambda) => reply(m, Value::Thunk(lambda.apply(arg)).into()),
                _ => todo!(),
            };
        }
        Message::ApplyAndRun(lambda, arg) => {
            let v = reconstruct(lambda.into()).unboxed();
            let arg = reconstruct(arg).unboxed();
            match v {
                Value::Lambda(lambda) => reply(m, lambda.apply(arg).run(cpu).into()),
                _ => todo!(),
            };
        }
        Message::Clone(handle) => {
            let p = reconstruct(handle);
            let q = p.clone();
            reply(m, q);
            core::mem::forget(p);
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
