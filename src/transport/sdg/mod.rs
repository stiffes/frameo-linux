//! SecureDeviceGrid (SDG) wire primitives.
//!
//! This module is deliberately below the application protocol layer. It does
//! not yet establish network connections or perform pairing.

pub mod crypto;
pub mod handshake;
mod identity;
pub mod message;
pub mod nonce;
mod packet;

pub use identity::{Identity, Key};
pub use nonce::Nonce;
pub use packet::{MAX_FRAME_SIZE, Packet, read_frame, write_frame};
