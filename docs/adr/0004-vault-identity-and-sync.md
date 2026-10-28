# ADR 0004 — Vault identity, device enrollment and the encrypted sync protocol

Status: proposed by B021 on 28 October 2026 for the owner's review. The Rust crate `crates/sync`, the JSON Schemas `sync-operation`, `device-enrollment` and `recovery-export`, the examples under `examples/` and the conformance fixture `sync-operation-log-merge` implement it. Builds on ADR 0001 (one Rust core), ADR 0003 (schema versioning) and specs/05-sharing-security.md "Identity and encrypted notes"; the operation-log rules are those of specs/03-data-model.md "Notes and synchronization".

## Context

Private notes, highlights, tags, workspaces and ink move between a reader's devices through an optional self-hosted hub, a direct device-to-device exchange or a file. specs/05 fixes the requirements: a vault identity independent of any login, `(issuer, subject)` as the remote principal linked by a user-approved enrollment, a vault key wrapped to approved device keys and to a user-held recovery secret, enrollment confirmed by an existing device or by recovery material, revocation with key rotation and the honest statement that obtained plaintext cannot be recalled, no server recovery when all keys are lost, an append-only operation log with globally unique ids, causal parents and per-device sequence numbers, sibling revisions for concurrent bodies, deterministic merges, tombstone compaction with acknowledgement, and a reviewed, maintained cryptographic library rather than new cryptography.

This ADR chooses the library, fixes the constructions composed from it, and specifies the enrollment, revocation, recovery and causal-conflict contract. It designs no primitive and no protocol beyond composing documented constructions; the review below says where each one comes from.

## Decision 1: the cryptographic library

### Candidates reviewed

Facts marked *verified* were read from the crate's own repository, README or crates.io record on 28 October 2026; everything else is marked *unverified* or *estimated*.

| Candidate | License | Maintainers | Last release (crates.io) | Audit history | wasm32-unknown-unknown | Pure Rust | Size estimate |
|---|---|---|---|---|---|---|---|
| RustCrypto AEADs: `chacha20poly1305` | Apache-2.0 OR MIT (verified) | RustCrypto organization (verified) | 0.11.0, 28 June 2026 (verified) | One security audit by NCC Group, February 2020, "with no significant findings", funded by MobileCoin; the crate's README links the public report (verified from the README of 0.11.0; the report itself was not re-read) | Compiles in `just check-wasm` (verified here) | Yes (verified: no build script, no C) | ~10–20 KB (estimated) |
| RustCrypto `hkdf`, `sha2`, `argon2`, `zeroize`, `subtle` | MIT OR Apache-2.0; `subtle` BSD-3-Clause (verified) | RustCrypto; `subtle` by dalek-cryptography (verified) | hkdf 0.13.0, 30 March 2026; sha2 0.11.1, 12 October 2026; argon2 0.6.0, 27 August 2026; zeroize 1.9.1; subtle 2.6.1 (verified) | No audit statement in these crates' READMEs (unverified) | Compiles (verified) | Yes (verified) | sha2 ~8 KB, argon2 ~10 KB, hkdf/hmac ~3 KB (estimated) |
| dalek: `x25519-dalek`, `ed25519-dalek`, `curve25519-dalek` | BSD-3-Clause (verified) | dalek-cryptography organization (verified) | x25519-dalek 3.0.0 and ed25519-dalek 3.0.0, 6 July 2026; curve25519-dalek 5.0.0 (verified) | No audit statement in the repository README (unverified); the README states "pure-Rust implementation" (verified); an optional formally verified field-arithmetic backend (`fiat-crypto`) exists (verified in the dependency tree) | Compiles with the portable u32/u64 backends (verified) | Yes (verified) | 40–80 KB without precomputed tables (estimated) |
| `ring` | Apache-2.0 AND ISC, with BoringSSL and "other bits" licenses (verified) | Brian Smith (verified) | 0.17.14, 11 March 2025 (verified) | No audit statement; README says most of its C and assembly comes from BoringSSL (verified) | Supported for a subset of algorithms with a JS-backed random source; not tested here (unverified) | No: C and assembly per target, needs a C toolchain for every cross target (verified) | 200–400 KB (estimated) |
| `age` / `rage` | MIT OR Apache-2.0 (verified) | str4d; format designed by Filippo Valsorda and Ben Cartwright-Cox (verified) | 0.12.1, 14 July 2026 (verified) | No audit statement in the README (unverified) | Not stated in the README (unverified) | Built on the RustCrypto and dalek crates above (verified from its dependency list) | 150–250 KB with armor, scrypt and stream support (estimated) |
| OpenMLS | MIT (verified) | Phoenix R&D and CE Labs (verified) | 0.9.2; 0.8.3 patch on 22 October 2026 (verified) | No audit statement in the README (unverified) | README: CI "builds (but doesn't test)" wasm32-unknown-unknown with a `js` feature (verified) | Two crypto providers: RustCrypto or libcrux, both Rust (verified) | > 500 KB with HPKE, TLS codec and tree state (estimated) |
| `snow` (Noise) | Apache-2.0 OR MIT (verified) | mcginty (verified) | 0.10.0, 19 July 2025 (verified) | README: "This library has not received any formal audit" (verified) | Not stated (unverified) | Default resolver is pure Rust over the dalek and RustCrypto crates; `ring` optional (verified) | 60–120 KB (estimated) |

