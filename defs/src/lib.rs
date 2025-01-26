#![no_std]

pub mod syscall {
    pub const NOOP: u64 = 0x11;
    pub const EXIT: u64 = 0x12;
    pub const FORCE: u64 = 0x13;
    pub const ARGUMENT: u64 = 0x14;
    pub const EQ: u64 = 0x15;
    pub const FIND: u64 = 0x16;
    pub const LEN: u64 = 0x17;
    pub const ATOM_CREATE: u64 = 0x21;
    pub const BLOB_CREATE: u64 = 0x31;
    pub const BLOB_READ: u64 = 0x32;
}
