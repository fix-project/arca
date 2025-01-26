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
        let Thunk { f, x } = self;
        let Lambda { arca } = f;
        log::info!("loading Arca");
        let mut arca = arca.load(&mut cpu);
        loop {
            log::info!("jumping into Arca");
            let result = arca.run();
            log::info!("result: {result:?}");
            if result.code != 256 {
                let arca = arca.unload();
                let tree = vec![
                    Value::Atom("exception".into()),
                    Value::Thunk(Thunk::new(Lambda::new(arca), x)),
                    Value::Blob(result.code.to_be_bytes().into()),
                    Value::Blob(result.error.to_be_bytes().into()),
                ];
                return Value::Error(Value::Tree(tree.into()).into());
            }
            match arca.registers()[Register::RDI] {
                defs::syscall::SYS_NOOP => {
                    continue;
                }
                defs::syscall::SYS_EXIT => {
                    return Value::None;
                }
                _ => {
                    let arca = arca.unload();
                    let tree = vec![
                        Value::Atom("invalid syscall".into()),
                        Value::Thunk(Thunk::new(Lambda::new(arca), x)),
                    ];
                    return Value::Error(Value::Tree(tree.into()).into());
                }
            }
        }
    }
}
