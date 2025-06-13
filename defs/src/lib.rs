#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub use arca_datatype as datatype;
pub use arca_entry as entry;
pub use arca_entry_mode as entry_mode;
pub use arca_error as error;
pub use arca_syscall as syscall;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SyscallError {
    BadSyscall,
    BadIndex,
    BadType,
    BadArgument,
    OutOfMemory,
    Interrupted,
}

impl SyscallError {
    pub fn code(&self) -> u32 {
        match self {
            SyscallError::BadSyscall => error::ERROR_BAD_SYSCALL,
            SyscallError::BadIndex => error::ERROR_BAD_INDEX,
            SyscallError::BadType => error::ERROR_BAD_TYPE,
            SyscallError::BadArgument => error::ERROR_BAD_ARGUMENT,
            SyscallError::OutOfMemory => error::ERROR_OUT_OF_MEMORY,
            SyscallError::Interrupted => error::ERROR_INTERRUPTED,
        }
    }

    pub fn new(code: u32) -> Self {
        match code {
            error::ERROR_BAD_SYSCALL => SyscallError::BadSyscall,
            error::ERROR_BAD_INDEX => SyscallError::BadIndex,
            error::ERROR_BAD_TYPE => SyscallError::BadType,
            error::ERROR_BAD_ARGUMENT => SyscallError::BadArgument,
            error::ERROR_OUT_OF_MEMORY => SyscallError::OutOfMemory,
            error::ERROR_INTERRUPTED => SyscallError::Interrupted,
            _ => unreachable!(),
        }
    }
}
