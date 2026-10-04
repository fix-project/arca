# Fix Handles

Every value in Fix is referred to by a _handle_.  A handle combines a _name_ of
some underlying data with metadata bits describing how that data is to be
interpreted (roughly, a type annotation).

## Handle Format

Handles are designed to be 32 bytes long, such that they will fit in a u8x32
SIMD register on modern hardware (`ymm` registers). They are also formatted
such that common operations become simple, efficient, bit operations.

### Type
The right-most byte (31) of the handle is its type tag. Its contents are
interpreted as follows:


| Byte 31    | Meaning               |
|------------|-----------------------|
| `xxxxxx00` | No ENCODE             |
| `xxxxxx01` | Strict ENCODE         |
| `xxxxxx10` | Shallow ENCODE        |
| `xxxxxx11` | Invalid (Unassigned)  |
| `xxxx00xx` | No Thunk              |
| `xxxx01xx` | Application Thunk     |
| `xxxx10xx` | Identification Thunk  |
| `xxxx11xx` | Selection Thunk       |
| `xxx0xxxx` | Object                |
| `xxx1xxxx` | Ref                   |
| `000xxxxx` | Blob Data (Literal)     |
| `100xxxxx` | Blob Data (Named)   |
| `101xxxxx` | Tree Data (Not a Tag) |
| `111xxxxx` | Tree Data (Tag)       |


An all-zero handle is therefore a zero-length literal blob.

### Metadata

The second-to-rightmost byte (30) of the handle is a metadata field. 

For literal blobs

| Byte 30, Bits | Meaning                     |
|---------------|-----------------------------|
| 0:4           | Length (<= 30)              |
| 5:7           | Unassigned (should be zero) |


In all other cases:

| Byte 30, Bits | Meaning                                                 |
|---------------|---------------------------------------------------------|
| 0:1           | Name type (00 = canonical, 10 = machine, 11 = local)    |
| 2             | Eq (0 = equality is undefined, 1 = equality is defined) |
| 3:7           | Unassigned (should be zero)                             |


### Name

The other 30 bytes of the handle are the name. This has several formats:

#### Literal Name

| Bytes | Meaning                                                      |
|-------|--------------------------------------------------------------|
| 0:29  | The literal contents of the blob; remaining bytes are zeros. |


Literal names *are* the canonical names of short blobs. It is incorrect to
generate a canonical-but-not-literal name for a short blob.

#### Canonical Name

| Bytes | Meaning                                            |
|-------|----------------------------------------------------|
| 0:23  | The first 24 bytes of the BLAKE3 hash of the data. |
| 24:29 | The length of the data in bytes.                  |


For both BlobData and TreeData, the hash is of payload bytes---in the case of
TreeData, this means it is a hash of the _handles_ in the tree. It is not
possible to have a canonical name for a tree containing non-canonical names.

For BlobData, the length must be greater than or equal to 31. Short blobs must
use literal names.

#### Machine Name

| Bytes | Meaning                                                   |
|-------|-----------------------------------------------------------|
| 0:7   | The index of the corresponding data in in-memory storage. |
| 8:15  | Storage ID.                                               |
| 16:23 | Unassigned (should be zero)                               |
| 24:29 | The length of the data in bytes.                          |


#### Local Name

| Bytes | Meaning                                                           |
|-------|-------------------------------------------------------------------|
| 0:7   | The base virtual address of the data in the active address space. |
| 8:23  | Unassigned (should be zero)                                       |
| 24:29 | The length of the data in bytes.                                  |
