#!/usr/bin/env false

# This file contains *pseudocode* corresponding to a hypothetical fix command-line interface.
# It is not expected to run.

# Create a blob from bytes (blob data).
BLOB0=$(fix create-blob < data0.txt)
BLOB1=$(fix create-blob < data1.txt)
# Opposite: `fix read-blob`

# Create a tree from handles (tree data).
# Could also be from standard input if no arguments were passed.
TREE=$(fix create-tree $BLOB0 $BLOB1)
# Opposite: `fix read-tree`

# Turn an object into a ref.
# Could also be from standard input if no argument was passed.
REF=$(fix create-ref $TREE)

# Turn a ref into an application thunk.
# We could also directly create it from the tree handle; the CLI would auto-convert.
THUNK=$(fix create-application-thunk $REF)
# Alternatives:
# THUNK=$(fix create-identification-thunk $REF)
# THUNK=$(fix create-selection-thunk $REF)

# Encode a Thunk.
ENCODE=$(fix create-strict-encode $THUNK)
# Alternative:
# ENCODE=$(fix create-encode $THUNK)

# Adds a "label" (non-normative user-friendly alias) for a handle.
# This acts as a garbage collection root.
# Labels can also be specified on create-blob, create-tree, etc. using `-l`.
fix label my-encode $ENCODE

# Helper (not a core part of Fix) to turn a UNIX directory into a Fix tree:
fix create-directory ./foo
# Opposite: `fix read-directory ./foo`
