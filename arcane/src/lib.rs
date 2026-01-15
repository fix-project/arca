#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[derive(Copy, Clone, Eq, PartialEq, Debug, derive_more::TryFrom)]
#[repr(u32)]
#[try_from(repr)]
pub enum SyscallError {
    BadSyscall = __ERR_bad_syscall,
    BadIndex = __ERR_bad_index,
    BadType = __ERR_bad_type,
    BadArgument = __ERR_bad_argument,
    OutOfMemory = __ERR_out_of_memory,
    Interrupted = __ERR_interrupted,
}
