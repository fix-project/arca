# Standalone programs

`//programs` contains useful programs runnable with `bazel run`.
Programs used only for tests belong in `//tests`. Integration tests for a
useful program can stay beside it in `//programs`.

Flatware utilities are in `//programs/flatware`. Fix programs in WAT, C, and Rust are in `//programs/fix/{wat,c,rust}`. Each language provides the same
blob, addition, and directory-path programs, with path integration tests
alongside them. Other Fix programs, including `addblob`, `parser`, `fib`, `map`,
and `mapreduce`, live directly under `//programs/fix`.

Arca provides arithmetic, identity, null, I/O, map, and curry operations, plus
HTTP servers. Fix provides identity and addition operations and the text-format
parser. `//programs/kernel:hello` prints a greeting. SIMD, threading, and
artificial slow-add fixtures live under `//tests`.
