alias b := build
alias r := run
alias t := test

target := 'x86_64-unknown-none'

mode := 'debug'
extra_cargoflags := if mode == 'release' { '--release' } else { '' }

default:
  just --list

build:
  cargo build {{extra_cargoflags}}
  cargo build -p kernel {{extra_cargoflags}}
  cargo build -p fix {{extra_cargoflags}}

test:
  cargo test
  cargo test -p kernel --target={{target}}

run bin:
  cargo run -p kernel --bin={{bin}} --target={{target}} {{extra_cargoflags}}

fix:
  cargo run -p fix --target={{target}} {{extra_cargoflags}}
