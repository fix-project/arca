#![no_std]
extern crate alloc;
mod core_loop;
mod state;
pub use core_loop::{Centralized, Decision, Endpoint, Input, Message};
pub use state::*;

#[cfg(test)]
mod tests;
