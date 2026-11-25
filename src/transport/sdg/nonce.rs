//! Nonce layouts used by the CurveCP-shaped SDG tunnel.

/// A NaCl crypto_box nonce (24 bytes).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Nonce([u8; 24]);

impl Nonce {
    /// Build a nonce from a 16-byte protocol label and a big-endian counter.
    pub fn short(prefix: &[u8; 16], counter: u64) -> Self {
        let mut bytes = [0; 24];
        bytes[..16].copy_from_slice(prefix);
        bytes[16..].copy_from_slice(&counter.to_be_bytes());
        Self(bytes)
    }

    /// Build a nonce from an 8-byte protocol label and a 16-byte random tail.
    pub fn long(prefix: &[u8; 8], tail: &[u8; 16]) -> Self {
        let mut bytes = [0; 24];
        bytes[..8].copy_from_slice(prefix);
        bytes[8..].copy_from_slice(tail);
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 24] {
        &self.0
    }
}

pub const NONCE_HELLO: &[u8; 16] = b"CurveCP-client-H";
pub const NONCE_VOUCH: &[u8; 16] = b"CurveCP-client-I";
pub const NONCE_CLIENT_MESSAGE: &[u8; 16] = b"CurveCP-client-M";
pub const NONCE_SERVER_MESSAGE: &[u8; 16] = b"CurveCP-server-M";
pub const NONCE_READY: &[u8; 16] = b"CurveCP-server-R";
pub const NONCE_COOKIE: &[u8; 8] = b"CurveCPK";
pub const NONCE_VOUCHER: &[u8; 8] = b"CurveCPV";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_nonce_uses_big_endian_counter() {
        let nonce = Nonce::short(NONCE_HELLO, 1);
        assert_eq!(&nonce.as_bytes()[..16], b"CurveCP-client-H");
        assert_eq!(&nonce.as_bytes()[16..], &[0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn long_nonce_copies_the_entire_tail() {
        let tail = core::array::from_fn(|i| i as u8);
        let nonce = Nonce::long(NONCE_COOKIE, &tail);
        assert_eq!(&nonce.as_bytes()[..8], b"CurveCPK");
        assert_eq!(&nonce.as_bytes()[8..], &tail);
    }
}
