//! # arca-pipe
//!
//! A `no_std`-compatible, lock-free bidirectional byte pipe built from two
//! single-producer, single-consumer (SPSC) ring buffers over shared memory.
//!
//! This is the lowest-level transport primitive in arca-networking — both the
//! control protocol and per-connection data streams are built on top of it.
//!
//! The pipe is a raw byte stream with no message framing. Higher layers
//! (control protocol, data protocol) add their own framing on top.

#![no_std]

mod error;
mod traits;
mod ring;
mod ring_producer;
mod ring_consumer;
mod shared_memory_region;
mod bidirectional_pipe;

pub use error::PipeError;
pub use traits::{Read, Write};
pub use ring::{RingData, RingHeader};
pub use ring_producer::RingProducer;
pub use ring_consumer::RingConsumer;
pub use shared_memory_region::SharedMemoryRegion;
pub use bidirectional_pipe::{BidirectionalPipe, Side};
