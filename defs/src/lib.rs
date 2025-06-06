#![no_std]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SyscallError {
    BadSyscall,
    BadIndex,
    BadType,
    BadArgument,
    OutOfMemory,
    Continued,
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
            SyscallError::Continued => error::ERROR_CONTINUED,
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
            error::ERROR_CONTINUED => SyscallError::Continued,
            error::ERROR_INTERRUPTED => SyscallError::Interrupted,
            _ => unreachable!(),
        }
    }
}
