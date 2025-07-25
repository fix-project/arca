pub mod syscall;

use core::ops::ControlFlow;

use alloc::collections::vec_deque::VecDeque;

use super::arca::Arca;
use crate::{cpu::ExitReason, prelude::*, types::function::syscall::handle_syscall};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    defn: Definition,
    args: VecDeque<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Definition {
    Symbolic(Box<Value>),
    Arcane(Arca),
}

impl Function {
    pub fn arcane(arca: Arca) -> Self {
        Self {
            defn: Definition::Arcane(arca),
            args: VecDeque::new(),
        }
    }

    pub fn arcane_with_args(arca: Arca, args: VecDeque<Value>) -> Self {
        Self {
            defn: Definition::Arcane(arca),
            args,
        }
    }

    pub fn symbolic(symbol: impl Into<Value>) -> Self {
        Self {
            defn: Definition::Symbolic(Box::new(symbol.into())),
            args: VecDeque::new(),
        }
    }

    pub fn apply(&mut self, arg: impl Into<Value>) {
        self.args.push_back(arg.into());
    }

    pub fn force(self) -> Value {
        let mut cpu = CPU.borrow_mut();
        self.force_on(&mut cpu)
    }

    pub fn force_on(mut self, cpu: &mut Cpu) -> Value {
        match self.defn {
            Definition::Symbolic(_) => Value::Function(arca::Function::from_inner(self)),
            Definition::Arcane(arca) => {
                let mut arca = arca.load(cpu);

                loop {
                    let result = arca.run();
                    match result {
                        ExitReason::SystemCall => {}
                        ExitReason::Interrupted(x) => {
                            if x == 0x20 {
                                continue;
                            }
                            panic!("exited with interrupt: {x:?}");
                        }
                        x => {
                            panic!(
                                "exited with exception: {x:x?} @ rip={:#x}",
                                arca.registers()[Register::RIP]
                            );
                        }
                    }
                    if let ControlFlow::Break(result) = handle_syscall(&mut arca, &mut self.args) {
                        return result;
                    }
                }
            }
        }
    }

    pub fn is_arcane(&self) -> bool {
        matches!(self.defn, Definition::Arcane(_))
    }

    pub fn read(self) -> (Value, Tuple) {
        let args = Tuple::from(&mut *Vec::from_iter(self.args).into_boxed_slice());
        match self.defn {
            Definition::Symbolic(value) => (*value, args),
            Definition::Arcane(_) => todo!(),
        }
    }
}
