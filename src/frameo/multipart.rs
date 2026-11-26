// SPDX-License-Identifier: GPL-3.0-or-later
//! Frameo TypeMultiPartMessage (30) fragmentation and bounded reassembly.

use std::collections::{BTreeMap, HashMap};

use anyhow::{Result, ensure};
use prost::Message;

use super::{
    proto::MultiPartMessage,
    wire::{Frame, MAX_TRANSPORT_MESSAGE, TYPE_MULTIPART},
};

const CHUNK_SIZE: usize = 16_316;
const MAX_PARTIAL: usize = 16;
const MAX_REASSEMBLED: usize = 64 << 20;

/// Split one application frame into transport-safe messages.
pub fn split_frame(frame: &Frame, message_id: i64) -> Result<Vec<Vec<u8>>> {
    let whole = frame.encode();
    if whole.len() <= MAX_TRANSPORT_MESSAGE {
        return Ok(vec![whole]);
    }
    ensure!(i32::try_from(whole.len()).is_ok(), "oversized Frameo application message");
    let mut result = Vec::new();
    for (index, chunk) in whole.chunks(CHUNK_SIZE).enumerate() {
        let part = MultiPartMessage {
            message_id,
            message_size: i32::try_from(whole.len())?,
            data_index: i32::try_from(index)?,
            message_data: chunk.to_vec(),
        };
        let wire = Frame::from_protobuf(TYPE_MULTIPART, &part).encode();
        ensure!(wire.len() <= MAX_TRANSPORT_MESSAGE, "Frameo multipart exceeds transport limit");
        result.push(wire);
    }
    Ok(result)
}

#[derive(Debug)]
struct Partial {
    size: usize,
    parts: BTreeMap<i32, Vec<u8>>,
    received: usize,
}

#[derive(Debug, Default)]
pub struct Reassembler {
    partial: HashMap<i64, Partial>,
}

impl Reassembler {
    pub fn new() -> Self { Self::default() }

    /// Returns a complete inner Frameo message or None for pending fragments.
    pub fn push(&mut self, packet: &Frame) -> Result<Option<Frame>> {
        if packet.kind != TYPE_MULTIPART {
            return Ok(Some(packet.clone()));
        }
        let fragment = MultiPartMessage::decode(packet.payload.as_slice())?;
        ensure!(fragment.message_size > 0, "invalid multipart size");
        let size = usize::try_from(fragment.message_size)?;
        ensure!(size <= MAX_REASSEMBLED, "multipart message exceeds memory limit");
        ensure!(fragment.data_index >= 0, "negative multipart index");
        let offset = usize::try_from(fragment.data_index)?
            .checked_mul(CHUNK_SIZE)
            .ok_or_else(|| anyhow::anyhow!("multipart offset overflow"))?;
        ensure!(offset < size, "multipart offset out of bounds");
        ensure!(!fragment.message_data.is_empty() && fragment.message_data.len() <= CHUNK_SIZE,
            "invalid multipart fragment size");
        ensure!(offset + fragment.message_data.len() <= size, "multipart fragment exceeds message");

        if !self.partial.contains_key(&fragment.message_id) {
            ensure!(self.partial.len() < MAX_PARTIAL, "too many incomplete Frameo messages");
            self.partial.insert(fragment.message_id, Partial {
                size, parts: BTreeMap::new(), received: 0
            });
        }
        let partial = self.partial.get_mut(&fragment.message_id)
            .expect("entry was just inserted");
        ensure!(partial.size == size, "multipart size changed");
        if let Some(previous) = partial.parts.get(&fragment.data_index) {
            ensure!(previous == &fragment.message_data, "conflicting duplicate multipart data");
        } else {
            partial.received += fragment.message_data.len();
            partial.parts.insert(fragment.data_index, fragment.message_data);
        }
        if partial.received < size {
            return Ok(None);
        }
        let mut data = Vec::with_capacity(size);
        for (&index, bytes) in &partial.parts {
            ensure!(
                usize::try_from(index)? * CHUNK_SIZE == data.len(),
                "multipart contains missing fragments"
            );
            data.extend_from_slice(bytes);
        }
        ensure!(data.len() == size, "multipart byte total does not match header");
        self.partial.remove(&fragment.message_id);
        Ok(Some(Frame::decode(&data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_frame_is_not_fragmented() {
        let f = Frame { kind: 1, payload: vec![1,2,3] };
        assert_eq!(split_frame(&f, 2).unwrap(), vec![f.encode()]);
    }

    #[test]
    fn multipart_reassembles_out_of_order() {
        let original = Frame { kind: 4, payload: vec![0xaa;70_000] };
        let mut chunks = split_frame(&original, 4321).unwrap();
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.len() <= MAX_TRANSPORT_MESSAGE));
        chunks.reverse();
        let mut r = Reassembler::new();
        let mut result = None;
        for chunk in chunks {
            result = r.push(&Frame::decode(&chunk).unwrap()).unwrap().or(result);
        }
        assert_eq!(result, Some(original));
    }

    #[test]
    fn refuses_corrupted_or_oversized_fragments() {
        let mut r = Reassembler::new();
        let f = Frame::from_protobuf(TYPE_MULTIPART, &MultiPartMessage {
            message_id: 2, message_size: -1, data_index: 0, message_data: vec![1],
        });
        assert!(r.push(&f).is_err());
    }
}
