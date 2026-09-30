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
            let rlimit: Tuple = data.get(3).try_into().ok()?;

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
            let arca = Arca::new_with(register_file, memory, descriptors, rlimit);
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
                    (Value::Tuple(rr), Value::Table(t), Value::Tuple(d))
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
                            log::error!(
                                "exited with exception: {x:x?} @ rip={:#x}",
                                arca.registers()[Register::RIP]
                            );
                            return Value::Function(arca::Function::from_inner(
                                Function::symbolic_with_args(
                                    "",
                                    vec![Value::Blob(Blob::from("exception")), Value::from(x)]
                                        .into(),
                                ),
                            ));
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

    pub fn arca_mut(&mut self) -> Option<&mut Arca> {
        match &mut self.defn {
            Definition::Symbolic(_) => None,
            Definition::Arcane(arca) => Some(arca),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies symbolic function parsing and read round-trip.
    #[test]
    fn test_symbolic_parse_and_read() {
        let args = Tuple::from((1u64, "two"));
        let value = Value::Tuple(Tuple::from((
            Blob::from("Symbolic"),
            Value::Word(Word::new(5)),
            Value::Tuple(args),
        )));
        let func = Function::new(value.clone()).expect("symbolic parse failed");
        assert!(!func.is_arcane());
        assert_eq!(func.read(), value);
    }

    /// Ensures unrecognized function tags are rejected.
    #[test]
    fn test_invalid_tag_rejected() {
        let value = Value::Tuple(Tuple::from((Blob::from("Other"), Value::Null(Null::new()))));
        assert!(Function::new(value).is_none());
    }

    /// Verifies arcane function parsing accepts a valid register/memory layout.
    #[test]
    fn test_arcane_parse_valid_layout() {
        let mut registers = Tuple::new(18);
        for i in 0..18 {
            registers.set(i, Value::Null(Null::new()));
        }
        let mut data = Tuple::new(4);
        data.set(0, Value::Tuple(registers));
        data.set(1, Value::Table(Table::new(1)));
        data.set(2, Value::Tuple(Tuple::new(0)));
        data.set(3, Value::Tuple(Tuple::new(0)));

        let value = Value::Tuple(Tuple::from((
            Blob::from("Arcane"),
            Value::Tuple(data),
            Value::Tuple(Tuple::new(0)),
        )));
        let func = Function::new(value).expect("arcane parse failed");
        assert!(func.is_arcane());
    }
}
