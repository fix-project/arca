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
  cargo test -p fix --bin fix --target={{target}}

build bin:
  cargo build -p kernel --example={{bin}} --target={{target}} {{release}}

run bin *args:
  cargo run -p kernel --example={{bin}} --target={{target}} {{release}} -- {{args}}

fix *args:
  cargo run -p fix --target={{target}} {{release}} -- {{args}}

lint *args:
  cargo clippy --all-targets -- {{args}}
  cargo clippy --all-targets -p kernel -- {{args}}
  cargo clippy -p fix -- {{args}}
  cargo clippy -p user -- {{args}}

ctags:
  ctags -R arca arcane common fix kernel macros user vmm