### Choice

**The RustCrypto and dalek crates**, pinned exactly in the workspace `Cargo.toml` and used only by `crates/sync` (the core, text, collections and conformance crates stay free of them):

| Crate | Version | Features | Role |
|---|---|---|---|
| `x25519-dalek` | 3.0.0 | `static_secrets`, `zeroize` | Key agreement for wrapping the vault key |
| `ed25519-dalek` | 3.0.0 | `zeroize` | Operation, approval and revocation signatures |
| `chacha20poly1305` | 0.11.0 | `alloc` | XChaCha20-Poly1305 for wraps and payloads |
| `hkdf` | 0.13.0 | none | HKDF-SHA256 for the wrap key |
| `sha2` | 0.11.1 | none | SHA-256 for ids and payload hashes; HKDF's hash |
| `argon2` | 0.6.0 | `alloc` | Argon2id for the passphrase of the recovery export |
| `rand_core` | 0.10.1 | none | The `CryptoRng` trait the host's random source implements |
| `zeroize` | 1.9.1 | `alloc` | Secret memory cleared on drop |
| `subtle` | 2.6.1 | none | Constant-time key comparison |

Default features are off everywhere so that no crate links `getrandom`: on wasm32-unknown-unknown that crate needs a backend configuration flag, and the core's rule is that the host owns platform services. The host passes a `rand_core::CryptoRng` (the OS random source on Apple and Android, `crypto.getRandomValues` through the web host) to every function that needs randomness; the tests use a seeded ChaCha20 generator (`rand_chacha` 0.10.0, dev-dependency). `proptest` 1.12.0 is a dev-dependency for the property tests.

Why this set: pure Rust on every target the project builds (aarch64 Apple and Android, x86_64 Linux, wasm32) with no C toolchain in the loop; the smallest footprint that covers key agreement, AEAD, KDF, signatures and a password hash; one audited AEAD; the same primitives age, Signal's libraries and WireGuard rely on, so the constructions below have published precedents; active releases in 2026 under organizations rather than one person; and the dependency tree stays inspectable (the sync crate's third-party tree is the crates above plus `curve25519-dalek`, `digest`, `hmac`, `poly1305`, `chacha20`, `cipher`, `aead`, `crypto-common`, `hybrid-array`, `blake2` for argon2, and small utilities).

### Rejected

