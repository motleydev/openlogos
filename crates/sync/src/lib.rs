//! Private sync as an encrypted operation log (docs/adr/0004-vault-identity-and-sync.md,
//! specs/05-sharing-security.md "Identity and encrypted notes", specs/03-data-model.md
//! "Notes and synchronization").
//!
//! Every data change is an [`log::Operation`]: signed by the device that made it, numbered
//! per device, linked to its causal parents and carrying only the hash of a sealed payload.
//! [`log::OpLog`] orders, merges and resolves conflicts between the logs of several devices
//! and converges whatever order, duplication or delay the transport introduces. A vault is a
//! locally generated identity ([`keys::VaultId`]); its symmetric [`keys::VaultKey`] is wrapped
//! to every approved device key and to a user-held recovery secret ([`envelope`]), and the
//! [`enrollment::Roster`] says which devices may read and write at which key epoch. An
//! optional self-hosted hub stores operations, sealed payloads and the roster without being
//! able to read a note; direct device-to-device exchange and file transfer carry the same
//! records. This crate holds no network code and no server.
//!
//! No cryptographic primitive is designed or implemented here. Every construction is a
//! documented composition of the pinned libraries named in ADR 0004: `x25519-dalek`,
//! `ed25519-dalek`, `chacha20poly1305`, `hkdf` + `sha2` and `argon2`. The crate never asks the
//! operating system for randomness: the host passes a [`rand_core::CryptoRng`].

pub mod canonical;
pub mod enrollment;
pub mod envelope;
pub mod keys;
pub mod log;
pub mod recovery;

pub use rand_core::CryptoRng;

/// `schema_version` written into every `sync-operation` record (schemas/sync-operation.schema.json).
pub const SYNC_OPERATION_SCHEMA: &str = "1.0";
/// `schema_version` of the device roster (schemas/device-enrollment.schema.json).
pub const DEVICE_ENROLLMENT_SCHEMA: &str = "1.0";
/// `schema_version` of the offline recovery package (schemas/recovery-export.schema.json).
pub const RECOVERY_EXPORT_SCHEMA: &str = "1.0";

/// The crate's package name, as Cargo records it.
#[must_use]
pub fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_is_part_of_the_openlogos_workspace() {
        assert!(crate_name().starts_with("openlogos-"));
    }
}
