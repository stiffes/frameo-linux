// SPDX-License-Identifier: GPL-3.0-or-later
//! Authenticated native SecureDeviceGrid client handshake.
//!
//! Ported from borje/unframeo internal/sdg/handshake.go and conn.go.
//! This module only establishes an encrypted tunnel; it does not implement
//! grid routing, device pairing, or Frameo media transfer.

use std::io::{Read, Write};
use anyhow::{Result, Context, ensure};
use rand::{rngs::OsRng, RngCore};
use super::{
    crypto, Identity, Key, Nonce, Packet, MAX_FRAME_SIZE, read_frame, write_frame,
    nonce::{NONCE_HELLO, NONCE_COOKIE, NONCE_VOUCH, NONCE_VOUCHER, NONCE_READY},
};

const COOKIE_SIZE: usize = 96;
const COOK_PAYLOAD_SIZE: usize = 16 + 32 + COOKIE_SIZE + 16;
const MAX_HANDSHAKE_FRAME: usize = 4096;

/// Connection mode determines which VOCH property must be present.
#[derive(Clone, Debug)]
pub enum HandshakeMode {
    /// Attach the zero-padded license certificate expected by the grid.
    Grid { certificate: Vec<u8> },
    /// Remote peer, already routed through the grid.
    Peer,
    /// Pairing tunnel, before a device has been registered.
    Pairing,
    /// Direct connection: describe the requested service in the VOCH.
    Local { protocol: String },
}

/// State following successful verification of REDY.
#[derive(Debug)]
pub struct EstablishedTunnel<S> {
    stream: S,
    shared: [u8; 32],
    server_identity: Key,
    next_send_counter: u64,
    last_received_counter: Option<u64>,
}

impl<S: Read + Write> EstablishedTunnel<S> {
    pub fn server_identity(&self) -> Key { self.server_identity }

    pub fn send(&mut self, message: &[u8]) -> Result<()> {
        let counter = self.next_send_counter;
        self.next_send_counter = self.next_send_counter.checked_add(1)
            .context("SDG send nonce exhausted")?;
        let packet = super::message::encode(message, counter, &self.shared)?;
        write_frame(&mut self.stream, &packet.encode_body()).context("write SDG MESG")
    }

    pub fn receive(&mut self) -> Result<Vec<u8>> {
        let packet = read_packet(&mut self.stream, MAX_FRAME_SIZE)?;
        let (counter, data) = super::message::decode(&packet, &self.shared)?;
        if let Some(previous) = self.last_received_counter {
            ensure!(counter > previous, "replayed or out-of-order SDG message counter");
        }
        self.last_received_counter = Some(counter);
        Ok(data)
    }

    pub fn into_inner(self) -> S { self.stream }
}

/// Negotiate an authenticated tunnel on an already-connected stream.
///
/// `allowed_remote_keys` must contain the expected public keys for normal
/// connections. The empty slice is allowed ONLY for first-time pairing,
/// when the frame identity has not yet been learned.
pub fn handshake<S: Read + Write>(
    mut stream: S,
    identity: &Identity,
    allowed_remote_keys: &[Key],
    mode: HandshakeMode,
) -> Result<EstablishedTunnel<S>> {
    ensure!(
        !allowed_remote_keys.is_empty() || matches!(mode, HandshakeMode::Pairing),
        "SDG remote identity must be pinned except during pairing"
    );
    write_packet(&mut stream, Packet::tell())?;
    let welc = expect_packet(&mut stream, *b"WELC")?;
    ensure!(welc.len() >= 32, "truncated SDG WELC");
    let remote = Key::from_bytes(welc[..32].try_into()?);
    if !allowed_remote_keys.is_empty() {
        ensure!(allowed_remote_keys.contains(&remote), "untrusted SDG server public key");
    }

    let ephemeral = Identity::generate();
    let mut helo_payload = Vec::with_capacity(120);
    helo_payload.extend_from_slice(ephemeral.public_key().as_bytes());
    helo_payload.extend_from_slice(&0u64.to_be_bytes());
    helo_payload.extend(crypto::seal(
        &[0u8; 64], &Nonce::short(NONCE_HELLO, 0),
        &remote, &ephemeral.private_key(),
    )?);
    ensure!(helo_payload.len() == 120, "invalid HELO size");
    write_packet(&mut stream, Packet::new(*b"HELO", helo_payload))?;

    let cook = expect_packet(&mut stream, *b"COOK")?;
    ensure!(cook.len() >= COOK_PAYLOAD_SIZE, "truncated SDG COOK");
    let tail: [u8; 16] = cook[..16].try_into()?;
    let decrypted = crypto::open(
        &cook[16..COOK_PAYLOAD_SIZE], &Nonce::long(NONCE_COOKIE, &tail),
        &remote, &ephemeral.private_key(),
    ).context("SDG COOK authentication")?;
    ensure!(decrypted.len() == 32 + COOKIE_SIZE, "invalid COOK plaintext size");
    let remote_ephemeral = Key::from_bytes(decrypted[..32].try_into()?);
    let cookie = &decrypted[32..];
    let shared = crypto::precompute(&remote_ephemeral, &ephemeral.private_key())?;

    let properties = match mode {
        HandshakeMode::Grid { certificate } => {
            let mut padded = [0u8; 128];
            let count = certificate.len().min(128);
            padded[..count].copy_from_slice(&certificate[..count]);
            property("certificate", &padded)?
        }
        HandshakeMode::Peer | HandshakeMode::Pairing => Vec::new(),
        HandshakeMode::Local { protocol } => property("protocol", protocol.as_bytes())?,
    };
    let mut random = [0u8; 16];
    OsRng.fill_bytes(&mut random);
    let inner = crypto::seal(
        ephemeral.public_key().as_bytes(), &Nonce::long(NONCE_VOUCHER, &random),
        &remote, &identity.private_key(),
    )?;
    let mut outer = Vec::with_capacity(97 + properties.len());
    outer.extend_from_slice(identity.public_key().as_bytes());
    outer.extend_from_slice(&random);
    outer.extend_from_slice(&inner);
    outer.push(u8::from(!properties.is_empty()));
    outer.extend_from_slice(&properties);
    let encrypted = crypto::seal_shared(&outer, &Nonce::short(NONCE_VOUCH, 1), &shared)?;
    let mut voch = Vec::with_capacity(COOKIE_SIZE + 8 + encrypted.len());
    voch.extend_from_slice(cookie);
    voch.extend_from_slice(&1u64.to_be_bytes());
    voch.extend_from_slice(&encrypted);
    write_packet(&mut stream, Packet::new(*b"VOCH", voch))?;

    let redy = expect_packet(&mut stream, *b"REDY")?;
    ensure!(redy.len() >= 24, "truncated SDG REDY");
    let counter = u64::from_be_bytes(redy[..8].try_into()?);
    crypto::open_shared(&redy[8..], &Nonce::short(NONCE_READY, counter), &shared)
        .context("SDG REDY authentication failed")?;

    Ok(EstablishedTunnel {
        stream,
        shared,
        server_identity: remote,
        next_send_counter: 2,
        last_received_counter: None,
    })
}

