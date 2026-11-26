// SPDX-License-Identifier: GPL-3.0-or-later
//! Native Frameo photo upload built on authenticated SDG messages.
//!
//! This is transport-independent: socket setup, discovery and grid routing
//! are separate. Physical-device compatibility has not yet been verified.

use std::{
    fs,
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use prost::Message;

use super::{
    multipart::{Reassembler, split_frame},
    proto::{AcknowledgeReceipt, ClientInfo, FrameInfo, GetInfo, Media, MediaDataSegment},
    wire::{
        Frame, TYPE_ACKNOWLEDGE_RECEIPT, TYPE_CLIENT_INFO, TYPE_FRAME_INFO,
        TYPE_GET_INFO, TYPE_MEDIA, TYPE_MEDIA_DATA_SEGMENT,
    },
};
use crate::transport::sdg::handshake::EstablishedTunnel;

pub const PHOTO_SEGMENT_SIZE: usize = 16_000;
const MAX_UNRELATED_REPLIES: usize = 128;

/// A single ordered, authenticated message pipe to the frame.
pub trait MessageTransport {
    fn send_message(&mut self, data: &[u8]) -> Result<()>;
    fn receive_message(&mut self) -> Result<Vec<u8>>;
}

impl<S: Read + Write> MessageTransport for EstablishedTunnel<S> {
    fn send_message(&mut self, data: &[u8]) -> Result<()> { self.send(data) }
    fn receive_message(&mut self) -> Result<Vec<u8>> { self.receive() }
}

pub struct Client<T> {
    transport: T,
    name: String,
    next_id: i64,
    reassembler: Reassembler,
}

impl<T: MessageTransport> Client<T> {
    pub fn new(transport: T, name: impl Into<String>) -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64;
        Self { transport, name: name.into(), next_id: millis, reassembler: Reassembler::new() }
    }

    fn next_id(&mut self) -> Result<i64> {
        self.next_id = self.next_id.checked_add(1).context("Frameo message ID exhausted")?;
        Ok(self.next_id)
    }

    pub fn into_inner(self) -> T { self.transport }

    pub fn send_frame(&mut self, kind: i32, payload: &[u8]) -> Result<()> {
        let frame = Frame { kind, payload: payload.to_vec() };
        let message_id = self.next_id()?;
        for packet in split_frame(&frame, message_id)? {
            self.transport.send_message(&packet)?;
        }
        Ok(())
    }

    pub fn send_proto<P: Message>(&mut self, kind: i32, data: &P) -> Result<()> {
        self.send_frame(kind, &data.encode_to_vec())
    }

    pub fn receive_frame(&mut self) -> Result<Frame> {
        for _ in 0..MAX_UNRELATED_REPLIES {
            let bytes = self.transport.receive_message()?;
            let frame = Frame::decode(&bytes)?;
            let Some(frame) = self.reassembler.push(&frame)? else {
                continue;
            };
            if frame.kind == TYPE_GET_INFO && !self.name.is_empty() {
                self.send_proto(TYPE_CLIENT_INFO, &ClientInfo {
                    name: self.name.clone(), user_account_id: String::new(),
                    profile_picture_id: 0,
                })?;
                continue;
            }
            return Ok(frame);
        }
        anyhow::bail!("too many unrelated Frameo messages");
    }

    /// Request the frame's published identity and screen dimensions.
    pub fn get_info(&mut self) -> Result<FrameInfo> {
        self.send_proto(TYPE_GET_INFO, &GetInfo {})?;
        self.await_type(TYPE_FRAME_INFO, "FrameInfo")?.protobuf()
    }

    fn await_type(&mut self, kind: i32, label: &str) -> Result<Frame> {
        for _ in 0..MAX_UNRELATED_REPLIES {
            let frame = self.receive_frame()?;
            if frame.kind == kind { return Ok(frame); }
        }
        anyhow::bail!("Frameo did not send {label}");
    }

    /// Upload a photo using the Frameo Media/MediaDataSegment/AcknowledgeReceipt
    /// sequence. Caller must configure an I/O timeout on the underlying socket.
    pub fn send_photo(&mut self, data: &[u8], extension: &str, caption: &str, captured_ms: i64, fit: bool) -> Result<i64> {
        ensure!(!data.is_empty(), "photo is empty");
        ensure!(data.len() <= i32::MAX as usize, "photo exceeds Frameo 32-bit file length");
        let extension = extension.trim_start_matches('.').to_ascii_lowercase();
        let extension = if extension.is_empty() { "jpg" } else if extension == "jpeg" { "jpg" } else { &extension };
        let id = self.next_id()?;
        let ack_id = self.next_id()?;
        let media = Media {
            size: i32::try_from(data.len())?,
            caption: caption.into(),
            center_point_x: 0.5,
            center_point_y: 0.5,
            file_extension: extension.into(),
            id,
            media_type: 0, // PICTURE
            extra: Vec::new(),
            capture_date: captured_ms,
            content_id: id,
            error: None,
            scale_type: if fit { 1 } else { 2 }, // FIT_INSIDE / CENTER_POINT_CROP
        };
        self.send_proto(TYPE_MEDIA, &media)?;
        let count = data.len().div_ceil(PHOTO_SEGMENT_SIZE);
        for (index, piece) in data.chunks(PHOTO_SEGMENT_SIZE).enumerate() {
            self.send_proto(TYPE_MEDIA_DATA_SEGMENT, &MediaDataSegment {
                data: piece.to_vec(),
                requires_acknowledge_receipt_id: if index + 1 == count { ack_id } else { 0 },
            })?;
        }
        for _ in 0..MAX_UNRELATED_REPLIES {
            let frame = self.receive_frame()?;
            if frame.kind != TYPE_ACKNOWLEDGE_RECEIPT { continue; }
            let ack = AcknowledgeReceipt::decode(frame.payload.as_slice())?;
            if ack.acknowledge_id != ack_id { continue; }
            if let Some(error) = ack.error {
                ensure!(error.code == 0, "Frameo rejected photo (error {})", error.code);
            }
            return Ok(id);
        }
        anyhow::bail!("Frameo never acknowledged photo");
    }

    /// Read one local photo, retaining the reference sender's metadata rules.
    pub fn send_photo_file(&mut self, path: &Path, caption: &str, fit: bool) -> Result<i64> {
        let data = fs::read(path).with_context(|| format!("read photo {}", path.display()))?;
        let metadata = fs::metadata(path)?;
        let captured_ms = metadata.modified().unwrap_or_else(|_| SystemTime::now())
            .duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
            .min(i64::MAX as u128) as i64;
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("jpg");
        self.send_photo(&data, extension, caption, captured_ms, fit)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use super::*;

    #[derive(Default)]
    struct FakeTransport {
        sent: Vec<Vec<u8>>,
        replies: VecDeque<Vec<u8>>,
    }

    impl MessageTransport for FakeTransport {
        fn send_message(&mut self, msg: &[u8]) -> Result<()> {
            self.sent.push(msg.to_vec());
            Ok(())
        }

        fn receive_message(&mut self) -> Result<Vec<u8>> {
            self.replies.pop_front().context("fake server has no reply")
        }
    }

    #[test]
    fn sends_header_and_photo_segments_then_waits_for_ack() {
        let intro = Frame::from_protobuf(TYPE_GET_INFO, &GetInfo{}).encode();
        let ack = Frame::from_protobuf(TYPE_ACKNOWLEDGE_RECEIPT,
            &AcknowledgeReceipt { acknowledge_id: 102, error: None }).encode();
        let fake = FakeTransport { sent: vec![], replies: VecDeque::from([intro, ack]) };
        let mut client = Client::new(fake, "Framebridge");
        client.next_id = 100;
        let id = client.send_photo(&vec![0x7e;32_001], "jpeg", "Hello", 123456, true).unwrap();
        assert_eq!(id, 101);
        let sent = client.into_inner().sent;
        let decoded: Vec<Frame> = sent.iter().map(|v| Frame::decode(v).unwrap()).collect();
        assert_eq!(decoded[0].kind, TYPE_MEDIA);
        let media: Media = decoded[0].protobuf().unwrap();
        assert_eq!(media.size, 32_001);
        assert_eq!(media.file_extension, "jpg");
        assert_eq!(media.scale_type, 1);
        assert_eq!(media.caption, "Hello");
        assert_eq!(decoded.iter().filter(|f| f.kind == TYPE_MEDIA_DATA_SEGMENT).count(), 3);
        assert_eq!(decoded.last().unwrap().kind, TYPE_CLIENT_INFO);
        let chunks: Vec<MediaDataSegment> = decoded.iter()
            .filter(|f| f.kind == TYPE_MEDIA_DATA_SEGMENT)
            .map(|f| f.protobuf().unwrap()).collect();
        assert_eq!(chunks[0].data.len(), PHOTO_SEGMENT_SIZE);
        assert_eq!(chunks[1].data.len(), PHOTO_SEGMENT_SIZE);
        assert_eq!(chunks[2].data.len(), 1);
        assert_eq!(chunks[2].requires_acknowledge_receipt_id, 102);
    }

    #[test]
    fn rejects_error_ack_and_empty_photo() {
        let ack = Frame::from_protobuf(TYPE_ACKNOWLEDGE_RECEIPT,
            &AcknowledgeReceipt {
                acknowledge_id: 102,
                error: Some(super::super::proto::FrameError { code: 5, additional_int: 0 })
            }).encode();
        let fake = FakeTransport { sent: vec![], replies: VecDeque::from([ack]) };
        let mut client = Client::new(fake, "");
        client.next_id = 100;
        assert!(client.send_photo(&[], "jpg", "", 0, false).is_err());
        assert!(client.send_photo(b"x", "jpg", "", 0, false).is_err());
    }
}
