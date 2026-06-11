#![allow(stable_features, unused_features)]
#![feature(allocator_api)]
#![feature(ptr_metadata)]
#![feature(str_from_raw_parts)]
#![feature(exitcode_exit_method)]
#![feature(cstr_display)]

mod control_monitor;
mod doorbell;
mod monitor;
pub mod runtime;
mod vmm_pipe;
