// SPDX-License-Identifier: GPL-3.0-or-later
//! Frameo's application envelope, matching unframeo internal/frameo/frame.go.
//!
//! The envelope is *inside* an encrypted SDG MESG. It is not TCP framing.

use anyhow::{Result, ensure};
use prost::Message;

pub const FRAME_PREFIX: u32 = 18;
pub const HEADER_SIZE: usize = 8;
pub const MAX_TRANSPORT_MESSAGE: usize = 16_416;

pub const TYPE_GET_INFO: i32 = 1;
pub const TYPE_FRAME_INFO: i32 = 2;
pub const TYPE_CLIENT_INFO: i32 = 3;
pub const TYPE_MEDIA: i32 = 4;
pub const TYPE_MEDIA_DATA_SEGMENT: i32 = 5;
pub const TYPE_ACKNOWLEDGE_RECEIPT: i32 = 6;
pub const TYPE_GET_MEDIA: i32 = 23;
pub const TYPE_REQUEST_PERMISSION: i32 = 27;
pub const TYPE_MULTIPART: i32 = 30;
pub const TYPE_GET_ALL_MEDIA_METADATA: i32 = 31;
pub const TYPE_ALL_MEDIA_METADATA: i32 = 32;
pub const TYPE_CHANGE_MEDIA_VISIBILITY: i32 = 33;
pub const TYPE_DELETE_MEDIA: i32 = 34;

/// Decoded Frameo application message.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Frame {
    pub kind: i32,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(HEADER_SIZE + self.payload.len());
        data.extend_from_slice(&FRAME_PREFIX.to_be_bytes());
        data.extend_from_slice(&self.kind.to_be_bytes());
        data.extend_from_slice(&self.payload);
        data
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        ensure!(data.len() >= HEADER_SIZE, "short Frameo message header");
        ensure!(
            u32::from_be_bytes(data[..4].try_into()?) == FRAME_PREFIX,
            "invalid Frameo message prefix"
        );
        Ok(Self {
            kind: i32::from_be_bytes(data[4..8].try_into()?),
            payload: data[8..].to_vec(),
        })
    }

    pub fn from_protobuf<T: Message>(kind: i32, message: &T) -> Self {
        Self {
            kind,
            payload: message.encode_to_vec(),
        }
    }

    pub fn protobuf<T: Message + Default>(&self) -> Result<T> {
        Ok(T::decode(self.payload.as_slice())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_matches_upstream_frameo_envelope() {
        assert_eq!(
            Frame {
                kind: TYPE_GET_INFO,
                payload: vec![]
            }
            .encode(),
            [0, 0, 0, 18, 0, 0, 0, 1]
        );
        let input = Frame {
            kind: TYPE_MEDIA,
            payload: vec![1, 2, 3],
        };
        assert_eq!(Frame::decode(&input.encode()).unwrap(), input);
        assert!(Frame::decode(&[0; 7]).is_err());
        assert!(Frame::decode(&[0; 8]).is_err());
    }
}
