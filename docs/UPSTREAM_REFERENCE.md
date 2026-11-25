# Upstream protocol reference and baseline

This project uses the public `unframeo` implementation as a behavioral and interoperability reference while implementing an independent Rust client.

## Pinned reference

- Project: [borje/unframeo](https://github.com/borje/unframeo)
- Audited revision: `149f60b5ac7702f2cbb0761185b0d49a7431bf3f`
- Revision URL: https://github.com/borje/unframeo/commit/149f60b5ac7702f2cbb0761185b0d49a7431bf3f
- License: GPL-3.0-or-later
- Key reference areas:
  - `internal/config/config.go`: persisted identity and paired-frame schema
  - `internal/sdg/keys.go`: Curve25519 identity and peer IDs
  - `internal/sdg/packet.go`: TCP framing and SDG packet header
  - `internal/sdg/nonce.go`: protocol nonce layouts
  - `internal/sdg/crypto.go`: NaCl crypto_box behavior
  - `internal/sdg/handshake.go`: HELO/COOK/VOCH/REDY packet construction
  - `internal/sdg/reference_test.go`: byte-for-byte reference fixtures
  - `internal/sdg/packet_test.go` and `internal/sdg/nonce_test.go`: wire primitive tests
  - `internal/sdg/sdgtest`: offline fake grid and peer
  - `internal/frameo/frameotest`: offline fake Frameo frame

The reference repository describes its SDG package as a Go port of the GPLv3 opensdg implementation. The upstream README says the protocol is undocumented and was reconstructed through reverse engineering. A local fake server is useful for deterministic tests, but upstream explicitly notes that a fake built from the same protocol assumptions cannot prove real-device interoperability.

## Baseline status

- Frameo Linux CI was green on commit `16787fad21a99280796bf9f1b806453c4af91635` before this implementation began.
- The upstream source contains offline test suites under `internal/sdg` and `internal/frameo`.
- The upstream Go test suite has not been run as part of this change; no claim is made that it was executed here.
- The Rust implementation will carry independent tests for packet framing, key derivation, nonce construction and cryptographic wire compatibility.

## Compatibility and migration

The Rust config reader deliberately preserves the existing `unframeo/config.json` location and JSON field names. This allows an existing private identity and paired-frame records to remain usable when the native transport is ready. Loading a missing file never generates a replacement identity. New identity creation is an explicit operation reserved for the pairing flow.

Do not copy real user configuration files, private keys, pairing codes, peer IDs or packet captures into this repository.
