//! Bounded SDG TCP framing and the eight-byte tunnel packet header.

use std::io::{self, Read, Write};

const LENGTH_PREFIX_SIZE: usize = 2;
const MAGIC: [u8; 4] = [0xf0, 0x9f, 0x90, 0x9f];
const HEADER_SIZE: usize = 8;

/// Maximum body size representable by the SDG 16-bit length prefix.
pub const MAX_FRAME_SIZE: usize = u16::MAX as usize;

/// One SDG tunnel packet body, excluding the TCP length prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Packet {
    pub command: [u8; 4],
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new(command: [u8; 4], payload: Vec<u8>) -> Self {
        Self { command, payload }
    }

    pub fn encode_body(&self) -> Vec<u8> {
        let mut body = Vec::with_capacity(HEADER_SIZE + self.payload.len());
        body.extend_from_slice(&MAGIC);
        body.extend_from_slice(&self.command);
        body.extend_from_slice(&self.payload);
        body
    }

    pub fn decode_body(body: &[u8]) -> io::Result<Self> {
        if body.len() < HEADER_SIZE {
            return Err(invalid_data(format!(
                "SDG packet is {} bytes; header requires {HEADER_SIZE}",
                body.len()
            )));
        }
        if body[..4] != MAGIC {
            return Err(invalid_data("invalid SDG packet magic"));
        }
        let mut command = [0; 4];
        command.copy_from_slice(&body[4..8]);
        Ok(Self {
            command,
            payload: body[HEADER_SIZE..].to_vec(),
        })
    }

    pub fn tell() -> Self {
        Self::new(*b"TELL", Vec::new())
    }
}

/// Read one length-prefixed SDG frame, rejecting empty and over-limit bodies
/// before allocating the body buffer.
pub fn read_frame<R: Read>(reader: &mut R, max_len: usize) -> io::Result<Vec<u8>> {
    let mut prefix = [0; LENGTH_PREFIX_SIZE];
    reader.read_exact(&mut prefix)?;
    let len = usize::from(u16::from_be_bytes(prefix));

    if len == 0 {
        return Err(invalid_data("empty SDG frame"));
    }
    if len > max_len.min(MAX_FRAME_SIZE) {
        return Err(invalid_data(format!(
            "SDG frame of {len} bytes exceeds the configured limit {}",
            max_len.min(MAX_FRAME_SIZE)
        )));
    }

    let mut body = vec![0; len];
    reader
        .read_exact(&mut body)
        .map_err(|error| invalid_data(format!("truncated SDG frame body: {error}")))?;
    Ok(body)
}

/// Write one length-prefixed SDG frame.
pub fn write_frame<W: Write>(writer: &mut W, body: &[u8]) -> io::Result<()> {
    if body.is_empty() {
        return Err(invalid_data("cannot write an empty SDG frame"));
    }
    if body.len() > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("SDG frame is {} bytes; maximum is {MAX_FRAME_SIZE}", body.len()),
        ));
    }
    writer.write_all(&(body.len() as u16).to_be_bytes())?;
    writer.write_all(body)
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn tell_packet_matches_upstream_wire_fixture() {
        let mut frame = Vec::new();
        write_frame(&mut frame, &Packet::tell().encode_body()).unwrap();
        assert_eq!(hex(&frame), "0008f09f909f54454c4c");
    }

    #[test]
    fn packet_round_trip_preserves_command_and_payload() {
        let packet = Packet::new(*b"WELC", vec![1, 2, 3]);
        assert_eq!(Packet::decode_body(&packet.encode_body()).unwrap(), packet);
    }

    #[test]
    fn rejects_bad_magic_and_short_headers() {
        assert_eq!(
            Packet::decode_body(&[1, 2, 3]).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let mut bad = Packet::tell().encode_body();
        bad[0] ^= 0xff;
        assert!(Packet::decode_body(&bad).is_err());
    }

    #[test]
    fn read_write_frame_round_trip() {
        let expected = vec![0xab; 4096];
        let mut encoded = Vec::new();
        write_frame(&mut encoded, &expected).unwrap();
        assert_eq!(read_frame(&mut Cursor::new(encoded), 16_416).unwrap(), expected);
    }

    #[test]
    fn rejects_empty_oversized_and_truncated_frames() {
        assert!(read_frame(&mut Cursor::new([0, 0]), 100).is_err());
        assert!(read_frame(&mut Cursor::new([0xff, 0xff]), 100).is_err());
        assert!(read_frame(&mut Cursor::new([0, 3, 1]), 100).is_err());
        assert!(write_frame(&mut Vec::new(), &vec![0; MAX_FRAME_SIZE + 1]).is_err());
    }
}