| Option | Why not |
|---|---|
| `ring` | Not pure Rust: BoringSSL C and assembly compiled per target, which complicates the Apple, Android and wasm cross builds the project already has to keep working; last release March 2025; API offers one-shot X25519 only (static keys and Argon2id would need another crate anyway); a single maintainer. Its BoringSSL lineage is a real strength, not enough to carry the toolchain cost. |
| `age` / `rage` | A file-encryption format: recipients, armor, scrypt, streaming. Its **X25519 recipient construction is exactly what the vault-key wrap needs, and this ADR adopts that construction** (below) using the same underlying crates, without taking on the file format, its extra dependencies or a format we would otherwise never produce. Remains the natural candidate if the recovery export ever becomes an age file. |
| OpenMLS | MLS solves group key agreement with forward secrecy and post-compromise security among many members; a personal vault has one user and a handful of devices with a static key per epoch. The tree state, HPKE and TLS codec would dominate the binary and the review surface, and wasm32 is built but not tested upstream. Revisit only if shared vaults between people become a requirement. |
| `snow` (Noise) | A handshake protocol for live sessions between two peers. The right tool for the LAN pairing channel of specs/05 "Package exchange protocol v1" (a later task), the wrong layer for at-rest key wrapping and a signed log that a hub stores. Unaudited by its own statement. |

## Decision 2: constructions, each from the library's own documentation

No cipher, hash, key derivation, signature or random generator is implemented in this repository. `crates/sync/src/envelope.rs` composes the following and nothing else; its tests run the libraries against their published vectors (RFC 7748 §6.1 for X25519, RFC 8032 §7.1 test 1 for Ed25519, RFC 5869 appendix A case 1 for HKDF-SHA256, draft-irtf-cfrg-xchacha appendix A for XChaCha20-Poly1305, the Argon2 reference test suite for Argon2id) and round-trip the layer over them.

| Purpose | Construction | Source |
|---|---|---|
| Wrap the vault key to a device or to the recovery identity | Ephemeral X25519 key against the recipient's static X25519 key; shared secret rejected if not contributory (`SharedSecret::was_contributory`); HKDF-SHA256 with salt = ephemeral public ‖ recipient public and info `openlogos-sync/v1/x25519-wrap` expanded to 32 bytes; XChaCha20-Poly1305 with a fresh random 24-byte nonce and associated data = canonical (vault id, key epoch, recipient label, recipient public key). Ciphertext is 48 bytes. | The X25519 recipient stanza of the age format (age-encryption.org/v1: "the key is derived with HKDF-SHA-256 with salt = ephemeral share ‖ recipient public key") with a random nonce added because XChaCha's 192-bit nonce makes that safe; `x25519-dalek` docs for `EphemeralSecret::diffie_hellman` and `SharedSecret::was_contributory`; `hkdf` docs (RFC 5869); `chacha20poly1305` docs and the XChaCha draft |
| Seal an operation payload | XChaCha20-Poly1305 under the vault key of the operation's epoch, random nonce, associated data = canonical (vault id, device id, device sequence) | `chacha20poly1305` docs ("ChaCha20Poly1305 variant with an extended 192-bit (24-byte) nonce"; a nonce "MUST be unique per message"); draft-irtf-cfrg-xchacha §1, which exists so that nonces can be random: 192 bits make collisions negligible |
| Hash | SHA-256 over canonical bytes for operation ids; SHA-256 over nonce ‖ ciphertext for payload hashes (never over plaintext, so equal notes do not produce equal hashes) | `sha2` docs (FIPS 180-4) |
| Sign | Ed25519 over the canonical signing input; verification with `verify_strict` | `ed25519-dalek` docs for `VerifyingKey::verify_strict` ("performs *both* of the above signature malleability checks": scalar and group-element malleability) |
| Protect the recovery secret with a passphrase | Argon2id version 0x13 with the crate's default parameters (`Params::DEFAULT_M_COST` = 19 456 KiB, `DEFAULT_T_COST` = 2, `DEFAULT_P_COST` = 1) and a random 16-byte salt, recorded beside the ciphertext; the 32-byte output keys XChaCha20-Poly1305 with associated data = canonical (vault id) | `argon2` docs for `Argon2::hash_password_into` and `Params::DEFAULT` (RFC 9106) |
| Randomness | Supplied by the host as `rand_core::CryptoRng` | `rand_core` 0.10 docs |
| Secret memory | `Zeroizing` wrappers, `zeroize` feature of the dalek crates, no `Serialize` and no `Debug` output of secret bytes | `zeroize` docs |

