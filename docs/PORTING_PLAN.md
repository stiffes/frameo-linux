# Native Rust protocol porting plan

Status: source audit complete; Rust protocol implementation not started.

## Decision

Frameo Linux will implement the Frameo client natively in Rust. The Rust CLI and the planned GTK4 application will use one shared Rust library. The Go `unframeo` executable is a protocol reference during development only; it will not be a runtime or installation dependency.

Do not attempt to translate the whole Go command-line application in one step. Port the protocol layers behind a small, testable Rust API, then move CLI commands onto that API.

## Source audit

Reference implementation: [borje/unframeo](https://github.com/borje/unframeo), default branch `main`.

The source is divided into these useful boundaries:

| Upstream area | Responsibility | Rust destination |
| --- | --- | --- |
| `internal/config` | Long-term client identity, paired-frame peer IDs, names, default frame, atomic config persistence | `config` |
| `internal/sdg` | SecureDeviceGrid sockets, framing, CurveCP-like encrypted tunnel, pairing, grid control channel, relayed and direct peer connections | `transport::sdg` |
| `internal/sdg/control` | SecureDeviceGrid control-channel protobuf messages | `transport::sdg::control` |
| `internal/mdns` | DNS-SD browsing for `_frameo._tcp` local frame services | `discovery::mdns` |
| `internal/frameo` | Frameo application messages, message IDs, acknowledgements, multipart reassembly, media transfer and remote management | `protocol::frameo` |
| `internal/frameo/pb` | Frameo protobuf schemas and generated Go bindings | `protocol::frameo::proto` |
| `cmd/unframeo` | CLI orchestration and user-facing operations | Existing `frameo` CLI, expanded incrementally |

### Protocol facts established by the source

- A frame's long-term public Curve25519 key is its SDG peer ID. The client's private identity key must survive restarts; losing it means pairing again.
- SDG uses TCP length-prefixed frames. Tunnel packets have an 8-byte magic/command header and commands `TELL`, `WELC`, `HELO`, `COOK`, `VOCH`, `REDY` and `MESG`.
- The encrypted tunnel is a CurveCP-shaped handshake using NaCl `crypto_box` semantics, 24-byte nonces, and per-direction message counters. The implementation must reproduce the exact NaCl/libsodium wire format.
- Pairing is a separate challenge-response exchange after establishing an SDG pairing tunnel. It uses SHA-512, Salsa20 and Curve25519 operations. This is security-sensitive and must be ported against deterministic vectors.
- The grid connection is a control plane, not the photo data path. It negotiates a protocol version, maintains pings, asks the grid to connect to a peer, and receives relay endpoint/tunnel details. Relayed peer traffic uses a separate socket.
- Direct LAN connections use the same encrypted peer tunnel but include the `framedump_local` service property. The relay path requests `framedump`.
- Frames advertise DNS-SD service `_frameo._tcp`. The advertised instance is the peer ID truncated to the DNS label limit; discovery is a hint, not authentication. The SDG handshake must still verify the expected peer key.
- Frameo application messages are an 8-byte header (two big-endian 32-bit values) followed by protobuf. The transport maximum is 16,416 bytes; larger application messages use multipart reassembly.
- Photo upload sends a `Media` metadata message followed by ordered `MediaDataSegment` messages and waits for an acknowledgement. The upstream default segment payload is 16,000 bytes and its acknowledgement timeout is 120 seconds.
- The upstream also implements listing, downloads, visibility changes, deletion, permission requests, frame information, names and other management operations. These are later feature-parity work, not prerequisites for the first native upload.

## Rust crate and module design

Keep the public API independent of GTK and CLI types:

```text
src/
  lib.rs
  config/
    mod.rs
    store.rs
  discovery/
    mod.rs
    mdns.rs
  transport/
    mod.rs
    sdg/
      mod.rs
      framing.rs
      packet.rs
      nonce.rs
      crypto.rs
      handshake.rs
      pairing.rs
      grid.rs
      relay.rs
      peer.rs
      control.proto
  protocol/
    mod.rs
    frameo/
      mod.rs
      framing.rs
      multipart.rs
      client.rs
      media.rs
      messages.proto
  main.rs
```

The exact file split can change as the implementation develops, but keep these boundaries: configuration, discovery, secure transport, Frameo application protocol and user interface.

Candidate Rust dependencies to evaluate before locking versions:

- `tokio`: async TCP, cancellation and timeouts for concurrent grid/peer operations.
- `prost` and `prost-build`: protobuf messages and generated Rust bindings.
- `x25519-dalek` or an equivalent audited X25519 implementation.
- An audited NaCl-compatible `crypto_box` implementation, plus Salsa20 and SHA-2 crates where needed.
- `mdns-sd`: DNS-SD/mDNS discovery.
- `serde` and `serde_json`: configuration serialization.
- `directories`: XDG-aware configuration paths.

These are candidates, not final choices. In particular, do not assume a crate named `crypto_box` is byte-compatible: verify nonce construction, key handling, authenticator placement and ciphertext bytes against the reference vectors before using it. Do not write custom cryptographic primitives.

## Implementation phases and acceptance criteria

### 0. Preserve the reference and establish a baseline

- Pin the upstream source revision used for each porting milestone.
- Record upstream file paths and commit IDs in the project's `NOTICE`/porting notes.
- Run the upstream offline test suite and inspect its reference-vector tests.
- Keep a small set of non-secret protocol fixtures in this repository. Never commit private keys, pairing codes, real frame identities or captured user data.
- Confirm the current Rust CLI builds and its CI passes before changing the backend.

**Exit:** reproducible source baseline, documented provenance and a green Rust baseline.

### 1. Configuration and identity

- Implement secure random 32-byte identity generation and public-key derivation.
- Load/save the identity and paired-frame records using XDG paths and an explicit config override.
- Write atomically (temporary file, restrictive permissions, rename); reject malformed or truncated key material.
- Never silently generate a new identity when an existing config is missing or unreadable.
- Support multiple paired frames and a default frame.

**Exit:** deterministic serialization tests, permission tests, interrupted-write tests and key derivation vectors pass.

### 2. SDG wire primitives and cryptographic compatibility

- Implement bounded 16-bit length-prefixed TCP frames and packet parsing.
- Implement nonce construction and packet encoders/decoders.
- Implement NaCl-compatible box operations through audited dependencies.
- Reproduce HELO/COOK/VOCH/REDY and MESG packets.
- Add byte-for-byte reference tests for packet layouts, nonce values, encryption/decryption and invalid packets.
- Enforce packet-size limits before allocating memory; fail closed on authentication errors.

**Exit:** Rust produces and consumes the same deterministic packet bytes as the reference implementation. No live network code is needed yet.

### 3. Pairing

- Implement the pairing challenge parser and response calculation.
- Port the SHA-512, Salsa20 and Curve25519 calculation using fixed test inputs.
- Verify the result in constant time.
- Implement the pairing tunnel and persist the returned frame peer ID only after successful pairing.

**Exit:** all pairing vectors pass; invalid codes, malformed challenges and interrupted pairing do not modify saved configuration.

### 4. Grid control and relayed peers

- Implement the control protobuf schema and message dispatch.
- Connect to configured grid endpoints, validate server keys, negotiate protocol version and complete initial ping.
- Maintain ping/RTT and pending request IDs safely.
- Implement peer request, relay forwarding handshake and peer tunnel setup.
- Add cancellation, bounded retries, timeouts and deterministic cleanup for losing connection attempts.

**Exit:** fake-grid tests exercise connect, refusal, relay setup, ping, malformed replies, timeouts and cancellation.

### 5. Direct LAN transport and discovery

- Implement DNS-SD browsing for `_frameo._tcp`, including IPv4 and IPv6 address handling where practical.
- Match discovered instances to paired peer IDs without treating DNS names as trusted identity.
- Dial the direct service with `framedump_local`; authenticate the expected peer key.
- Add `auto`, `local` and `relay` connection selection. Auto should prefer a verified local connection and fall back to relay.

**Exit:** discovery tests pass and direct transport is verified against a real frame. A spoofed mDNS record cannot impersonate a paired frame.

### 6. Frameo application protocol and photo upload

- Port Frameo message framing and protobuf definitions.
- Implement bounded multipart reassembly, duplicate/out-of-order fragment handling and message-ID management.
- Implement the read loop, acknowledgement correlation and clean shutdown.
- Implement image upload metadata, ordered segments, caption, capture date, extension, fit/crop options and acknowledgement/error handling.
- Keep media-size limits and transfer timeouts explicit.

**Exit:** fake-frame end-to-end tests validate metadata, segment order, fragmentation, acknowledgement success/failure, timeout and cancellation. Then verify upload to a real frame over both LAN and relay.

### 7. CLI migration and feature parity

Move the CLI from the temporary process adapter to the Rust library. Keep the CLI as a thin caller of the library.

Initial native commands:
- `pair`
- `upload`
- `frames`
- `discover`

Then add:
- `info`, `list`, `get`
- `hide`, `show`, `delete`
- permission management, `name`, `forget`, `whoami`

**Exit:** no runtime call to `unframeo`, no Go installation requirement, and CLI tests cover success and failure paths.

### 8. GTK integration

The GTK4 application must call the same Rust library API as the CLI. It must not duplicate protocol logic or shell out to the CLI. Keep GTK dependencies out of the core library so headless builds and non-GTK CLI installations remain straightforward.

## Test strategy

Use three layers:

1. **Pure unit tests:** packet framing, nonce construction, crypto vectors, protobuf encoding, media chunking and configuration.
2. **In-process integration tests:** fake SDG grid, fake relay and fake Frameo peer. Keep these deterministic and offline in CI.
3. **Opt-in live tests:** explicitly enabled tests against an owned, paired Frameo device. Never run pairing, upload or destructive operations as part of ordinary CI.

A fake server derived from the same assumptions can share the same protocol mistake. Byte-for-byte reference vectors reduce that risk, but only a real frame proves interoperability. Live tests must be opt-in and must clearly identify operations that create or delete photos.

## Licensing and provenance

The current Frameo Linux repository is GPL-3.0-or-later. The upstream `unframeo` project is GPL-3.0-or-later and documents its SDG package as a port of the GPLv3 `opensdg` C library. A direct translation of that implementation remains subject to its applicable GPL obligations.

- Preserve upstream copyright and licence notices for material ported from `unframeo` and `opensdg`.
- Keep a project `NOTICE` file documenting upstream URLs, authors, licences and the portions ported.
- Keep the GPL-3.0-or-later licence in this repository and verify all added dependencies' licences.
- The opensdg README contains a non-commercial request that upstream `unframeo` describes as a request rather than a condition in its GPL licence. Preserve the attribution/context in NOTICE; do not misrepresent it as an additional licence term.
- This is an engineering checklist, not legal advice. Review the final source provenance and notices before distributing binaries.

## Immediate next implementation task

Start with Phase 0 and Phase 1, then implement the SDG packet/crypto primitives with byte-for-byte tests. Do not start the GTK application or claim native upload support until the transport and Frameo protocol work against a real frame.
