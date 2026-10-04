# Flatware

Flatware is a compatibility layer allowing standard WASIp1 applications to run
on Fix. It is an implementation of WASIp1 against the Fix API.

## Input Tree

A Flatware procedure is called with a combination of this form:

```
Tree: apply combination
├─ Tag: executable procedure (as usual for Fix)
├─ Tree: command-line arguments
│	├─ Blob: string argument (argv[0])
│	└─ ...
├─ Tree: environment
│	├─ Blob: "NAME=VALUE" pair (envp[0])
│	└─ ...
├─ Tree: initial descriptors
│	├─ Tree: preopen descriptor 0
│	└─ ...
├─ Blob: random seed (64 bits)
└─ Blob: initial timestamp (8 bytes, little-endian)
```

The random seed is used to seed a pRNG; the algorithm is unspecified.

Timestamps are in nanoseconds. Every WASIp1 operation increments the current
timestamp by one nanosecond; this allows programs to perceive the passage of
time deterministically.

## Output Tree

The result of a Flatware procedure is a tree of this form:

```
Tree: output
├─ Tree: final descriptors
│	├─ Tree: modified preopen descriptor 0
│	└─ ...
└─ Blob: exit status (4 bytes, little endian)
```

## Descriptors

A descriptor in WASIp1 may refer to a file or a directory (or a character
device, block device, symlink, etc.).

By convention, WASIp1 provides descriptors 0, 1, and 2 as stdin, stdout, and
stderr respectively. Descriptors 3+ may also be provided as "preopens", i.e.,
mounted directories.

Some examples of preopened structures are as follows:

```
Tree: preopen descriptor (stdin)
├─ Blob: name ("stdin")
├─ Tree: stat info
│	├─ Blob: st_dev (e.g., 0 for "fix")
│   └─ ...
└─ Blob: contents ("hello world")
```

```
Tree: preopen descriptor (stdout)
├─ Blob: name ("stdout")
├─ Tree: stat info
│	├─ Blob: st_dev (e.g., 0 for "fix")
│   └─ ...
└─ Blob: initial contents ("")
```

```
Tree: preopen descriptor (directory)
├─ Blob: name ("/", also used as mount point)
├─ Tree: stat info
│	├─ Blob: st_dev (e.g., 0 for "fix")
│   └─ ...
└─ Tree: contents (dirents using the same descriptor format)
 	├─ Tree: dirents[0]
    └─ ...
```

```
Tree: preopen descriptor (directory image)
├─ Blob: name ("/home", also used as mount point)
├─ Tree: stat info
│	├─ Blob: st_dev (e.g., 1 for "cpio")
│   └─ ...
└─ Blob: contents (cpio format)
```

When using the "fix" format for nested directories, those directory entries are
represented in the same format. The "contents" field may be a BlobRef or
TreeRef rather than being an object.

## Stat Info

```
Tree: stat info
├─ Blob: device
├─ Blob: inode
├─ Blob: filetype
├─ Blob: link count
├─ Blob: size
├─ Blob: atime
├─ Blob: mtime
└─ Blob: ctime
```

Filetype is one byte, the other fields are eight-byte little-endian integers.

Size is in bytes for files and entries for directories.

Device numbers:

| Device Number | Meaning      |
|---------------|--------------|
| 0             | Fix-native   |
| 1             | CPIO Archive |
| 2             | I/O Effects  |
