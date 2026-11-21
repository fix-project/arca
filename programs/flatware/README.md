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

## TeX

`//third_party/tex:tex` builds TeX for WASIp1 from TeX Live source. `tex/hello.fix` typesets
`tex/hello.tex` with INITEX and evaluates to the DVI file. `//programs/flatware:tex_test` checks its bytes.

This build has no `texmf.cnf`, so
`TEXINPUTS` and `TFMFONTS` name the directories searched for input files and
font metrics. `//third_party/tex:cmr10` copies `cmr10.tfm` from `texk/web2c/tests` in TeX Live.
