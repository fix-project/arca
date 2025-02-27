#![no_std]

pub mod syscall {
    pub const NOP: u64 = 0x00;
    pub const MOV: u64 = 0x01;
    pub const DUP: u64 = 0x02;
    pub const NULL: u64 = 0x03;
    pub const RESIZE: u64 = 0x04;

    pub const EXIT: u64 = 0x10;
    pub const EQ: u64 = 0x11;
    pub const FIND: u64 = 0x12;
    pub const LEN: u64 = 0x13;
    pub const READ: u64 = 0x14;
    pub const TYPE: u64 = 0x15;

    pub const CREATE_ATOM: u64 = 0x20;
    pub const CREATE_BLOB: u64 = 0x30;
    pub const CREATE_TREE: u64 = 0x40;

    pub const CONTINUATION: u64 = 0x50;
    pub const APPLY: u64 = 0x51;
    pub const TRACE: u64 = 0x52;
    pub const UNAPPLY: u64 = 0x53;
    pub const EXPLODE: u64 = 0x54;
    pub const IMPLODE: u64 = 0x55;
    pub const FORCE: u64 = 0x56;
    pub const PROMPT: u64 = 0x57;
    pub const PERFORM: u64 = 0x58;
    pub const TAILCALL: u64 = 0x59;

    pub const SHOW: u64 = 0xf0;
    pub const LOG: u64 = 0xf1;
}

pub mod error {
    pub const BAD_SYSCALL: u32 = 1;
    pub const BAD_INDEX: u32 = 2;
    pub const BAD_TYPE: u32 = 3;
    pub const BAD_ARGUMENT: u32 = 4;
    pub const OUT_OF_MEMORY: u32 = 5;
    pub const INTERRUPTED: u32 = 6;
    pub const CONTINUED: u32 = 7;
}

pub mod types {
    pub const NULL: u64 = 0x00;
    pub const ERROR: u64 = 0x01;
    pub const ATOM: u64 = 0x02;
    pub const BLOB: u64 = 0x03;
    pub const TREE: u64 = 0x04;
    pub const PAGE: u64 = 0x05;
    pub const PAGETABLE: u64 = 0x06;
    pub const LAMBDA: u64 = 0x07;
    pub const THUNK: u64 = 0x08;
}
