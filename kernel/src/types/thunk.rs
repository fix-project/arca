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
                let arca = arca.unload();
                let tree = vec![
                    Value::Atom("exception".into()),
                    Value::Thunk(Thunk::new(Lambda::new(arca), x)),
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
                defs::syscall::NOOP => {
                    continue;
                }
                defs::syscall::EXIT => {
                    let idx = args[0] as usize;
                    let val = arca.descriptors_mut().get_mut(idx);
                    return match val {
                        Some(x) => {
                            let mut val = Value::None;
                            core::mem::swap(x, &mut val);
                            val
                        }
                        None => {
                            let tree = vec![
                                Value::Atom("invalid index".into()),
                                Value::Blob(idx.to_ne_bytes().into()),
                            ];
                            Value::Error(Value::Tree(tree.into()).into())
                        }
                    };
                }
                defs::syscall::ARGUMENT => {
                    let idx = arca.descriptors().len();
                    let mut y = Value::None;
                    core::mem::swap(&mut *x, &mut y);
                    arca.descriptors_mut().push(y);
                    result[0] = idx as u64;
                }
                defs::syscall::LEN => {
                    let idx = args[0] as usize;
                    let val = arca.descriptors_mut().get(idx);
                    let y = match val {
                        Some(Value::None) => 0,
                        Some(Value::Blob(x)) => x.len() as isize,
                        Some(Value::Tree(x)) => x.len() as isize,
                        _ => -1,
                    };
                    result[0] = y as u64;
                }
                defs::syscall::BLOB_READ => {
                    // TODO: sanitize user inputs
                    let idx = args[0] as usize;
                    let val = arca.descriptors_mut().get(idx);
                    let Some(Value::Blob(x)) = val else {
                        unimplemented!();
                    };
                    let ptr = args[1] as *mut u8;
                    let len = args[2] as usize;
                    let offset = args[3] as usize;
                    unsafe {
                        let slice = core::slice::from_raw_parts_mut(ptr, len);
                        slice.copy_from_slice(&x[offset..]);
                    }
                    result[0] = (x.len() - offset) as u64;
                }
                defs::syscall::BLOB_CREATE => {
                    // TODO: sanitize user inputs
                    let idx = arca.descriptors().len();
                    let ptr = args[0] as *mut u8;
                    let len = args[1] as usize;
                    unsafe {
                        let slice = core::slice::from_raw_parts_mut(ptr, len);
                        arca.descriptors_mut().push(Value::Blob(slice.into()));
                    }
                    result[0] = idx as u64;
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
            regs[Register::RAX] = result[0];
            regs[Register::RDX] = result[1];
        }
    }
}
