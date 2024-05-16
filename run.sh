#!/usr/bin/env bash
cargo build --bin kernel
cargo run --bin runner -- --kernel target/x86_64-unknown-none/debug/kernel "$@"
