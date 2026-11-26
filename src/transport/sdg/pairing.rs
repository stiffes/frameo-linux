// SPDX-License-Identifier: GPL-3.0-or-later
//! Native Frameo SecureDeviceGrid OTP challenge-response pairing.
//!
//! Derived from borje/unframeo internal/sdg/pairing.go and its pinned
//! opensdg/libsodium reference vector. This does not perform grid routing.

use anyhow::{Result, ensure};
use rand::{RngCore, rngs::OsRng};
use salsa20::{
    XSalsa20,
    cipher::{KeyIvInit, StreamCipher},
};
use sha2::{Digest, Sha512};
use subtle::ConstantTimeEq;
use x25519_dalek::{PublicKey, StaticSecret};

use super::{Identity, Key, handshake::EstablishedTunnel};

const CHALLENGE: u8 = 3;
const RESPONSE: u8 = 4;
const RESULT: u8 = 5;

#[derive(Debug)]
pub struct PairingChallenge {
    pub x: [u8; 32],
    pub nonce: [u8; 32],
    pub y: [u8; 32],
}

impl PairingChallenge {
    pub fn decode(packet: &[u8]) -> Result<Self> {
        ensure!(packet.len() == 97, "invalid SDG pairing challenge length");
        ensure!(packet[0] == CHALLENGE, "expected SDG pairing challenge");
        Ok(Self {
            x: packet[1..33].try_into()?,
            nonce: packet[33..65].try_into()?,
            y: packet[65..97].try_into()?,
        })
    }
}

/// Response bytes and the MAC-like result expected from the frame.
pub struct PairingProof {
    pub response: [u8; 65],
    pub expected_result: [u8; 32],
}

/// Compute the code-based pairing response with a caller-supplied scalar.
///
/// `shared` is the precomputed NaCl handshake key, reused by SDG as a
/// Curve25519 scalar in the device pairing exchange.
pub fn pairing_proof(
    otp: &str,
    client: Key,
    server: Key,
    shared: &[u8; 32],
    challenge: &PairingChallenge,
    random: &[u8; 32],
) -> Result<PairingProof> {
    let mut hash = Sha512::new();
    hash.update(otp.as_bytes());
    hash.update(client.as_bytes());
    hash.update(server.as_bytes());
    let h0 = hash.finalize();
    let mut hash = Sha512::new();
    hash.update(h0);
    hash.update(challenge.nonce);
    let h1 = hash.finalize();

    let mut unmasked = challenge.y;
    let nonce: [u8; 24] = challenge.nonce[..24].try_into()?;
    let mut cipher = XSalsa20::new((&h1[..32]).into(), (&nonce).into());
    cipher.apply_keystream(&mut unmasked);

    let base = PublicKey::from(&StaticSecret::from(*shared));
    let pivot = StaticSecret::from(unmasked).diffie_hellman(&base);
    ensure!(pivot.was_contributory(), "invalid pairing pivot");
    let response_x = StaticSecret::from(*random).diffie_hellman(&PublicKey::from(pivot.to_bytes()));
    ensure!(response_x.was_contributory(), "invalid pairing response point");
    let secret = StaticSecret::from(*random)
        .diffie_hellman(&PublicKey::from(challenge.x));
    ensure!(secret.was_contributory(), "invalid pairing challenge point");

    let hash_point = |point: &[u8; 32]| -> [u8; 32] {
        let inner = Sha512::digest(point);
        let mut outer = Sha512::new();
        outer.update(inner);
        outer.update(secret.as_bytes());
        let sum = outer.finalize();
        sum[..32].try_into().expect("SHA512 is at least 32 bytes")
    };
    let y = hash_point(&challenge.x);
    let expected_result = hash_point(response_x.as_bytes());
    let mut response = [0; 65];
    response[0] = RESPONSE;
    response[1..33].copy_from_slice(response_x.as_bytes());
    response[33..].copy_from_slice(&y);
    Ok(PairingProof { response, expected_result })
}

/// Finish a pairing handshake on a previously established pairing tunnel.
/// The frame's identity must be persisted by the caller ONLY after success.
pub fn run_pairing<S: std::io::Read + std::io::Write>(
    tunnel: &mut EstablishedTunnel<S>,
    identity: &Identity,
    otp: &str,
) -> Result<()> {
    ensure!(tunnel.pairing_mode, "pairing requires a pairing-mode SDG tunnel");
    ensure!(tunnel.client_identity == identity.public_key(), "pairing identity mismatch");
    ensure!(!otp.is_empty() && otp.bytes().all(|c| c.is_ascii_digit()), "invalid pairing code");
    let challenge = PairingChallenge::decode(&tunnel.receive()?)?;
    let mut random = [0u8; 32];
    OsRng.fill_bytes(&mut random);
    let proof = pairing_proof(
        otp, identity.public_key(), tunnel.server_identity(),
        &tunnel.shared, &challenge, &random,
    )?;
    tunnel.send(&proof.response)?;
    let reply = tunnel.receive()?;
    ensure!(reply.len() == 33 && reply[0] == RESULT, "invalid SDG pairing result");
    ensure!(bool::from(reply[1..].ct_eq(&proof.expected_result)), "SDG pairing rejected");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_matches_opensdg_reference_vector() {
        let challenge = PairingChallenge {
            x: hex::decode("358072d6365880d1aeea329adf9121383851ed21a28e3b75e965d0d2cd166254")
                .unwrap().try_into().unwrap(),
            nonce: core::array::from_fn(|i| 0x40 + i as u8),
            y: core::array::from_fn(|i| 0x70 + i as u8),
        };
        let client = Key::from_bytes(core::array::from_fn(|i| 1 + i as u8));
        let server = Key::from_bytes(core::array::from_fn(|i| 0xa0 + i as u8));
        let shared = core::array::from_fn(|i| 0x10 + i as u8);
        let random = core::array::from_fn(|i| 0xc0 + i as u8);
        let proof = pairing_proof("12345678", client, server, &shared, &challenge, &random).unwrap();
        assert_eq!(
            hex::encode(&proof.response[1..33]),
            "eff3f1b06110e23bc2858f1db21b409aa90866a9260cbc4fd186a4ce62cf1e5b"
        );
        assert_eq!(
            hex::encode(&proof.response[33..]),
            "6ebdb5069bb8cc08c63a7a3b75443684834276b803b131045d47637327432965"
        );
        assert_eq!(
            hex::encode(proof.expected_result),
            "c9886c250d44939dec0b21aacc133ab1b78695397b0c5f383fa7e4c055ab23f7"
        );
    }

    #[test]
    fn rejects_wrong_challenge_size_and_type() {
        assert!(PairingChallenge::decode(&[3;96]).is_err());
        assert!(PairingChallenge::decode(&[0;97]).is_err());
    }
}
