use core::ops::ControlFlow;

use super::syscall::handle_syscall;
use crate::{cpu::ExitReason, prelude::*};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Application {
    pub arca: Box<Arca>,
}

impl arca::RuntimeType for Application {
    type Runtime = Runtime;

    fn runtime(&self) -> &Self::Runtime {
        &Runtime
    }
}

impl Application {
    pub fn new<T: Into<Box<Arca>>>(arca: T) -> Application {
        Application { arca: arca.into() }
    }

    pub fn run_on(self, cpu: &mut Cpu) -> arca::associated::Value<Self> {
        let Application { arca } = self;
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

    pub fn into_raw(self) -> *mut Arca {
        Box::into_raw(self.arca)
    }

    pub unsafe fn from_raw(raw: *mut Arca) -> Self {
        Application {
            arca: Box::from_raw(raw as *mut _),
        }
    }
}