**Canonical bytes** are an encoding, not a cryptographic construction: a domain string (`openlogos-sync-op/1`, `openlogos-sync-wrap/1`, `openlogos-sync-payload/1`, `openlogos-sync-passphrase/1`, `openlogos-sync-approval/1`, `openlogos-sync-revocation/1`) followed by 4-byte big-endian length-prefixed fields (`crates/sync/src/canonical.rs`). JSON is never signed directly. The `/1` suffix is the version of each input layout; changing a layout is a new suffix and a MAJOR bump of the record that carries it.

## Decision 3: identities and keys

1. **Vault id** `vault-<32 lower-case hex>`: 128 random bits from the host, generated locally when the user first creates private data. It is the identity of the user's private state; it never derives from a login, an email, a device or a server.
2. **Remote principal** `(issuer, subject)` of an OIDC identity is linked to a vault by `Roster::link_principal` on an active device after the user confirms; the roster records issuer, subject, time and device. A changed issuer, or the same email under another issuer, is a new principal and is never matched automatically. A vault may exist with no principal forever.
3. **Device keys**: each device generates two independent keys from the host's random source and keeps them in OS secure storage (`DeviceSecrets::to_bytes`): an Ed25519 signing key and an X25519 exchange key. Signing and encryption keys are distinct, as specs/05 requires. **Device id** `dev-<32 hex>` is the first 16 bytes of the SHA-256 of the signing public key, so a device id cannot be claimed without the key.
4. **Vault key**: a 256-bit XChaCha20-Poly1305 key per **key epoch**. Epoch 1 is minted at founding; every revocation mints the next. The key exists in the clear only in memory; the roster holds it wrapped to each active device for every epoch the device was active in, and to the recovery identity for every epoch.
5. **Recovery identity**: an X25519 key pair. The private scalar is the user-held recovery secret, shown once as a 64-hex-digit code in groups of four (`RecoverySecret::display_code`) and optionally kept as a passphrase-protected recovery export; the public key sits in the roster so that any device can rewrap a rotated vault key to it without holding the secret. This is the age identity model and it is what makes rotation after a lost device possible without the user re-entering the recovery code.

## Decision 4: enrollment, revocation and recovery (the roster state machine)

`crates/sync/src/enrollment.rs`; record shape `schemas/device-enrollment.schema.json`.

