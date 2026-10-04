# Arca

## Paper

The Arca kernel and the benefits of continuation-centric computing are described and evaluated in our paper:

> Continuation-Centric Computing with Arca
> Akshay Srivatsan, Yuhan Deng, Katherine Mohr, Emma Sudo, Sebastian Ingino, Francis Chua, Keith Winstein 
> 20th USENIX Symposium on Operating Systems Design and Implementation (OSDI 26), Seattle, WA, 2026

[Open on USENIX](https://www.usenix.org/conference/osdi26/presentation/srivatsan)

## Overview

Arca is an operating system kernel designed around the concept of
_continuations._ This means that an executing Arca process can, at any point,
snapshot itself. That snapshot is reified as an object that can be passed
between processes, inspected, manipulated, copied, or resumed.

## Data Model

Arca differs from traditional Unix-like operating systems by providing a
_functional_ programming inspired model. Instead of files, directories, and pipes, it provides:

1. Words: machine words (register-sized)
2. Blobs: byte arrays (allocated on the kernel heap)
3. Tuples: collections of values (allocated on the kernel heap)
4. Pages: machine pages (on x86-64, 4 KiB, 2 MiB, or 1 GiB)
5. Tables: machine page tables (on x86-64, 2 MiB, 1 GiB, or 512 GiB)
6. Functions: runnable/invokable subprograms

Functions can be "arcane" (defined by an Arca process) or "symbolic" (defined
by a symbolic name, usually a Blob representing a ASCII string). Symbolic
functions are used for effect handling.

An Arca process consists of a register file, a page table, and a "descriptor
table". This descriptor table is how Arca programs can manipulate their
arguments and internal state; it is similar to a file descriptor table in Unix.

## Limitations

In order to make continuation-centric computing efficient, Arca imposes limits on functions:
1. They can only manipulate local state (their own descriptor table).
2. They cannot have shared mutable state.
3. They cannot directly perform I/O; they must instead trigger an effect to be
   handled by a provider.
