//! Long-term Curve25519 identity keys used by SecureDeviceGrid.

use std::fmt;

use anyhow::{Context, Result};
use rand::rngs::OsRng;
use x25519_dalek::{PublicKey, StaticSecret};

/// The wire format uses 32-byte Curve25519 keys and lowercase hexadecimal IDs.
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct Key([u8; 32]);

impl Key {
    pub const SIZE: usize = 32;

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(value: &str) -> Result<Self> {
        let bytes = hex::decode(value).context("key is not valid hexadecimal")?;
        let bytes: [u8; 32] = bytes.try_into().map_err(|bytes: Vec<u8>| {
            anyhow::anyhow!("key has {} bytes; expected 32", bytes.len())
        })?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Key").field(&self.to_hex()).finish()
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// A persistent client identity. Its public key is the identity paired with a
/// Frameo frame; losing the private key requires pairing with every frame again.
pub struct Identity {
    private: StaticSecret,
    public: Key,
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity")
            .field("private", &"[REDACTED]")
            .field("public", &self.public)
            .finish()
    }
}

impl Identity {
    /// Generate a fresh identity using the operating system CSPRNG.
    pub fn generate() -> Self {
        let private = StaticSecret::random_from_rng(OsRng);
        Self::from_secret(private)
    }

    /// Reconstruct an identity from the 32-byte private key stored by unframeo.
    pub fn from_private_key(private: Key) -> Self {
        Self::from_secret(StaticSecret::from(*private.as_bytes()))
    }

    fn from_secret(private: StaticSecret) -> Self {
        let public = PublicKey::from(&private);
        Self {
            private,
            public: Key::from_bytes(public.to_bytes()),
        }
    }

    /// Return the private key bytes for secure persistence. Never log this.
    pub fn private_key(&self) -> Key {
        Key::from_bytes(self.private.to_bytes())
    }

    /// Return this client's public peer ID.
    pub const fn public_key(&self) -> Key {
        self.public
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_rfc_7748_alice_public_key() {
        let private =
            Key::from_hex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a")
                .unwrap();
        let identity = Identity::from_private_key(private);
        assert_eq!(
            identity.public_key().to_hex(),
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
        );
        assert_eq!(identity.private_key(), private);
    }

    #[test]
    fn key_parser_requires_exactly_32_bytes() {
        assert!(Key::from_hex("abcd").is_err());
        assert!(Key::from_hex("zz".repeat(32).as_str()).is_err());
    }

    #[test]
    fn generated_identity_has_stable_public_key() {
        let identity = Identity::generate();
        let restored = Identity::from_private_key(identity.private_key());
        assert_eq!(identity.public_key(), restored.public_key());
    }
}
