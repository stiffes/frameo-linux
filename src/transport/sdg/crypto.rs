//! NaCl-compatible public-key boxes.
//!
//! The reference protocol uses crypto_box/XSalsa20-Poly1305. This wrapper uses
//! RustCrypto's audited SalsaBox implementation. Interoperability is guarded by
//! a deterministic HELO packet test against the upstream reference fixture.

use anyhow::{Context, Result};
use crypto_box::{aead::Aead, Nonce as BoxNonce, PublicKey, SalsaBox, SecretKey};

use super::{Key, Nonce};

/// Encrypt with NaCl crypto_box semantics (16-byte authenticator + ciphertext).
pub fn seal(
    plaintext: &[u8],
    nonce: &Nonce,
    recipient_public: &Key,
    sender_private: &Key,
) -> Result<Vec<u8>> {
    let cipher = SalsaBox::new(
        &PublicKey::from(*recipient_public.as_bytes()),
        &SecretKey::from(*sender_private.as_bytes()),
    );
    cipher
        .encrypt(BoxNonce::from_slice(nonce.as_bytes()), plaintext)
        .context("SDG crypto_box encryption failed")
}

/// Authenticate and decrypt a NaCl crypto_box message.
pub fn open(
    ciphertext: &[u8],
    nonce: &Nonce,
    sender_public: &Key,
    recipient_private: &Key,
) -> Result<Vec<u8>> {
    let cipher = SalsaBox::new(
        &PublicKey::from(*sender_public.as_bytes()),
        &SecretKey::from(*recipient_private.as_bytes()),
    );
    cipher
        .decrypt(BoxNonce::from_slice(nonce.as_bytes()), ciphertext)
        .context("SDG crypto_box authentication failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::sdg::Identity;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn salsa_box_matches_upstream_helo_ciphertext_fixture() {
        let mut server_private = [0u8; 32];
        let mut client_ephemeral_private = [0u8; 32];
        for i in 0..32 {
            server_private[i] = 0xa0 + i as u8;
            client_ephemeral_private[i] = 0x50 + i as u8;
        }
        let server = Identity::from_private_key(Key::from_bytes(server_private));
        let client_ephemeral =
            Identity::from_private_key(Key::from_bytes(client_ephemeral_private));
        let nonce = Nonce::short(super::super::nonce::NONCE_HELLO, 0);
        let ciphertext = seal(
            &[0u8; 64],
            &nonce,
            &server.public_key(),
            &client_ephemeral.private_key(),
        )
        .unwrap();

        // libsodium/opensdg reference output for this fixed input. The first
        // 32 bytes are the ephemeral public key in the HELO packet and are
        // asserted separately by the identity vector test.
        assert_eq!(
            hex(&ciphertext),
            concat!(
                "3b8c384e099796e1c716e8debb6a7dffd80a999048d5973f6aa2cb900b581950",
                "ae08306f0cea1f0a22ffebdcec3ee6c27676f5d579c942ea2207ca945ac5c5e7",
                "1e533cfae28d05629da0820dd34640b8"
            )
        );
        assert_eq!(
            open(
                &ciphertext,
                &nonce,
                &client_ephemeral.public_key(),
                &server.private_key(),
            )
            .unwrap(),
            vec![0; 64]
        );
    }

    #[test]
    fn authentication_failure_is_rejected() {
        let alice = Identity::generate();
        let bob = Identity::generate();
        let nonce = Nonce::short(super::super::nonce::NONCE_HELLO, 7);
        let mut ciphertext =
            seal(b"hello", &nonce, &bob.public_key(), &alice.private_key()).unwrap();
        ciphertext[0] ^= 1;
        assert!(open(&ciphertext, &nonce, &alice.public_key(), &bob.private_key()).is_err());
    }
}