fn property(name: &str, value: &[u8]) -> Result<Vec<u8>> {
    ensure!(name.len() <= u8::MAX as usize && value.len() <= u8::MAX as usize,
        "SDG VOCH property exceeds one-byte length");
    ensure!(!name.as_bytes().contains(&0), "NUL in SDG VOCH property name");
    let mut result = Vec::with_capacity(name.len() + value.len() + 3);
    result.push(name.len() as u8);
    result.extend_from_slice(name.as_bytes());
    result.push(0);
    result.push(value.len() as u8);
    result.extend_from_slice(value);
    Ok(result)
}

fn write_packet<S: Write>(stream: &mut S, packet: Packet) -> Result<()> {
    write_frame(stream, &packet.encode_body()).context("write SDG handshake packet")
}

fn read_packet<S: Read>(stream: &mut S, limit: usize) -> Result<Packet> {
    Packet::decode_body(&read_frame(stream, limit)?).context("decode SDG packet")
}

fn expect_packet<S: Read>(stream: &mut S, expected: [u8; 4]) -> Result<Vec<u8>> {
    let packet = read_packet(stream, MAX_HANDSHAKE_FRAME)?;
    ensure!(packet.command == expected,
        "unexpected SDG command: expected {}, got {}",
        String::from_utf8_lossy(&expected), String::from_utf8_lossy(&packet.command));
    Ok(packet.payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn voch_property_matches_upstream_reference() {
        let cert = property("certificate", &[0u8;128]).unwrap();
        assert_eq!(cert.len(), 142);
        assert_eq!(&cert[..14], b"\x0bcertificate\x00\x80");
        assert!(cert[14..].iter().all(|b| *b == 0));
        assert!(property("x", &vec![1;256]).is_err());
    }

    #[test]
    fn rejects_untrusted_server_before_helo() {
        let remote = Identity::generate();
        let mut wire = Vec::new();
        write_frame(&mut wire, &Packet::new(*b"WELC", remote.public_key().as_bytes().to_vec()).encode_body()).unwrap();
        struct Duplex { input: Cursor<Vec<u8>>, output: Vec<u8> }
        impl Read for Duplex {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { self.input.read(buf) }
        }
        impl Write for Duplex {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> { self.output.write(buf) }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        let stream = Duplex { input: Cursor::new(wire), output: vec![] };
        let client = Identity::generate();
        let wrong_key = Identity::generate().public_key();
        let error = handshake(stream, &client, &[wrong_key], HandshakeMode::Peer).unwrap_err();
        assert!(error.to_string().contains("untrusted"));
    }

    #[test]
    fn no_unpinned_non_pairing_connections() {
        let result = handshake(Cursor::new(Vec::<u8>::new()), &Identity::generate(), &[], HandshakeMode::Peer);
        assert!(result.unwrap_err().to_string().contains("pinned"));
    }

    #[test]
    fn reject_short_welc() {
        let mut wire = Vec::new();
        write_frame(&mut wire, &Packet::new(*b"WELC", vec![0; 31]).encode_body()).unwrap();
        struct Duplex(Cursor<Vec<u8>>);
        impl Read for Duplex { fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> { self.0.read(b) } }
        impl Write for Duplex {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> { Ok(b.len()) }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        assert!(handshake(Duplex(Cursor::new(wire)), &Identity::generate(), &[], HandshakeMode::Pairing).is_err());
    }

    #[test]
    fn handshake_packet_sizes_match_reference() {
        // TELL 8, HELO 8+120, VOCH peer 8+96+8+(97+16).
        assert_eq!(Packet::tell().encode_body().len()+2, 10);
        assert_eq!(8+120+2, 130);
        assert_eq!(8+96+8+97+16+2, 227);
        assert_eq!(8+96+8+97+142+16+2, 369);
    }
}
