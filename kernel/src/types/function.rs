pub mod syscall;

use core::ops::ControlFlow;

use alloc::collections::vec_deque::VecDeque;

use super::arca::Arca;
use crate::{
    cpu::ExitReason,
    prelude::*,
    types::{function::syscall::handle_syscall, internal},
};

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
    pub fn new(value: Value) -> Option<Self> {
        let Value::Tuple(mut x) = value else {
            return None;
        };
        let t = x.take(0);
        let arca = t == Value::Blob("Arcane".into());
        let symbolic = t == Value::Blob("Symbolic".into());
        let data = x.take(1);
        let args = if x.len() < 3 {
            Tuple::new(0)
        } else {
            let Value::Tuple(args) = x.take(2) else {
                return None;
            };
            args
        };
        let args = args.into_inner();
        let args = VecDeque::from(args.into_inner().into_vec());
        let value = if arca {
            let data: Tuple = data.try_into().ok()?;
            let registers: Tuple = data.get(0).try_into().ok()?;
            let memory: Table = data.get(1).try_into().ok()?;
            let descriptors: Tuple = data.get(2).try_into().ok()?;

            let registers = registers.into_inner();
            let mut register_file = RegisterFile::new();
            for (i, x) in registers
                .iter()
                .take(18)
                .enumerate()
                .filter(|(_, x)| x.datatype() != DataType::Null)
            {
                let Value::Word(w) = *x else {
                    return None;
                };
                register_file[i] = w.read();
            }
            let memory = memory.into_inner();
            let descriptors = descriptors.into_inner();
            let arca = Arca::new_with(register_file, memory, descriptors);
            Function::arcane_with_args(arca, args)
        } else if symbolic {
            Function::symbolic_with_args(data, args)
        } else {
            return None;
        };
        Some(value)
    }

    pub fn read(self) -> Value {
        let args = Vec::from_iter(self.args);
        let args = Tuple::from_inner(internal::Tuple::new(args));
        match self.defn {
            Definition::Symbolic(value) => {
                Value::Tuple(Tuple::from((Blob::from("Symbolic"), *value, args)))
            }
            Definition::Arcane(arca) => Value::Tuple(Tuple::from((
                Blob::from("Arcane"),
                Tuple::from({
                    let (r, t, d) = arca.read();
                    let mut rr = Tuple::new(18);
                    for i in 0..18 {
                        rr.set(i, Value::Word(Word::new(r[i])));
                    }
                    (
                        Value::Tuple(rr),
                        Value::Table(Table::from_inner(t)),
                        Value::Tuple(Tuple::from_inner(d)),
                    )
                }),
                args,
            ))),
        }
    }

    pub fn arcane_with_args(arca: Arca, args: VecDeque<Value>) -> Self {
        Self {
            defn: Definition::Arcane(arca),
            args,
        }
    }

    pub fn symbolic_with_args(symbol: impl Into<Value>, args: VecDeque<Value>) -> Self {
        Self {
            defn: Definition::Symbolic(Box::new(symbol.into())),
            args,
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
}
