//! SDG encrypted message payloads, independent of socket transport.
//! Wire behavior verified against the pinned reference in docs/UPSTREAM_REFERENCE.md.

use anyhow::{Result, ensure};

use super::{
    MAX_FRAME_SIZE, Nonce, Packet, crypto,
    nonce::{NONCE_CLIENT_MESSAGE, NONCE_SERVER_MESSAGE},
};

/// Maximum application payload fitting a length-prefixed SDG MESG packet.
pub const MAX_MESSAGE_SIZE: usize = MAX_FRAME_SIZE - 8 - 8 - 16 - 2;

/// Encode a client-to-server MESG. The caller must never reuse a counter.
pub fn encode(data: &[u8], counter: u64, shared: &[u8; 32]) -> Result<Packet> {
    ensure!(data.len() <= MAX_MESSAGE_SIZE, "SDG message too large");
    let mut plain = Vec::with_capacity(data.len() + 2);
    plain.extend_from_slice(&(data.len() as u16).to_be_bytes());
    plain.extend_from_slice(data);
    let nonce = Nonce::short(NONCE_CLIENT_MESSAGE, counter);
    let mut payload = counter.to_be_bytes().to_vec();
    payload.extend(crypto::seal_shared(&plain, &nonce, shared)?);
    Ok(Packet::new(*b"MESG", payload))
}

/// Decode a server-to-client MESG, returning its authenticated counter.
/// Replay protection belongs to the connection state machine.
pub fn decode(packet: &Packet, shared: &[u8; 32]) -> Result<(u64, Vec<u8>)> {
    ensure!(packet.command == *b"MESG", "expected SDG MESG");
    ensure!(packet.payload.len() >= 24, "truncated SDG MESG");
    ensure!(
        packet.payload.len() <= MAX_FRAME_SIZE - 8,
        "SDG MESG exceeds frame limit"
    );
    let counter = u64::from_be_bytes(packet.payload[..8].try_into()?);
    let nonce = Nonce::short(NONCE_SERVER_MESSAGE, counter);
    let plain = crypto::open_shared(&packet.payload[8..], &nonce, shared)?;
    ensure!(plain.len() >= 2, "missing SDG message length");
    let length = usize::from(u16::from_be_bytes(plain[..2].try_into()?));
    ensure!(length <= plain.len() - 2, "truncated SDG message data");
    Ok((counter, plain[2..2 + length].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::sdg::write_frame;

    fn key() -> [u8; 32] {
        hex::decode("e6c55502fce47eaddcf296ef4219ed87eba5692dc84bedf702ea8915ba744d30")
            .unwrap()
            .try_into()
            .unwrap()
    }

    #[test]
    fn client_message_matches_reference_frame() {
        let packet = encode(b"frameo", 2, &key()).unwrap();
        let mut wire = Vec::new();
        write_frame(&mut wire, &packet.encode_body()).unwrap();
        assert_eq!(
            hex::encode(wire),
            "0028f09f909f4d45534700000000000000022aae70bb95881fa9c2a612d7f5d401fc291d6e7f2fabe4a3"
        );
    }

    #[test]
    fn server_message_authentication_and_length() {
        let counter = 7u64;
        let nonce = Nonce::short(NONCE_SERVER_MESSAGE, counter);
        let mut payload = counter.to_be_bytes().to_vec();
        payload.extend(crypto::seal_shared(b"\x00\x03abc", &nonce, &key()).unwrap());
        let mut packet = Packet::new(*b"MESG", payload);
        assert_eq!(decode(&packet, &key()).unwrap(), (7, b"abc".to_vec()));
        packet.payload[8] ^= 1;
        assert!(decode(&packet, &key()).is_err());

        let mut malformed = counter.to_be_bytes().to_vec();
        malformed.extend(crypto::seal_shared(b"\x00\x04abc", &nonce, &key()).unwrap());
        assert!(decode(&Packet::new(*b"MESG", malformed), &key()).is_err());
        assert!(decode(&Packet::new(*b"MESG", vec![0; 23]), &key()).is_err());
        assert!(encode(&vec![0; MAX_MESSAGE_SIZE + 1], 0, &key()).is_err());
    }
}
