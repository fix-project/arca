import fix

(argument,) = fix.argv
assert argument[-1].equals(fix.Blob(b"right"))
assert str(argument[-1]) == "right"
assert bytes(fix.Blob("café")) == b"caf\xc3\xa9"
assert str(fix.Blob("café")) == "café"
try:
    fix.Blob("\ud800")
except UnicodeEncodeError:
    pass
else:
    raise AssertionError("expected a UnicodeEncodeError")
try:
    str(fix.Blob(b"\xff"))
except UnicodeDecodeError:
    pass
else:
    raise AssertionError("expected a UnicodeDecodeError")
assert int(fix.Blob(b"\xff")) == -1
assert int(fix.Blob(b"\x00\xff")) == -256
assert int(fix.Blob(b"\xfe\xff\xff\xff")) == -2
assert int(fix.Blob(b"\x00\x00\x00\x00\x01\x00\x00\x00")) == 1 << 32
assert int(fix.Blob(b"\xff" * 8)) == -1
assert bool(fix.Blob(-1, width=1)) is True
assert bool(fix.Blob(0)) is False
assert bytes(fix.Blob(True)) == b"\x01"
assert bytes(fix.Blob(False)) == b"\x00"
for high in ("true", "True"):
    assert bool(fix.Blob(high)) is True
for low in ("false", "False"):
    assert bool(fix.Blob(low)) is False
sequence = fix.Tree([1, "two", (False,)])
assert list(map(int, list(sequence)[:1])) == [1]
assert str(tuple(sequence)[1]) == "two"
assert bool(sequence[2][0]) is False
table = fix.Tree({"left": 1, b"right": [2, 3]})
assert int(dict(table)[b"left"]) == 1
assert list(map(int, table[b"right"])) == [2, 3]
assert bytes(fix.Blob(1 << 32)) == b"\x00\x00\x00\x00\x01\x00\x00\x00"
for width in (1, 2, 4, 8):
    minimum = -(1 << (width * 8 - 1))
    blob = fix.Blob(minimum, width=width)
    assert bytes(blob) == b"\x00" * (width - 1) + b"\x80"
    assert int(blob) == minimum
try:
    fix.Blob(128, width=1)
except OverflowError:
    pass
else:
    raise AssertionError("expected an OverflowError")
try:
    int(argument[-1])
except ValueError:
    pass
else:
    raise AssertionError("expected a ValueError for a five-byte Blob")
try:
    fix.Blob(0, width=3)
except ValueError:
    pass
else:
    raise AssertionError("expected a ValueError for a width of three")
assert len(argument) == 2
try:
    argument[2]
except IndexError:
    pass
else:
    raise AssertionError("expected an IndexError")
target = fix.Tree([fix.Tree([argument])]).reference().select(0)
word = target.select(0)[1]
blob = argument[-1]
assert blob[-1] == ord("t")
assert blob[1:4] == b"igh"
fix.exit([word.strict(), word[0].strict(), blob.reference().select(1).strict(),
          blob.reference().select(2).strict(), word[1:4].strict(),
          blob.reference().select(-2, 5).strict(), argument[0:1],
          argument.reference().select(1, 2).strict()])
