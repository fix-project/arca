use crate::{ArcaError, Runtime, prelude::*, syscall_result, syscall_result_raw};
use arcane::*;

pub fn argument() -> Value {
    unsafe {
        loop {
            let result = syscall_result(arca_argument());
            if let Err(ArcaError::Interrupted) = result {
                continue;
            }
            return Runtime::raw_convert(result.unwrap());
        }
    }
}

pub fn exit(value: impl Into<Value>) -> ! {
    unsafe {
        let val = super::Runtime::get_raw(value.into()).into_raw();
        loop {
            arca_exit(val.into())
        }
    }
}

pub fn call_with_current_continuation(f: Function) -> Value {
    unsafe {
        syscall_result_raw(arca_call_with_current_continuation(
            f.into_inner().into_raw() as i64,
        ))
        .unwrap();
        os::argument()
    }
}

pub fn continuation() -> Result<Function, ()> {
    unsafe {
        let result = arca_get_continuation();
        if result <= 0 {
            return Err(());
        } else {
            return Ok(arca::Function::from_inner(syscall_result(result).unwrap()));
        }
    }
}
