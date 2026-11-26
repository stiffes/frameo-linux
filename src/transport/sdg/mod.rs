//! SecureDeviceGrid (SDG) wire primitives.
//!
//! This module is deliberately below the application protocol layer. It does
//! provides an authenticated tunnel and pairing proof, but not grid routing or media upload.

pub mod crypto;
pub mod handshake;
mod identity;
pub mod message;
pub mod nonce;
mod packet;
pub mod pairing;

pub use identity::{Identity, Key};
pub use nonce::Nonce;
pub use packet::{MAX_FRAME_SIZE, Packet, read_frame, write_frame};
