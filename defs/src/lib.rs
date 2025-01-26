#![no_std]

pub mod syscall {
    pub const SYS_NOOP: u64 = 0;
    pub const SYS_EXIT: u64 = 1;
}
