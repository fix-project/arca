pub mod syscall;

use core::ops::ControlFlow;

use common::message::Handle;
use syscall::handle_syscall;

use crate::{cpu::ExitReason, prelude::*};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thunk {
    pub arca: Box<Arca>,
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
    pub fn new<T: Into<Box<Arca>>>(arca: T) -> Thunk {
        Thunk { arca: arca.into() }
    }

    fn run_on(self, cpu: &mut Cpu) -> arca::associated::Value<Self> {
        let Thunk { arca } = self;
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
            if let ControlFlow::Break(result) = handle_syscall(&mut arca) {
                return result;
            }
        }
    }

    pub fn from_elf(elf: &[u8]) -> Thunk {
        common::elfloader::load_elf(&Runtime, elf)
    }
}

impl TryFrom<Handle> for Thunk {
    type Error = Handle;

    fn try_from(value: Handle) -> Result<Self, Self::Error> {
        if value.datatype() == <Self as arca::ValueType>::DATATYPE {
            let (raw, _) = value.read();
            unsafe {
                Ok(Thunk {
                    arca: Box::from_raw(raw as *mut _),
                })
            }
        } else {
            Err(value)
        }
    }
}

impl From<Thunk> for Handle {
    fn from(value: Thunk) -> Self {
        let ptr = Box::into_raw(value.arca);
        Handle::new(DataType::Thunk, (ptr as usize, 0))
    }
}
