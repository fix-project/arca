# Arca

## Installation

### Runtime

Arca is currently paravirtualized within a custom hypervisor; it requires a
Linux AMD64 machine with KVM enabled.

### Toolchain

Arca is written in nightly Rust.  You should install Rust and Cargo via
`rustup`. Arca requires the `x86_64-unknown-none` target.  Arca expects Rust
version 1.85+.

Instructions for Debian- and RHEL-based distributions:
```sh
sudo [apt|dnf] install rustup
# rustup toolchain install stable # (optional)
rustup toolchain install nightly
rustup target install x86_64-unknown-none
```

You can update Rust and Cargo using:
```sh
rustup update
```

## Fix Compilation

Building Fix-on-Arca additionally requires installing [the GCC multilib package
(`gcc-multilib`)][gcc-multilib] on Debian-based distributions.

[gcc-multilib]: https://packages.debian.org/bookworm/gcc-multilib

## Running

Arca can be run using the standard Cargo build commands.

```sh
cargo build
cargo run
cargo test
```
