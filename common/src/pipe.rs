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

mod bidirectional_pipe;
mod error;
mod ring;
mod ring_consumer;
mod ring_producer;
mod shared_memory_region;
mod traits;

pub use bidirectional_pipe::{BidirectionalPipe, ARCA_SIDE, HOST_SIDE};
pub use error::PipeError;
pub use ring::{RingData, RingHeader};
pub use ring_consumer::RingConsumer;
pub use ring_producer::RingProducer;
pub use shared_memory_region::SharedMemoryRegion;
pub use traits::{DoorBell, Read, Write};

#[cfg(feature = "std")]
pub use traits::DoorBellWaiter;

#[cfg(test)]
pub(crate) use bidirectional_pipe::{test_utils, Side};
