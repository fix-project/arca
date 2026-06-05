# Arca Data Pipe Protocol

This doc specifies the **data pipe**: how bytes move between Arca and
the Linux monitor for an established connection. The control protocol
(`PROTOCOL.md`) handles session setup and hands off a `DataPipeInfo`
(SHM handle + ring size); this doc covers everything after that handoff.

---

## 1. Overview

Each connection gets its own **bidirectional pipe**, a pair of
single-producer/single-consumer (SPSC) ring buffers in a shared-memory
region. One ring carries bytes Arca→Linux (outgoing), the other Linux→Arca
(incoming). The monitor relays bytes between the ring and the kernel
`TcpStream`; Arca reads and writes through `SyncStream` / `AsyncStream`.

```
  Arca                shared memory               Monitor
  ────                ─────────────               ───────
  SyncStream ──write──► Ring A (A→B) ──read──► pipe_to_tcp → TcpStream
  SyncStream ◄──read──  Ring B (B→A) ◄──write── tcp_to_pipe ← TcpStream
```

---

## 2. Ring layer (`arca-pipe`)

### 2.1 Ring buffer

Each ring is a fixed-capacity byte buffer in shared memory with a header
and a data region:

```
[RingHeader: 24 bytes][Data: ring_size bytes]
```

`RingHeader` (stored in shared memory, all fields atomic):

```
read_cursor:   AtomicU64   — monotonically increasing logical read position
write_cursor:  AtomicU64   — monotonically increasing logical write position
writer_closed: AtomicBool  — set by the producer when it will write no more
reader_closed: AtomicBool  — set by the consumer when it will read no more
```

Physical position is `cursor % ring_size`. Cursors never reset; wrapping
arithmetic handles overflow.

### 2.2 Read and write

Both operations are **non-blocking**, return immediately if the ring
is full or empty.

**`RingProducer::write(buf)`**
- Writes `min(buf.len(), free_space)` bytes into the ring.
- Returns the number of bytes actually written OR returns `WouldBlock` 
  if the ring is full (`free_space == 0`).

**`RingConsumer::read(buf)`**
- Reads `min(buf.len(), used_space)` bytes from the ring.
- Returns the number of bytes actually read OR returns `WouldBlock` if the 
  ring is empty (`used_space == 0`).

Callers that need blocking behavior loop on `WouldBlock` (see §3).

### 2.3 Close flags

Each ring has two close flags in its header, set independently by each end:

| Flag            | Set by   | Meaning                                      |
| --------------- | -------- | -------------------------------------------- |
| `writer_closed` | Producer | No more bytes will be written to this ring.  |
| `reader_closed` | Consumer | No more bytes will be read from this ring.   |

A ring is **closed** when both flags are set. A `BidirectionalPipe` is
**fully closed** when both of its rings are closed (all four flags set).

### 2.4 `BidirectionalPipe` layout and API

Total shared-memory size for one pipe:

```
required_size(ring_size) = 2 × (24 + ring_size)  bytes
```

Layout: `[HeaderA][DataA: ring_size][HeaderB][DataB: ring_size]`

Side A's producer writes to Ring A; Side B's consumer reads from Ring A, 
and vice versa for Ring B.

**Close API on `BidirectionalPipe`:**

| Method                   | What it does                                               |
| ------------------------ | ---------------------------------------------------------- |
| `close_write()`          | Sets `writer_closed` on the outgoing ring.                 |
| `close_read()`           | Sets `reader_closed` on the incoming ring.                 |
| `is_peer_write_closed()` | Reads `writer_closed` on the incoming ring (peer set it).  |
| `is_peer_read_closed()`  | Reads `reader_closed` on the outgoing ring (peer set it).  |
| `is_closed()`            | True when all four flags across both rings are set.        |

---

## 3. Where the code lives

```
arca/common/src/
├── pipe/                   # ring buffers, BidirectionalPipe
│   └── src/
│       ├── Pipe.md
│       ├── traits.rs       #   Read, Write traits; Write::write_all default
│       ├── ring.rs         #   RingHeader (cursors + close flags), RingData
│       ├── ring_producer.rs#   RingProducer — write, close_writer, is_reader_closed
│       ├── ring_consumer.rs#   RingConsumer — read, close_reader, is_writer_closed
│       └── bidirectional_pipe.rs  # BidirectionalPipe — close_write/read, is_closed
```
