# Arca

## Installation

### Runtime

Arca is currently paravirtualized within QEMU; it specifically requires a Linux
AMD64 machine with KVM and the `invtsc`/`constant_tsc` CPU feature.

You can check for this feature using:
```sh
cat /proc/cpuinfo | grep -q "constant_tsc"
```

And you can install QEMU on Debian- or RHEL-based Linux distributions using:
```sh
sudo [apt|dnf] install qemu-system-x86
```

### Toolchain

Arca is written in nightly Rust.  You should install Rust and Cargo via
`rustup`. Arca requires the `x86_64-unknown-none` target.

Instructions for Debian- and RHEL-based distributions:
```sh
sudo [apt|dnf] install rustup
# rustup toolchain install stable # (optional)
rustup toolchain install nightly
rustup target install x86_64-unknown-none
```

You also need the Netwide Assembler:
```sh
sudo [apt|dnf] install nasm
```

## Running

Since Arca runs within QEMU, the `xtask` subproject contains a wrapper to set
up QEMU correctly (and use the correct toolchains, and run the test harness if
desired).  You can use these aliases in lieu of the default `cargo`
build/run/test commands.

```sh
cargo xtask build
cargo xtask run
cargo xtask test
```
