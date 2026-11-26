// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline SDG fake-server test. This is not a physical Frameo device test.

use std::os::unix::net::UnixStream;
use std::thread;

use anyhow::{Result, ensure};
use frameo_linux::transport::sdg::{
    Identity, Key, Nonce, Packet, crypto,
    handshake::{HandshakeMode, handshake},
    nonce::{
        NONCE_CLIENT_MESSAGE, NONCE_COOKIE, NONCE_HELLO, NONCE_READY, NONCE_SERVER_MESSAGE,
        NONCE_VOUCH, NONCE_VOUCHER,
    },
    read_frame, write_frame,
};

fn recv(stream: &mut UnixStream) -> Result<Packet> {
    Ok(Packet::decode_body(&read_frame(stream, 4096)?)?)
}

fn send(stream: &mut UnixStream, command: [u8; 4], payload: Vec<u8>) -> Result<()> {
    write_frame(stream, &Packet::new(command, payload).encode_body())?;
    Ok(())
}

fn fake_frame(mut stream: UnixStream, server: Identity) -> Result<()> {
    assert_eq!(recv(&mut stream)?.command, *b"TELL");
    send(
        &mut stream,
        *b"WELC",
        server.public_key().as_bytes().to_vec(),
    )?;
    let helo = recv(&mut stream)?;
    ensure!(
        helo.command == *b"HELO" && helo.payload.len() == 120,
        "bad HELO"
    );
    let client_short = Key::from_bytes(helo.payload[..32].try_into()?);
    ensure!(
        u64::from_be_bytes(helo.payload[32..40].try_into()?) == 0,
        "HELO nonce"
    );
    let hello = crypto::open(
        &helo.payload[40..],
        &Nonce::short(NONCE_HELLO, 0),
        &client_short,
        &server.private_key(),
    )?;
    ensure!(hello == [0u8; 64], "bad HELO proof");

    let short = Identity::generate();
    let cookie = [0x3cu8; 96];
    let tail = [0xabu8; 16];
    let mut cook_plain = Vec::new();
    cook_plain.extend_from_slice(short.public_key().as_bytes());
    cook_plain.extend_from_slice(&cookie);
    let mut cook = tail.to_vec();
    cook.extend(crypto::seal(
        &cook_plain,
        &Nonce::long(NONCE_COOKIE, &tail),
        &client_short,
        &server.private_key(),
    )?);
    send(&mut stream, *b"COOK", cook)?;

    let shared = crypto::precompute(&client_short, &short.private_key())?;
    let voch = recv(&mut stream)?;
    ensure!(voch.command == *b"VOCH", "bad VOCH command");
    ensure!(voch.payload[..96] == cookie, "bad VOCH cookie");
    let nonce = u64::from_be_bytes(voch.payload[96..104].try_into()?);
    ensure!(nonce == 1, "bad VOCH counter");
    let outer = crypto::open_shared(
        &voch.payload[104..],
        &Nonce::short(NONCE_VOUCH, nonce),
        &shared,
    )?;
    ensure!(outer.len() == 97 && outer[96] == 0, "bad VOCH peer body");
    let client_long = Key::from_bytes(outer[..32].try_into()?);
    let inner_tail: [u8; 16] = outer[32..48].try_into()?;
    let inner = crypto::open(
        &outer[48..96],
        &Nonce::long(NONCE_VOUCHER, &inner_tail),
        &client_long,
        &server.private_key(),
    )?;
    ensure!(
        inner == *client_short.as_bytes(),
        "VOCH identity proof mismatch"
    );
    let mut ready = 7u64.to_be_bytes().to_vec();
    ready.extend(crypto::seal_shared(
        b"certificate",
        &Nonce::short(NONCE_READY, 7),
        &shared,
    )?);
    send(&mut stream, *b"REDY", ready)?;

    let mesg = recv(&mut stream)?;
    ensure!(mesg.command == *b"MESG", "expected MESG");
    let cnt = u64::from_be_bytes(mesg.payload[..8].try_into()?);
    ensure!(cnt == 2, "wrong first client MESG nonce");
    let plain = crypto::open_shared(
        &mesg.payload[8..],
        &Nonce::short(NONCE_CLIENT_MESSAGE, cnt),
        &shared,
    )?;
    ensure!(plain == b"\x00\x04ping", "wrong client message");
    let mut answer = 0u64.to_be_bytes().to_vec();
    answer.extend(crypto::seal_shared(
        b"\x00\x04pong",
        &Nonce::short(NONCE_SERVER_MESSAGE, 0),
        &shared,
    )?);
    send(&mut stream, *b"MESG", answer.clone())?;
    // Replay the same authenticated packet; the client must reject it.
    send(&mut stream, *b"MESG", answer)?;
    Ok(())
}

#[test]
fn handshake_authenticated_exchange_and_replay_rejection() -> Result<()> {
    let (client_stream, server_stream) = UnixStream::pair()?;
    let server = Identity::generate();
    let trusted = server.public_key();
    let worker = thread::spawn(move || fake_frame(server_stream, server));
    let identity = Identity::generate();
    let mut tunnel = handshake(client_stream, &identity, &[trusted], HandshakeMode::Peer)?;
    ensure!(tunnel.server_identity() == trusted, "wrong remote key");
    tunnel.send(b"ping")?;
    ensure!(tunnel.receive()? == b"pong", "wrong peer message");
    ensure!(
        tunnel.receive().is_err(),
        "replayed authenticated packet was accepted"
    );
    worker.join().expect("fake server panicked")?;
    Ok(())
}
