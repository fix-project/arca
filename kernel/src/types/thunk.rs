use alloc::{boxed::Box, vec};

use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thunk {
    f: Lambda,
    x: Box<Value>,
}

impl Thunk {
    pub fn new<T: Into<Box<Value>>>(f: Lambda, x: T) -> Thunk {
        Thunk { f, x: x.into() }
    }

    pub fn run(self) -> Value {
        let mut cpu = CPU.borrow_mut();
        let Thunk { f, mut x } = self;
        let Lambda { arca } = f;
        log::debug!("loading Arca");
        let mut arca = arca.load(&mut cpu);
        loop {
            log::debug!("jumping into Arca");
            let result = arca.run();
            if result.code != 256 {
                log::debug!("exited with exception: {result:?}");
                // let arca = arca.unload();
                let tree = vec![
                    Value::Atom("exception".into()),
                    // Value::Thunk(Thunk::new(Lambda::new(arca), x)),
                    Value::Blob(result.code.to_ne_bytes().into()),
                    Value::Blob(result.error.to_ne_bytes().into()),
                ];
                return Value::Error(Value::Tree(tree.into()).into());
            }
            let regs = arca.registers();
            let num = regs[Register::RDI];
            let args = &[
                regs[Register::RSI],
                regs[Register::RDX],
                regs[Register::R10],
                regs[Register::R8],
                regs[Register::R9],
            ];
            log::debug!("exited with syscall: {num:#x?}({args:?})");
            let result = &mut [0, 0];
            match num {
                defs::syscall::RESIZE => {
                    let len = args[0] as usize;
                    arca.descriptors_mut().resize(len, Value::None);
                    result[0] = 0;
                }
                defs::syscall::EXIT => {
                    let idx = args[0] as usize;
                    if let Some(x) = arca.descriptors_mut().get_mut(idx) {
                        let mut val = Value::None;
                        core::mem::swap(x, &mut val);
                        return val;
                    };
                }
                defs::syscall::ARGUMENT => {
                    let idx = args[0] as usize;
                    if let Some(spot) = arca.descriptors_mut().get_mut(idx) {
                        core::mem::swap(spot, &mut x);
                        result[0] = 0;
                    } else {
                        result[0] = defs::error::BAD_INDEX;
                    }
                }
                defs::syscall::READ => 'read: {
                    // TODO: sanitize user inputs
                    let idx = args[0] as usize;
                    let Some(val) = arca.descriptors_mut().get(idx) else {
                        result[0] = defs::error::BAD_INDEX;
                        break 'read;
                    };
                    match val {
                        Value::Blob(blob) => {
                            let ptr = args[1] as *mut u8;
                            let len = args[2] as usize;
                            unsafe {
                                let slice = core::slice::from_raw_parts_mut(ptr, len);
                                slice.copy_from_slice(blob);
                            }
                            result[0] = 0;
                        }
                        _ => {
                            result[0] = defs::error::BAD_TYPE;
                        }
                    }
                }
                defs::syscall::CREATE_BLOB => 'create: {
                    // TODO: sanitize user inputs
                    let idx = args[0] as usize;
                    if idx >= arca.descriptors().len() {
                        result[0] = defs::error::BAD_INDEX;
                        break 'create;
                    }
                    let ptr = args[1] as *mut u8;
                    let len = args[2] as usize;
                    unsafe {
                        let slice = core::slice::from_raw_parts_mut(ptr, len);
                        arca.descriptors_mut()[idx] = Value::Blob(slice.into());
                        result[0] = 0;
                    }
                }
                _ => {
                    log::error!("invalid syscall {num:#x}");
                    // let arca = arca.unload();
                    let tree = vec![
                        Value::Atom("invalid syscall".into()),
                        Value::Blob(num.to_ne_bytes().into()),
                        // Value::Thunk(Thunk::new(Lambda::new(arca), x)),
                    ];
                    return Value::Error(Value::Tree(tree.into()).into());
                }
            }
            let regs = arca.registers_mut();
            regs[Register::RAX] = result[0] as u64;
            regs[Register::RDX] = result[1] as u64;
        }
    }
}
