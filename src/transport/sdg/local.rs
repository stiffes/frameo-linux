// SPDX-License-Identifier: GPL-3.0-or-later
//! Native direct LAN tunnel: Frameo uses the SDG service `framedump_local`.
//! mDNS discovery of the endpoint is a separate, unfinished layer.

use std::{
    net::{SocketAddr, TcpStream},
    time::Duration,
};

use anyhow::{Context, Result, ensure};

use super::{
    Identity, Key,
    handshake::{EstablishedTunnel, HandshakeMode, handshake},
};

pub const LOCAL_SERVICE: &str = "_frameo._tcp";
pub const LOCAL_PROTOCOL: &str = "framedump_local";

/// The mDNS instance name is the frame's hex peer ID truncated to 63 chars.
pub fn local_instance(peer: Key) -> String {
    peer.to_hex()[..63].to_owned()
}

pub fn matches_instance(instance: &str, peer: Key) -> bool {
    let name = peer.to_hex();
    instance.eq_ignore_ascii_case(&name) || instance.eq_ignore_ascii_case(&name[..63])
}

/// Establish an authenticated native SDG connection to a known LAN endpoint.
///
/// The frame's long-term public key must be from prior, trusted pairing.
/// The supplied timeout applies to TCP connects and individual reads/writes;
/// it must not be zero.
pub fn dial_local(
    address: SocketAddr,
    identity: &Identity,
    paired_frame: Key,
    timeout: Duration,
) -> Result<EstablishedTunnel<TcpStream>> {
    ensure!(!timeout.is_zero(), "network timeout must be greater than zero");
    let stream = TcpStream::connect_timeout(&address, timeout)
        .with_context(|| format!("connect to Frameo at {address}"))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    handshake(
        stream,
        identity,
        &[paired_frame],
        HandshakeMode::Local { protocol: LOCAL_PROTOCOL.to_owned() },
    ).context("Frameo local SDG handshake")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_instance_matches_advertised_truncated_peer_key() {
        let peer = Key::from_bytes([0xab;32]);
        assert_eq!(local_instance(peer).len(), 63);
        assert!(matches_instance(&local_instance(peer), peer));
        assert!(matches_instance(&peer.to_hex(), peer));
        assert!(!matches_instance(&"a".repeat(63), peer));
    }
}
