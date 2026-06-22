# Arca

## Installation

### Cloning

This repository contains submodules, you can clone it using:
```
git clone --recurse-submodules git@github.com:fix-project/arca
```

### Runtime

Arca is currently paravirtualized within a custom hypervisor; it requires a
Linux AMD64 machine with KVM enabled.  If you're using `stagecast.org`, make
sure your user is in the `kvm` group.

### Toolchain

Arca is written in nightly Rust.  You should install Rust and Cargo via
`rustup`. Arca requires the `x86_64-unknown-none` target.  Arca expects Rust
version 1.98+.

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

Be aware that nightly releases often break compatibility, so you may have to
patch code to run on newer versions.  In general we try to follow the latest
nightly release.

## Fix Compilation

Building Fix-on-Arca additionally requires installing [the GCC multilib package
(`gcc-multilib`)][gcc-multilib] on Debian-based distributions.

[gcc-multilib]: https://packages.debian.org/bookworm/gcc-multilib

## Running

We use the [just command runner](https://github.com/casey/just) to help
simplify the commands needed to build and run arca.

To run the test suite:
```sh
just test
```

To run an example kernel (from `kernel/examples`):
```sh
just run hello
just run threads
just run webserver
```

To run Fix-on-Arca, run:
```sh
just fix addblob.fix
```

# License

This codebase is licensed under the GNU Lesser General Public License v2.1 or
later (LGPL-2.1-or-later).  See [LICENSE](LICENSE)
for more information.
