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
  cargo build -p user {{release}}

test:
  cargo test
  cargo test -p kernel --target={{target}}

build bin:
  cargo build -p kernel --example={{bin}} --target={{target}} {{release}}

run bin *args:
  cargo run -p kernel --example={{bin}} --target={{target}} {{release}} -- {{args}}

fix *args:
  cargo run -p fix --target={{target}} {{release}} -- {{args}}

fmt:
  cargo fmt
  cargo fmt -p kernel
  cargo fmt -p fix
  cargo fmt -p user

lint:
  cargo clippy
  cargo clippy -p kernel
  cargo clippy -p fix
  cargo clippy -p user

ctags:
  ctags -R arca arcane common fix kernel macros user vmm
