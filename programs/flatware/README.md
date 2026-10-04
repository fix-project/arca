# Minimal Flatware utilities

```sh
bazel run //programs/flatware:ls -- --dir ./input::/work /work
bazel run //programs/flatware:wc -- --dir ./input::/work /work/example.txt
bazel run //programs/flatware:cat -- --stdin ./example.txt
bazel run //programs/flatware:head -- --dir ./input::/work /work/example.txt
```

`ls` prints sorted directory entry names, including hidden names. `wc` counts
newline bytes, words separated by ASCII whitespace, and bytes, with a total for
multiple operands. `cat` concatenates bytes. `head` prints the first ten lines
of each input. These programs have no option parser.

`ls` defaults to `.`. The other programs default to stdin and accept `-` as an
operand for stdin. See [the runner](../../flatware/README.md) for directory
snapshots and output handling.
