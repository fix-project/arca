//! Arca ↔ Linux **control protocol**.
//!
//! Layout of this crate:
//!
//! - [`protocol`]: the on-wire types — frame header, message catalog, payload
//!   structs ([`ControlFrame`], [`MessageType`], [`Endpoint`], …).
//! - Codec: [`read_frame`] / [`write_frame`] move frames over any
//!   `arca_pipe::Read`/`Write` byte transport (typically the dedicated
//!   control pipe, an `arca_pipe::BidirectionalPipe`).
//! - Arca side: [`ArcaSession`] owns the control pipe and exposes
//!   [`ArcaSession::bind`] / [`ArcaSession::connect`] / [`ArcaSession::accept`]
//!   — `accept` sends [`MessageType::AcceptRequest`] and waits for
//!   [`MessageType::IncomingConnection`].
//!
//! `no_std` throughout. The Linux-side counterpart lives in `arca-monitor`.

#![no_std]

mod arca_side;
mod codec;
mod message;
pub mod protocol;

pub use arca_side::{ArcaError, ArcaSession, ArcaTcpListener, ArcaTcpStream};
pub use codec::{read_frame, write_frame, FrameReadBuf, HEADER_LEN, MAX_WIRE_FRAME_LEN};
pub use message::{ControlReply, ControlRequest};
pub use protocol::*;