1. **Founding**: the first device mints the vault key, wraps it to itself and to the recovery identity, and records itself with a self-signed `founder` approval. Exactly one founder exists per roster.
2. **Request**: a new device produces an `EnrollmentRequest` (vault id, its two public keys, a display name). It holds nothing it can read with: no wrap exists for it, the log rejects its operations as `UnknownDevice`, and a guessed key opens no payload (tested).
3. **Approval by a device**: an *active* device unwraps the current vault key with its own exchange key, wraps it to the requester's exchange key and signs the approval input (vault, device id, both public keys, epoch, approver). The roster records approver and signature. A revoked, unknown or already-enrolled device cannot approve.
4. **Approval by recovery material**: the user presents the recovery secret (code or export plus passphrase); the vault key is unwrapped from the recovery wrap of the current epoch and wrapped to the requester; the approval is recorded as `recovery`. The recovery identity cannot sign (X25519), so the proof is the wrap itself, which only the holder of the secret could produce.
5. **Revocation**: an active device other than the revoked one mints the next epoch's key, wraps it to every remaining active device and to the recovery identity, signs a revocation record (vault, revoked device, revoker, reason, cutoff sequence, new epoch) and marks the device. A revoked device never becomes active again; a returning device enrolls with new keys as a new device. A device cannot revoke itself. The **cutoff sequence** is the revoked device's highest sequence number the revoker accepts: operations up to it stay valid wherever they arrive later, operations past it and any operation at the new epoch are rejected (`Authority::authorizes`).
6. **The documented limit**: a revoked device keeps the vault keys of the epochs it was active in and can read every payload it already synced, forever. Rotation protects what is written after revocation and removes the device's power to write; it recalls nothing. The specs/05 wording stands: previously obtained plaintext and keys cannot be recalled.
7. **Lost all keys**: with the recovery code, the current roster (from a hub, another device's export or a recovery export) suffices to enroll a fresh device (item 4). With the recovery export and its passphrase, nothing else is needed: the package carries the roster and the recovery wrap of its epoch (`RecoveryExport::open`). Without recovery material there is no recovery: the hub holds only ciphertext and the roster, and this ADR gives it no way to mint access. Tested in `recovery.rs` and `enrollment.rs`.
8. **Roster verification**: `Roster::verify` checks the single founder, every approval signature by a device enrolled no later than the approval epoch, every revocation signature, device ids bound to keys, and one wrap per epoch from enrollment to revocation (or now) for each device and for the recovery identity. Readers run it before trusting a roster.

## Decision 5: the operation log contract

`crates/sync/src/log.rs`; record shape `schemas/sync-operation.schema.json`.

1. **Operation** = signed header: `op_id`, vault id, device id, device sequence, sorted distinct parents, key epoch, a `change`, an optional payload hash, and the device's Ed25519 signature over the canonical header. **Globally unique id**: `op-<SHA-256 of the signing input>`; two different headers cannot share an id and an id commits to the whole header. The header carries no wall-clock time: order is causal, and timestamps would be metadata the hub does not need.
2. **Causal parents**: the heads of the writer's log at writing time. **Per-device sequence numbers** start at 1 without gaps; the device's previous operation is an implicit parent.
3. **Changes** (`note_body`, `highlight_add`, `highlight_remove`, `tag_add`, `tag_remove`, `tombstone`, `tombstone_ack`) carry opaque stable ids only. Bodies, anchors, styles and tag names travel in the sealed payload. Entity ids and change kinds are visible to whoever stores the log: see "What the hub sees".
4. **Acceptance** (`OpLog::apply`): wrong vault, id not matching the header, sequence 0, self-parent, unknown device, bad signature or an unauthorized `(device, sequence, epoch)` is **rejected** and leaves the log unchanged. An id already applied or pending is a **replay**: reported as `Duplicate`, ignored. A second operation claiming a used `(device, sequence)` slot is **equivocation**: rejected. An operation whose device predecessor or parents (or acknowledged tombstone) are missing is **pending** until they arrive, then released in dependency order; delayed and out-of-order delivery therefore converge, and nothing is applied before its causes.
5. **Merge rules** (`OpLog::materialize`, a function of the applied set only):
   - Note bodies: the heads of a note are its body operations not causally dominated by another body of the same note. One head is the current body; several are **sibling revisions** for the resolution UI; a body written in knowledge of all heads resolves them. No last-writer-wins, no silent loss.
   - Highlights and tags: observed-remove sets. A removal cancels exactly the additions in its causal past; an addition concurrent with a removal survives.
   - Tombstones: a tombstone wins over every concurrent and later change to its entity, forever; the entity leaves every view.
6. **Tombstone compaction** (`OpLog::compact`): the history of a tombstoned entity is dropped only after **every active device** has written a `tombstone_ack` (the tombstone's writer counts as acknowledging). Stubs keep each dropped operation's id, device, sequence and parents so replay detection, sequence chains and causal ancestry are unchanged; the entity id stays in the log's compacted set. A later operation for a compacted entity is accepted into the sequence chain as a stub and changes nothing: **a rejoining stale device cannot resurrect a deleted note**. A revoked device blocks nothing, because its operations past the cutoff are rejected and those before it must already be present for compaction's acknowledgements to have been applied. A device that stays silent for a long time does block compaction; whether to revoke such devices after a retention period is a product decision, listed below.
7. **Property tests** (48 cases each): any permutation of any generated multi-device history, with replays and with a delayed second half, produces the same applied set, the same state and nothing pending; tombstones are final under every order; compaction preserves the state and later replays change nothing. The conformance fixture `sync-operation-log-merge` fixes one concrete history (three devices, one revoked, two sibling bodies, a tombstone and its acknowledgements) with four delivery orders and the expected state, for every host that later exposes the log.

## Decision 6: what a hub stores and sees

A hub (specs/02 "personal hub") stores the roster, operation headers and sealed payloads per vault, keyed by the linked principal. It can verify signatures and the roster, refuse replays and equivocation, and serve operations to devices. It cannot read a payload, mint access, or approve a device. It sees: the vault id, the device ids and their count, the linked principal, operation ids, parents, sequence numbers and key epochs, change kinds, entity and tag ids, ciphertext sizes, and connection timing. That is the metadata specs/05 "Threat boundaries" lists plus change kinds and entity ids, which this ADR keeps in the clear so the hub and a thin client can compact and index without keys. Sealing them too is a MINOR-compatible addition (a `sealed_change` kind) if the owner wants the stronger property at the cost of hub-side compaction.

Direct device-to-device exchange and file transfer carry the same three records. This crate contains no network code and no server.

## Versioning

`sync-operation`, `device-enrollment` and `recovery-export` are top-level documents at `schema_version` 1.0 under the ADR 0003 rules. Adding a change kind, an approval kind or a recipient kind is a MINOR bump (a reader skips what it does not know; for a change kind it must then keep the operation as an opaque stub so sequence chains hold). Changing a canonical input layout, the id derivation, the wrap construction or the AEAD is MAJOR, with a new domain suffix. Key epochs are per vault and never reset.

## Consequences

- `crates/sync` compiles for wasm32-unknown-unknown and is on both wasm check lists; the web host must pass a `CryptoRng` backed by `crypto.getRandomValues`. The binary-size impact on the wasm module is not measured yet (the ffi crate does not link `sync` until a later task exposes it); the estimate for the whole set after `wasm-opt -Oz` is 100–200 KB.
- The ffi surface is unchanged. Exposing the roster and the log to Swift, Kotlin and the web is a later task; the conformance fixture is in the shared folder but `run_all` does not run it, because that would link the sync crate and its crypto dependencies into every host binary before that decision is made.
- A note's "modified" time for the UI must come from the sealed payload, not the header.
- Hub equivocation (serving different rosters to different devices, or withholding a revocation) is not prevented by a roster document with signed entries. The mitigation is to carry roster changes as signed operations in the log itself so every device sees one causal history; this is the first follow-up if the hub ships.

## Decisions for the owner's review

1. Change kinds and entity ids in the clear (hub-side compaction possible) versus sealed (less metadata).
2. Roster as a verified document (this ADR) now, roster changes as log operations later, or the latter from the start.
3. Recovery code as 64 hex digits in groups of four versus a word list (BIP39-style) for easier transcription.
4. Argon2id default cost (19 MiB, t = 2) on the web and on older phones; a measured budget may lower `m_cost` with the parameters still recorded per export.
5. Compaction requires every active device to acknowledge; a retention policy that revokes devices silent for N days is a product decision.
6. Rotation happens only on revocation; periodic rotation would bound what a silently compromised key exposes, at the cost of more wraps.
7. Hex encoding of ciphertext inside JSON doubles payload size; a binary carrier for payloads (the hub's storage, the file transfer) is open.

## Alternatives considered for the design

| Option | Why not chosen |
|---|---|
| Random UUID operation ids | Content-addressed ids make replay and tampering detection a hash comparison and need no coordination; a signed header's hash is already unique. |
| Last-writer-wins for note bodies | Forbidden by specs/03; sibling revisions keep both texts. |
| A CRDT text type | specs/03: justified only if collaborative editing becomes a requirement. |
| Recovery secret as a symmetric key (HKDF to a wrap key) | Rotation would then need the recovery secret present on the rotating device; an X25519 recovery identity lets any device rewrap to it. |
| Password-derived vault key | A vault key must be random and rotatable; passphrases protect only the recovery export. |
| Per-operation ratcheting keys (Signal-style) | Forward secrecy per message is not a stated requirement for a personal vault and would make offline sibling resolution and late delivery far harder. |
