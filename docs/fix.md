# Fix

## Paper

Fix and its benefits are described and evaluated in detail in our paper:

> Fix: externalizing network I/O in serverless computing (PDF)
> Yuhan Deng, Akshay Srivatsan, Sebastian Ingino, Francis Chua, Yasmine Mitchell, Matthew Vilaysack, Keith Winstein 
> European Conference on Computer Systems (EuroSys), Edinburgh, U.K., 2026.

[Open on ACM](https://dl.acm.org/doi/10.1145/3767295.3769387)

## Overview

Fix provides a language-agnostic ABI for expressing data and computation
dependencies between arbitrary deterministic procedures.  The Fix types and API
as described in this document are not specific to any language or
implementation. We give example interfaces in [pseudo-Rust](api.rs),
[pseudo-bash](cli.sh), and [WebAssembly text](fix.wat).

## Names/Handles

Fix describes different ways to refer to data. Every datum within Fix has a
content-addressed _name_, determined by a hash of its contents and its length.

Fix procedures don't refer to data directly; instead, they generally refer to
_values_, which are a combination of a datum and an access policy. A _handle_
is a combination of a name and an access policy (and other metadata).  In our
implementation, a handle is represented by a WebAssembly ExternRef.

## Data

There are two fundamental types of data within Fix:

1. BlobData are arbitrary byte arrays.
2. TreeData are compound structures represented as arrays of Handles.

## Values

There are four value types (access policies for data). Some functions are
defined for multiple value types, while some are defined only for particular
subsets.

### Objects

Objects are the simplest value type. Blob Objects refer to BlobData that are
immediately available, and Tree Objects refer to TreeData that are immediately
available.

Tag Objects also exist; they are special 3-element Trees which represent a
claim. They are structured as follows:

1. Subject of claim
2. Claim
3. Author List

A procedure can only create a tag when its own identity is in the author list.
This restriction is enforced by the Fix Shell. This allows us to use tags as
"type constructors" to carry proofs in LCF style.

Blob Objects and Tree Objects (and Tag Objects) are sometimes referred to
simply as Blobs and Trees (and Tags).

In WebAssembly, Blobs are "attached" to WebAssembly memories, and Trees (and
Tags) are attached to WebAssembly externref tables.

### Refs

Refs are similar to objects, but refer to data which may not be immediately
available. For example, the data may be resident on a node other than the
current node. This means that a running procedure cannot directly access the
_contents_ of the data, but can access metadata about it (such as its length).

### Thunks

While Refs refer to data which may not be _present_, Thunks refer to data which
may not yet be _computed_.  That is, Thunks represent deferred computation.

A Thunk may be constructed in three different ways:
1. Application Thunks are defined by a "combination" of a function/procedure
   and its arguments, stored as a Tree; they represent the result of applying
   the function to those arguments.
2. Identification Thunks are defined by a Ref; they represent the identity
   function applied to the data of that Ref.
3. Selection Thunks are defined by a Tree consisting of a Ref and an index (or
   indices); they represent slicing the data of that Ref.

In our implementation, a "function" is usually an ELF blob containing
x86-64-unknown-arca machine code. In order to ensure determinism, we check that
the blob is "tagged" by the trusted compilation toolchain (a combination of
LLVM and wasm2c).

When trying to find the function in a combination, we recursively "drill" into
the 0th element of the combination until we reach a blob (`combination[0]`, or
`combination[0][0]`, or `combination[0][0][0]`, etc.).  This means we can
effectively "curry" arguments by storing them at the other positions in the
combination (`combination[0][1]`, etc.), as long as the procedure itself
understands the nested structure.

A special method of constructing a Thunk, only available via the Fix-on-Arca
runtime, is to capture a _continuation_ of a currently-executing procedure.
This returns a new tree ref representing the continuation, which can be
provided as the "procedure" of a new application thunk. This can be done
explicitly using `get_cc` or implicitly using `call_cc` or `eval`.

### Encodes

In order to induce the computation of a Thunk, it must be wrapped in an Encode.
An encode is an "Explicit Named Computation On Data (or ENCODE)".

An Encode may request two different types of evaluation:
1. Strict Encodes recursively evaluate and load a value, producing an object
   that contains no refs (even transitively).
2. Shallow Encodes perform the minimum amount of evaluation to produce a ref.

## Equality

Certain types of values in Fix can be deterministically compared for equality.
This class, Eq, can be recursively defined as:
1. Blobs
2. Trees containing only Eq values
3. Refs to Eq values

In essence, these are values where every bit of information is already
computed; they do not contain Thunks anywhere in their body. Whether a given
value is Eq or not is stored as a metadata bit on its handle.
