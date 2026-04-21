alias b := build
alias r := run

target := 'x86_64-unknown-none'

release := if env('RELEASE', '0') != '0'{ '--release' } else { '' }

default:
  just --list

build-all:
  cargo build {{release}}
  cargo build -p kernel {{release}}
  cargo build -p fix {{release}}

test:
  cargo test
  cargo test -p kernel --target={{target}}

build bin:
  cargo build -p kernel --bin={{bin}} --target={{target}} {{release}}

run bin:
  cargo run -p kernel --bin={{bin}} --target={{target}} {{release}}

fix:
  cargo run -p fix --target={{target}} {{release}}

fmt:
  cargo fmt
  cargo fmt -p kernel
  cargo fmt -p fix

lint:
  cargo clippy
  cargo clippy -p kernel
  cargo clippy -p fix
