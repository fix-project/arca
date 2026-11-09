# Minimal Flatware utilities

`//programs/flatware:NAME_elf` builds each utility as a Fix ELF. Run them using the Flatware ABI,
and move files in and out with `fix flatware-import` and `fix flatware-export`.

`ls` prints sorted directory entry names, including hidden names. `wc` counts
newline bytes, words separated by ASCII whitespace, and bytes, with a total for
multiple operands. `cat` concatenates bytes. `head` prints the first ten lines
of each input. These programs have no option parser.

`ls` defaults to `.`. The other programs default to stdin and accept `-` as an
operand for stdin. See [Flatware](../../flatware/README.md) for directory
snapshots.
