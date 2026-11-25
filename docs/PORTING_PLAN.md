# Native Rust protocol porting plan

Status: Phase 0 complete; Phase 1 implemented; initial Phase 2 wire primitives and NaCl box compatibility tests added. Native network transport and Frameo photo transfer are not implemented.

## Decision

Frameo Linux will implement the Frameo client natively in Rust. The Rust CLI and the planned GTK4 application will use one shared Rust library. The Go `unframeo` executable is a protocol reference during development only; it will not be a runtime or installation dependency.

Do not attempt to translate the whole Go command-line application in one step. Port the protocol layers behind a small, testable Rust API, then move CLI commands onto that API.

## Source audit

Reference implementation: [borje/unframeo](https://github.com/borje/unframeo), pinned revision [149f60b5ac7702f2cbb0761185b0d49a7431bf3f](https://github.com/borje/unframeo/commit/149f60b5ac7702f2cbb0761185b0d49a7431bf3f).

See [UPSTREAM_REFERENCE.md](UPSTREAM_REFERENCE.md) and [NOTICE](../NOTICE) for provenance, source paths and baseline status.

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

## Implemented work

### Phase 0 -- reference and baseline

- Pinned upstream unframeo commit `149f60b5ac7702f2cbb0761185b0d49a7431bf3f`.
- Added `docs/UPSTREAM_REFERENCE.md` and `NOTICE` with provenance and GPL attribution.
- Confirmed the Frameo Linux baseline CI run passed before the implementation commit.
- Inspected the upstream offline fake-grid, fake-peer and fake-frame tests. The upstream Go suite has not been run as part of this change.

### Phase 1 -- configuration and identity

- Added Curve25519 identity generation and public-key derivation using `x25519-dalek`.
- Preserved the upstream `unframeo/config.json` path and JSON field names so existing identity and paired-frame data can be read.
- Added explicit config path overrides `UNFRAMEO_CONFIG` and `FRAMEO_CONFIG`.
- Added atomic config writes and owner-only (0600) config files on Unix.
- Loading a missing config never generates a replacement identity; creation is explicit and intended only for pairing.
- Added paired-frame validation, default-frame selection, rename-by-peer behavior and removal.
- Added tests for RFC 7748 X25519 public-key derivation, identity reload, JSON compatibility, invalid keys, duplicate peer records and file permissions.

### Initial Phase 2 -- SDG wire primitives

- Added bounded 16-bit TCP frame encoding/decoding and SDG packet header parsing.
- Added exact 24-byte nonce constructors for counter-based and random-tail nonce formats.
- Added RustCrypto `SalsaBox` wrappers for NaCl-compatible `crypto_box`.
- Added a deterministic HELO ciphertext compatibility test against the upstream reference fixture.
- Added native NaCl session-key precomputation, secretbox operations and encrypted MESG encoding/decoding with pinned reference vectors. VOCH, the handshake state machine and pairing remain unimplemented.

## Remaining implementation phases

### 2. Complete SDG cryptographic and packet compatibility

- Reproduce NaCl `crypto_box_beforenm` byte-for-byte. The pairing protocol reuses this output as a Curve25519 scalar, so merely encrypting/decrypting with `SalsaBox` is insufficient.
- Implement symmetric `crypto_secretbox` operations for precomputed session keys.
- Reproduce HELO/COOK/VOCH/REDY and MESG packets and test all fixed reference vectors.
- Add strict bounds and malformed-packet tests.

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
- Keep the project `NOTICE` file documenting upstream URLs, authors, licences and the portions ported.
- Keep the GPL-3.0-or-later licence in this repository and verify all added dependencies' licences.
- The opensdg README contains a non-commercial request that upstream `unframeo` describes as a request rather than a condition in its GPL licence. Preserve the attribution/context in NOTICE; do not misrepresent it as an additional licence term.
- This is an engineering checklist, not legal advice. Review the final source provenance and notices before distributing binaries.
