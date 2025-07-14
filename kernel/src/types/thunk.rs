pub mod concrete;
pub mod syscall;

use common::message::Handle;
use concrete::Application as Concrete;

use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Thunk {
    Symbolic(Box<Value>, Box<Value>),
    Concrete(Concrete),
}

impl arca::RuntimeType for Thunk {
    type Runtime = Runtime;

    fn runtime(&self) -> &Self::Runtime {
        &Runtime
    }
}

impl arca::ValueType for Thunk {
    const DATATYPE: DataType = DataType::Thunk;
}

impl arca::Thunk for Thunk {
    fn run(self) -> arca::associated::Value<Self> {
        let mut cpu = crate::cpu::CPU.borrow_mut();
        self.run_on(&mut cpu)
    }

    fn read(
        self,
    ) -> (
        arca::associated::Blob<Self>,
        arca::associated::Table<Self>,
        arca::associated::Tree<Self>,
    ) {
        todo!()
    }
}

impl Thunk {
    pub fn new(f: Value, x: Value) -> Thunk {
        if let Value::Lambda(l) = f {
            let mut arca = l.arca;
            let idx = arca.descriptors_mut().insert(x);
            arca.registers_mut()[Register::RAX] = idx as u64;
            Thunk::Concrete(Concrete::new(arca))
        } else {
            Thunk::Symbolic(f.into(), x.into())
        }
    }

    pub fn from_elf(elf: &[u8]) -> Thunk {
        common::elfloader::load_elf(&Runtime, elf)
    }

    pub fn run_on(self, cpu: &mut Cpu) -> Value {
        match self {
            Thunk::Symbolic(f, x) => {
                if let Value::Lambda(l) = *f {
                    l.apply(*x).run_on(cpu)
                } else {
                    Tree::new([*f, *x]).into()
                }
            }
            Thunk::Concrete(application) => application.run_on(cpu),
        }
    }

    pub fn symbolic(&self) -> Option<(&Value, Vec<&Value>)> {
        match self {
            Thunk::Symbolic(car, cdr) => match &**car {
                Value::Thunk(thunk) => {
                    let (symbol, mut args) = thunk.symbolic()?;
                    args.push(cdr);
                    Some((symbol, args))
                }
                car => Some((car, vec![cdr])),
            },
            Thunk::Concrete(_) => None,
        }
    }
}

impl TryFrom<Handle> for Thunk {
    type Error = Handle;

    fn try_from(value: Handle) -> Result<Self, Self::Error> {
        if value.datatype() == <Self as arca::ValueType>::DATATYPE {
            let (x, y) = value.read();
            Ok(if y == 0 {
                // application
                unsafe { Thunk::Concrete(Concrete::from_raw(x as *mut _)) }
            } else {
                unsafe { Thunk::Symbolic(Box::from_raw(x as *mut _), Box::from_raw(y as *mut _)) }
            })
        } else {
            Err(value)
        }
    }
}

impl From<Thunk> for Handle {
    fn from(value: Thunk) -> Self {
        match value {
            Thunk::Symbolic(_, _) => todo!("handle to symbolic Thunk"),
            Thunk::Concrete(application) => {
                let ptr = application.into_raw();
                Handle::new(DataType::Thunk, (ptr as usize, 0))
            }
        }
    }
}

impl From<Concrete> for Thunk {
    fn from(value: Concrete) -> Self {
        Thunk::Concrete(value)
    }
}

impl From<Arca> for Thunk {
    fn from(value: Arca) -> Self {
        Concrete::new(value).into()
    }
}
