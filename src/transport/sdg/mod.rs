//! SecureDeviceGrid (SDG) wire primitives.
//!
//! This module is deliberately below the application protocol layer. It does
//! not yet establish network connections or perform pairing.

mod crypto;
mod identity;
mod nonce;
mod packet;

pub use identity::{Identity, Key};
pub use nonce::Nonce;
pub use packet::{Packet, MAX_FRAME_SIZE, read_frame, write_frame};
