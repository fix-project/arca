#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[derive(Copy, Clone, Eq, PartialEq, Debug, derive_more::TryFrom)]
#[repr(u32)]
pub enum SyscallError {
    BadSyscall = __ERR_bad_syscall,
    BadIndex = __ERR_bad_index,
    BadType = __ERR_bad_type,
    BadArgument = __ERR_bad_argument,
    Interrupted = __ERR_interrupted,
}
