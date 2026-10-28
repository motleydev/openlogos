//! First-class study notes and their relationship to an external vault. This crate decides
//! which copy of a note is authoritative when the app and an Obsidian vault both hold it, owns
//! the merge contracts that protect external writing, and keeps note anchors stable through
//! the core's reanchoring rules (specs/12-workspace-ux.md, specs/13-obsidian-and-ink.md).
//! Broader bidirectional vault editing is gated separately by B045 and B060.
//!
//! B040 fixes the contracts: [`NoteBinding`] (the portable record, JSON form
//! `schemas/note-binding.schema.json`), the authority state machine [`apply`] over
//! [`BindingEvent`]s with one [`Effect`] per transition, and the [`credentials`] check that
//! keeps credential-shaped fields out of portable state. No file I/O, plugin code or sync
//! transport lives here: a host performs the effect and feeds the result back as an event.

pub mod binding;
pub mod credentials;

pub use binding::{
    Authority, BindingError, BindingEvent, ConflictCause, Effect, NOTE_ID_PATTERN, NoteBinding,
    OperationKind, OperationRecord, Resolution, SCHEMA_VERSION, SyncState, Transition, apply,
    is_note_id, is_relative_vault_path, is_sha256_hex,
};
pub use credentials::{
    find_credential_shaped, is_credential_shaped_key, is_credential_shaped_value,
};

/// The crate's package name, as Cargo records it.
#[must_use]
pub fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod binding_tests;

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_is_part_of_the_openlogos_workspace() {
        assert!(crate_name().starts_with("openlogos-"));
    }
}
