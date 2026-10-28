//! `NoteBinding`: which copy of a note is the body authority, the app's or a vault file's,
//! and the pure state machine that moves a binding between app-owned, vault-owned,
//! migrating, conflict and detached (specs/13-obsidian-and-ink.md, "Ownership and scope" and
//! "Direct vault integration"; specs/03-data-model.md, `NoteBinding`).
//!
//! The record is portable state: a stable note id, the authority, an opaque vault id, a
//! relative file path that is **not** identity (rename events update it without making a
//! second note), the hash of the vault file as the app last saw it, the hash of the body the
//! app's own store holds, the synchronization state, and the operation ids this binding
//! issued, which make create, write and delete idempotent and let the watcher's echo of our
//! own write be told from an external edit. Nothing here is a credential, a provider
//! secret, or the body text itself.
//!
//! [`apply`] is the whole contract: a binding and a [`BindingEvent`] give a [`Transition`],
//! the new binding plus one [`Effect`] the host performs (write or delete a file, reindex,
//! keep both bodies, nothing). No file is touched here; vault I/O belongs to the Obsidian
//! plugin and the platform companions (B043, B060). The rules it enforces:
//!
//! - one body authority per note, and an explicit migration with a preview hash and a
//!   backup id to change it;
//! - a write to the vault carries the hash the writer last saw; a write whose hash is not the
//!   binding's last-seen hash is rejected into a conflict instead of overwriting, and the
//!   conflict keeps both bodies by hash until the reader chooses one or merges;
//! - a vault-owned note's app copy follows the vault file (reindex), never the reverse; an
//!   app-owned note's published copy edited outside becomes a conflict, never a silent import
//!   or a silent overwrite;
//! - an operation id names each effect once: replaying an event with an id already applied
//!   is a no-op in every state, and the watcher's report of that operation's own result is
//!   suppressed by the id, not by ignoring later file events;
//! - a vault file that disappears detaches the binding: the app keeps its copy read-only
//!   until the file is relinked (rename, reappearance) or the note is converted to app-owned.

use serde::{Deserialize, Serialize};

use openlogos_core::versioning::SchemaVersion;

/// The schema version this crate writes (`schemas/note-binding.schema.json`).
pub const SCHEMA_VERSION: &str = "1.0";

/// Pattern of a note id: lower-case letters, digits and hyphens, 4 to 64 characters, never
/// derived from a title or a path (the same shape as collection ids, ADR 0003 item 6).
pub const NOTE_ID_PATTERN: &str = "^[a-z0-9][a-z0-9-]{3,63}$";

/// Whether a string matches [`NOTE_ID_PATTERN`].
#[must_use]
pub fn is_note_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    (4..=64).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// Whether a string is a lower-case hex SHA-256.
#[must_use]
pub fn is_sha256_hex(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Whether a path is a relative vault path: forward slashes, no leading slash, no `.` or
/// `..` segment, no backslash, colon or NUL, not empty.
#[must_use]
pub fn is_relative_vault_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains(['\\', ':', '\0'])
        && path
            .split('/')
            .all(|segment| !matches!(segment, "" | "." | ".."))
}

/// Which copy of the body is authoritative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authority {
    /// The app's local note store; a vault copy, if any, is a published export.
    App,
    /// The vault file; the app holds a derived index and never writes it back unasked.
    Vault,
}

/// What raised a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictCause {
    /// A write carried a last-seen hash older than the binding's.
    StaleWrite,
    /// The vault copy of an app-owned note changed outside the app.
    ExternalEdit,
}

/// The synchronization state. The body hashes name bodies the app's note store keeps as
/// sibling revisions, content-addressed, so a conflict loses neither.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SyncState {
    /// The app's store is authoritative.
    AppOwned,
    /// The vault file is authoritative.
    VaultOwned,
    /// An explicit change of authority is previewed and not yet committed.
    Migrating {
        /// The authority after commit.
        to: Authority,
        /// Hash of the body the destination will hold, as shown in the preview.
        preview_sha256: String,
        /// Where the pre-migration body was backed up (a note-store revision id).
        backup_id: String,
    },
    /// Both bodies are kept; nothing is written until the reader chooses or merges.
    Conflict {
        /// The app's body.
        app_sha256: String,
        /// The vault's body.
        vault_sha256: String,
        /// What raised it.
        cause: ConflictCause,
    },
    /// The vault file is gone; the app copy is read-only until relinked or converted.
    Detached,
}

impl SyncState {
    /// The state's name as serialized.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            SyncState::AppOwned => "app_owned",
            SyncState::VaultOwned => "vault_owned",
            SyncState::Migrating { .. } => "migrating",
            SyncState::Conflict { .. } => "conflict",
            SyncState::Detached => "detached",
        }
    }
}

/// What an operation did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    /// Created the vault file.
    Create,
    /// Wrote the vault file.
    Write,
    /// Deleted the vault file.
    Delete,
    /// Adopted the vault body in the app; no file written, so no echo will come.
    Adopt,
}

/// One operation this binding issued.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationRecord {
    /// Unique per operation; the idempotency key.
    pub id: String,
    /// What it did.
    pub kind: OperationKind,
    /// The hash written or adopted (`None` for a delete).
    #[serde(default)]
    pub sha256: Option<String>,
    /// Whether the watcher has reported this operation's own result yet (always `true` for
    /// an adopt).
    pub echoed: bool,
}

/// The binding of one note to its body authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteBinding {
    /// `1.0`.
    pub schema_version: SchemaVersion,
    /// Stable note id ([`NOTE_ID_PATTERN`]); the identity, whatever the path.
    pub note_id: String,
    /// Who owns the body.
    pub authority: Authority,
    /// Opaque id of the vault (never its filesystem location).
    #[serde(default)]
    pub vault_id: Option<String>,
    /// Path of the note file relative to the vault root. Not identity.
    #[serde(default)]
    pub vault_path: Option<String>,
    /// SHA-256 of the vault file as the app last observed it.
    #[serde(default)]
    pub last_seen_sha256: Option<String>,
    /// SHA-256 of the body the app's own store holds.
    #[serde(default)]
    pub app_sha256: Option<String>,
    /// Synchronization state.
    pub state: SyncState,
    /// Operations issued, for idempotency and echo suppression. A host may drop records
    /// that are echoed once it no longer retries them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<OperationRecord>,
}

/// How a conflict is resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "choice", rename_all = "snake_case")]
pub enum Resolution {
    /// Keep the app's body: it is written to the vault.
    ChooseApp,
    /// Keep the vault's body: the app reindexes it.
    ChooseVault,
    /// A merged body, written to the vault and held by the app.
    Merge {
        /// Hash of the merged body.
        merged_sha256: String,
    },
}

/// Something that happened to a note or its vault file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum BindingEvent {
    /// The reader saved the body in the app (app-owned, or during a conflict).
    AppSaved {
        /// Hash of the saved body.
        sha256: String,
    },
    /// The app wants the vault file to hold this body: a write-through edit of a
    /// vault-owned note, or a publish of an app-owned one.
    WriteToVault {
        /// Idempotency key.
        operation_id: String,
        /// Hash of the body to write.
        body_sha256: String,
        /// The vault file hash the writer last saw (`None`: it believes there is no file).
        #[serde(default)]
        last_seen_sha256: Option<String>,
    },
    /// The app removes the vault file (unpublish, or delete a vault-owned note; the app's
    /// copy remains as the recoverable body).
    DeleteFromVault {
        /// Idempotency key.
        operation_id: String,
    },
    /// The watcher reports the vault file.
    VaultObserved {
        /// The operation the watcher attributes the change to, when the plugin's atomic
        /// write carried one.
        #[serde(default)]
        operation_id: Option<String>,
        /// The file's hash now; `None` when the file is gone.
        #[serde(default)]
        sha256: Option<String>,
        /// The file's path now, when it moved.
        #[serde(default)]
        vault_path: Option<String>,
    },
    /// The reader starts changing the authority, after a preview and a backup.
    BeginMigration {
        /// The authority wanted.
        to: Authority,
        /// Hash of the body the destination will hold.
        preview_sha256: String,
        /// Note-store revision holding the pre-migration body.
        backup_id: String,
        /// The vault, when migrating to one the binding does not know yet.
        #[serde(default)]
        vault_id: Option<String>,
        /// The path, when migrating to a vault.
        #[serde(default)]
        vault_path: Option<String>,
    },
    /// The reader confirms the migration.
    CommitMigration {
        /// Idempotency key.
        operation_id: String,
        /// Hash of the body written; must equal the preview.
        written_sha256: String,
    },
    /// The reader abandons the migration.
    CancelMigration,
    /// The reader resolves a conflict.
    Resolve {
        /// Idempotency key.
        operation_id: String,
        /// The choice.
        resolution: Resolution,
    },
    /// A detached note's file was found again (moved, restored).
    Relink {
        /// The vault, when it changed.
        #[serde(default)]
        vault_id: Option<String>,
        /// The path now.
        vault_path: String,
        /// The file's hash now.
        sha256: String,
    },
    /// A detached note becomes app-owned with the app's copy as its body.
    ConvertDetachedToApp,
}

impl BindingEvent {
    /// The event's name as serialized.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            BindingEvent::AppSaved { .. } => "app_saved",
            BindingEvent::WriteToVault { .. } => "write_to_vault",
            BindingEvent::DeleteFromVault { .. } => "delete_from_vault",
            BindingEvent::VaultObserved { .. } => "vault_observed",
            BindingEvent::BeginMigration { .. } => "begin_migration",
            BindingEvent::CommitMigration { .. } => "commit_migration",
            BindingEvent::CancelMigration => "cancel_migration",
            BindingEvent::Resolve { .. } => "resolve",
            BindingEvent::Relink { .. } => "relink",
            BindingEvent::ConvertDetachedToApp => "convert_detached_to_app",
        }
    }

    /// The idempotency key the event carries, if any.
    #[must_use]
    pub fn operation_id(&self) -> Option<&str> {
        match self {
            BindingEvent::WriteToVault { operation_id, .. }
            | BindingEvent::DeleteFromVault { operation_id }
            | BindingEvent::CommitMigration { operation_id, .. }
            | BindingEvent::Resolve { operation_id, .. } => Some(operation_id),
            _ => None,
        }
    }
}

/// What the host does after a transition. At most one per event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum Effect {
    /// Nothing to do outside the binding.
    None,
    /// Nothing changed (an observation equal to the last-seen hash).
    Unchanged,
    /// Create the vault file with this body, tagging the write with the operation id.
    CreateVaultFile {
        /// The operation.
        operation_id: String,
        /// Where.
        vault_path: String,
        /// What.
        sha256: String,
    },
    /// Replace the vault file's managed body, tagging the write with the operation id.
    WriteVaultFile {
        /// The operation.
        operation_id: String,
        /// Where.
        vault_path: String,
        /// What.
        sha256: String,
    },
    /// Delete the vault file, tagging the delete with the operation id.
    DeleteVaultFile {
        /// The operation.
        operation_id: String,
        /// Where.
        vault_path: String,
    },
    /// The app's derived index follows the vault body.
    Reindex {
        /// The body to index.
        sha256: String,
    },
    /// A conflict: keep both bodies as sibling revisions and show the choose/merge UI.
    KeepBothBodies {
        /// The app's body.
        app_sha256: String,
        /// The vault's body.
        vault_sha256: String,
    },
    /// The write was refused because its last-seen hash is stale; both bodies are kept.
    StaleWriteRejected {
        /// The body the writer wanted to write.
        attempted_sha256: String,
        /// The vault body the binding last saw.
        vault_sha256: String,
    },
    /// The watcher reported our own operation; nothing to do.
    SuppressedEcho {
        /// The operation.
        operation_id: String,
    },
    /// The operation was already applied; nothing to do.
    Duplicate {
        /// The operation.
        operation_id: String,
    },
    /// The file moved; the index points at the new path, the note is the same.
    PathUpdated {
        /// The path now.
        vault_path: String,
    },
    /// Show the migration preview with the backup it made.
    PreviewMigration {
        /// The authority wanted.
        to: Authority,
        /// The body the destination will hold.
        preview_sha256: String,
        /// The backup.
        backup_id: String,
    },
    /// The authority changed without a file write.
    AuthorityChanged {
        /// The authority now.
        to: Authority,
    },
    /// The vault file is gone; the app copy is read-only until relinked or converted.
    FileMissing,
    /// The published copy of an app-owned note is gone; the note is unaffected.
    PublishedCopyRemoved,
}

/// A binding after an event, and what the host does next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    /// The binding now.
    pub binding: NoteBinding,
    /// What to do.
    pub effect: Effect,
}

/// Why an event cannot be applied, or a binding is malformed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BindingError {
    /// The event is not defined in this state.
    #[error("event {event} is not allowed in state {state}")]
    InvalidTransition {
        /// The state.
        state: String,
        /// The event.
        event: String,
    },
    /// A vault effect on a binding with no vault file.
    #[error("the binding names no vault file")]
    NoVaultFile,
    /// A migration to the authority the note already has.
    #[error("the note is already {authority:?}-owned")]
    SameAuthority {
        /// The authority.
        authority: Authority,
    },
    /// A migration to a vault without a vault id and path.
    #[error("migrating to a vault needs a vault id and a path")]
    MissingVault,
    /// The committed body is not the previewed one.
    #[error("commit wrote {written} but the preview showed {preview}")]
    PreviewMismatch {
        /// Previewed.
        preview: String,
        /// Written.
        written: String,
    },
    /// An app save under vault authority: edits go through `write_to_vault`.
    #[error("a vault-owned note's body is edited through the vault file, not saved in the app")]
    AppSaveUnderVaultAuthority,
    /// An app save while the vault file is gone.
    #[error("a detached note is read-only until it is relinked or converted")]
    DetachedReadOnly,
    /// An external change reported while a migration is open.
    #[error("the vault file changed during a migration; cancel it and observe again")]
    MigrationInProgress,
    /// An operation id reused for a different operation.
    #[error("operation id {operation_id} was already used for another operation")]
    OperationReused {
        /// The id.
        operation_id: String,
    },
    /// A malformed field.
    #[error("{field}: {reason}")]
    Malformed {
        /// The field.
        field: String,
        /// What is wrong.
        reason: String,
    },
}

fn malformed(field: &str, reason: &str) -> BindingError {
    BindingError::Malformed {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }
}

fn check_hash(field: &str, hash: &str) -> Result<(), BindingError> {
    if is_sha256_hex(hash) {
        Ok(())
    } else {
        Err(malformed(field, "not a lower-case hex SHA-256"))
    }
}

fn check_path(path: &str) -> Result<(), BindingError> {
    if is_relative_vault_path(path) {
        Ok(())
    } else {
        Err(malformed("vault_path", "not a relative vault path"))
    }
}

impl NoteBinding {
    /// A new app-owned note with no vault copy.
    #[must_use]
    pub fn app_owned(note_id: &str, app_sha256: &str) -> NoteBinding {
        NoteBinding {
            schema_version: SchemaVersion::new(1, 0),
            note_id: note_id.to_owned(),
            authority: Authority::App,
            vault_id: None,
            vault_path: None,
            last_seen_sha256: None,
            app_sha256: Some(app_sha256.to_owned()),
            state: SyncState::AppOwned,
            operations: Vec::new(),
        }
    }

    /// A vault-owned note the app has indexed from a vault file.
    #[must_use]
    pub fn vault_owned(
        note_id: &str,
        vault_id: &str,
        vault_path: &str,
        sha256: &str,
    ) -> NoteBinding {
        NoteBinding {
            schema_version: SchemaVersion::new(1, 0),
            note_id: note_id.to_owned(),
            authority: Authority::Vault,
            vault_id: Some(vault_id.to_owned()),
            vault_path: Some(vault_path.to_owned()),
            last_seen_sha256: Some(sha256.to_owned()),
            app_sha256: Some(sha256.to_owned()),
            state: SyncState::VaultOwned,
            operations: Vec::new(),
        }
    }

    /// Checks the record's shape and the consistency of authority, state and vault fields.
    pub fn validate(&self) -> Result<(), BindingError> {
        if self.schema_version != SchemaVersion::new(1, 0) {
            return Err(malformed("schema_version", "this crate reads 1.0"));
        }
        if !is_note_id(&self.note_id) {
            return Err(malformed("note_id", "not a note id"));
        }
        if let Some(path) = &self.vault_path {
            check_path(path)?;
            if self.vault_id.as_deref().is_none_or(str::is_empty) {
                return Err(malformed("vault_id", "a path without a vault id"));
            }
        }
        if self.vault_id.as_deref() == Some("") {
            return Err(malformed("vault_id", "empty"));
        }
        for (field, hash) in [
            ("last_seen_sha256", &self.last_seen_sha256),
            ("app_sha256", &self.app_sha256),
        ] {
            if let Some(h) = hash {
                check_hash(field, h)?;
            }
        }
        if self.last_seen_sha256.is_some() && self.vault_path.is_none() {
            return Err(malformed(
                "last_seen_sha256",
                "a last-seen hash without a vault path",
            ));
        }
        let has_file = self.vault_path.is_some() && self.last_seen_sha256.is_some();
        match &self.state {
            SyncState::AppOwned => {
                if self.authority != Authority::App {
                    return Err(malformed("authority", "app_owned requires app"));
                }
            }
            SyncState::VaultOwned => {
                if self.authority != Authority::Vault {
                    return Err(malformed("authority", "vault_owned requires vault"));
                }
                if !has_file {
                    return Err(malformed(
                        "state",
                        "vault_owned requires a vault path and a last-seen hash",
                    ));
                }
            }
            SyncState::Migrating {
                to,
                preview_sha256,
                backup_id,
            } => {
                if *to == self.authority {
                    return Err(malformed("state", "migrating to the current authority"));
                }
                check_hash("preview_sha256", preview_sha256)?;
                if backup_id.is_empty() {
                    return Err(malformed("backup_id", "empty"));
                }
                if *to == Authority::Vault && self.vault_path.is_none() {
                    return Err(malformed("state", "migrating to a vault without a path"));
                }
            }
            SyncState::Conflict {
                app_sha256,
                vault_sha256,
                ..
            } => {
                check_hash("conflict.app_sha256", app_sha256)?;
                check_hash("conflict.vault_sha256", vault_sha256)?;
                if !has_file {
                    return Err(malformed("state", "a conflict requires a vault file"));
                }
            }
            SyncState::Detached => {
                if self.authority != Authority::Vault {
                    return Err(malformed("authority", "detached requires vault"));
                }
                if self.vault_path.is_none() {
                    return Err(malformed("state", "detached requires the last path"));
                }
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        for op in &self.operations {
            if op.id.is_empty() || !ids.insert(op.id.as_str()) {
                return Err(malformed("operations", "empty or repeated operation id"));
            }
            if let Some(h) = &op.sha256 {
                check_hash("operations.sha256", h)?;
            }
        }
        Ok(())
    }

    /// The owned state for the current authority.
    fn owned_state(&self) -> SyncState {
        match self.authority {
            Authority::App => SyncState::AppOwned,
            Authority::Vault => SyncState::VaultOwned,
        }
    }

    fn operation(&self, id: &str) -> Option<&OperationRecord> {
        self.operations.iter().find(|op| op.id == id)
    }

    /// Adds an operation record. The caller has already checked the id is new.
    fn push_operation(
        &mut self,
        id: &str,
        kind: OperationKind,
        sha256: Option<&str>,
    ) -> Result<(), BindingError> {
        if id.is_empty() {
            return Err(malformed("operation_id", "empty"));
        }
        self.operations.push(OperationRecord {
            id: id.to_owned(),
            kind,
            sha256: sha256.map(str::to_owned),
            echoed: kind == OperationKind::Adopt,
        });
        Ok(())
    }

    fn vault_path_or_err(&self) -> Result<String, BindingError> {
        match (&self.vault_id, &self.vault_path) {
            (Some(_), Some(path)) => Ok(path.clone()),
            _ => Err(BindingError::NoVaultFile),
        }
    }
}

/// The idempotency pre-check: an event whose operation id is already recorded is a
/// duplicate (no-op) when it names the same body, and a reuse error otherwise.
fn replay(binding: &NoteBinding, event: &BindingEvent) -> Option<Result<Effect, BindingError>> {
    let id = event.operation_id()?;
    let existing = binding.operation(id)?;
    let same = match event {
        BindingEvent::WriteToVault { body_sha256, .. }
        | BindingEvent::CommitMigration {
            written_sha256: body_sha256,
            ..
        } => existing.sha256.as_deref() == Some(body_sha256),
        BindingEvent::DeleteFromVault { .. } => existing.kind == OperationKind::Delete,
        BindingEvent::Resolve { resolution, .. } => match resolution {
            Resolution::Merge { merged_sha256 } => {
                existing.sha256.as_deref() == Some(merged_sha256)
            }
            Resolution::ChooseApp => existing.kind == OperationKind::Write,
            Resolution::ChooseVault => existing.kind == OperationKind::Adopt,
        },
        _ => false,
    };
    Some(if same {
        Ok(Effect::Duplicate {
            operation_id: id.to_owned(),
        })
    } else {
        Err(BindingError::OperationReused {
            operation_id: id.to_owned(),
        })
    })
}

fn invalid(binding: &NoteBinding, event: &BindingEvent) -> BindingError {
    BindingError::InvalidTransition {
        state: binding.state.name().to_owned(),
        event: event.name().to_owned(),
    }
}

/// Applies one event to a binding. Pure: the input is read, the result is a new binding and
/// the one effect the host performs. An `Err` means the event is not defined here and the
/// binding is unchanged; a rejected stale write is not an error but a transition into
/// `conflict` with [`Effect::StaleWriteRejected`].
pub fn apply(binding: &NoteBinding, event: &BindingEvent) -> Result<Transition, BindingError> {
    binding.validate()?;
    if let Some(replayed) = replay(binding, event) {
        return replayed.map(|effect| Transition {
            binding: binding.clone(),
            effect,
        });
    }
    let mut next = binding.clone();
    let done = |next: NoteBinding, effect: Effect| {
        Ok(Transition {
            binding: next,
            effect,
        })
    };

    // The watcher's echo of our own operation is suppressed in every state, by its id.
    if let BindingEvent::VaultObserved {
        operation_id: Some(op_id),
        sha256,
        vault_path,
    } = event
        && let Some(index) = next.operations.iter().position(|op| op.id == *op_id)
    {
        let own = &next.operations[index];
        let is_echo = match own.kind {
            OperationKind::Delete => sha256.is_none(),
            OperationKind::Create | OperationKind::Write => {
                sha256.is_some() && *sha256 == own.sha256
            }
            OperationKind::Adopt => false,
        };
        next.operations[index].echoed = true;
        if is_echo {
            apply_rename(&mut next, vault_path.as_deref());
            return done(
                next,
                Effect::SuppressedEcho {
                    operation_id: op_id.clone(),
                },
            );
        }
        // The operation produced something other than we wrote: what follows is treated
        // as an external change, and the echo is no longer waited for.
    }

    match (&binding.state, event) {
        // ---- app_owned -------------------------------------------------------------
        (SyncState::AppOwned, BindingEvent::AppSaved { sha256 }) => {
            check_hash("sha256", sha256)?;
            next.app_sha256 = Some(sha256.clone());
            done(next, Effect::None)
        }
        (
            SyncState::AppOwned | SyncState::VaultOwned,
            BindingEvent::WriteToVault {
                operation_id,
                body_sha256,
                last_seen_sha256,
            },
        ) => {
            check_hash("body_sha256", body_sha256)?;
            let path = next.vault_path_or_err()?;
            if *last_seen_sha256 != next.last_seen_sha256 {
                // Stale: the writer saw an older file than the binding did. Keep both.
                let vault = next
                    .last_seen_sha256
                    .clone()
                    .unwrap_or_else(|| body_sha256.clone());
                next.app_sha256 = Some(body_sha256.clone());
                next.state = SyncState::Conflict {
                    app_sha256: body_sha256.clone(),
                    vault_sha256: vault.clone(),
                    cause: ConflictCause::StaleWrite,
                };
                return done(
                    next,
                    Effect::StaleWriteRejected {
                        attempted_sha256: body_sha256.clone(),
                        vault_sha256: vault,
                    },
                );
            }
            let creating = next.last_seen_sha256.is_none();
            let kind = if creating {
                OperationKind::Create
            } else {
                OperationKind::Write
            };
            next.push_operation(operation_id, kind, Some(body_sha256))?;
            next.last_seen_sha256 = Some(body_sha256.clone());
            next.app_sha256 = Some(body_sha256.clone());
            done(next, file_effect(creating, operation_id, path, body_sha256))
        }
        (
            SyncState::AppOwned | SyncState::VaultOwned,
            BindingEvent::DeleteFromVault { operation_id },
        ) => {
            let path = next.vault_path_or_err()?;
            if next.last_seen_sha256.is_none() {
                return Err(BindingError::NoVaultFile);
            }
            next.push_operation(operation_id, OperationKind::Delete, None)?;
            next.vault_id = None;
            next.vault_path = None;
            next.last_seen_sha256 = None;
            next.authority = Authority::App;
            next.state = SyncState::AppOwned;
            done(
                next,
                Effect::DeleteVaultFile {
                    operation_id: operation_id.clone(),
                    vault_path: path,
                },
            )
        }
        (
            SyncState::AppOwned,
            BindingEvent::VaultObserved {
                sha256, vault_path, ..
            },
        ) => {
            next.vault_path_or_err()?;
            let moved = apply_rename(&mut next, vault_path.as_deref());
            match sha256 {
                None => {
                    next.vault_id = None;
                    next.vault_path = None;
                    next.last_seen_sha256 = None;
                    done(next, Effect::PublishedCopyRemoved)
                }
                Some(sha) if Some(sha) == next.last_seen_sha256.as_ref() => {
                    done(next, unchanged_or_moved(moved))
                }
                Some(sha) => {
                    check_hash("sha256", sha)?;
                    let app = next
                        .app_sha256
                        .clone()
                        .or_else(|| next.last_seen_sha256.clone())
                        .unwrap_or_else(|| sha.clone());
                    next.last_seen_sha256 = Some(sha.clone());
                    next.state = SyncState::Conflict {
                        app_sha256: app.clone(),
                        vault_sha256: sha.clone(),
                        cause: ConflictCause::ExternalEdit,
                    };
                    done(
                        next,
                        Effect::KeepBothBodies {
                            app_sha256: app,
                            vault_sha256: sha.clone(),
                        },
                    )
                }
            }
        }
        (
            SyncState::AppOwned | SyncState::VaultOwned,
            BindingEvent::BeginMigration {
                to,
                preview_sha256,
                backup_id,
                vault_id,
                vault_path,
            },
        ) => {
            if *to == next.authority {
                return Err(BindingError::SameAuthority {
                    authority: next.authority,
                });
            }
            check_hash("preview_sha256", preview_sha256)?;
            if backup_id.is_empty() {
                return Err(malformed("backup_id", "empty"));
            }
            if *to == Authority::Vault {
                if let Some(id) = vault_id {
                    if id.is_empty() {
                        return Err(malformed("vault_id", "empty"));
                    }
                    next.vault_id = Some(id.clone());
                }
                if let Some(path) = vault_path {
                    check_path(path)?;
                    next.vault_path = Some(path.clone());
                }
                if next.vault_id.is_none() || next.vault_path.is_none() {
                    return Err(BindingError::MissingVault);
                }
            }
            next.state = SyncState::Migrating {
                to: *to,
                preview_sha256: preview_sha256.clone(),
                backup_id: backup_id.clone(),
            };
            done(
                next,
                Effect::PreviewMigration {
                    to: *to,
                    preview_sha256: preview_sha256.clone(),
                    backup_id: backup_id.clone(),
                },
            )
        }

        // ---- vault_owned -----------------------------------------------------------
        (SyncState::VaultOwned, BindingEvent::AppSaved { .. }) => {
            Err(BindingError::AppSaveUnderVaultAuthority)
        }
        (
            SyncState::VaultOwned,
            BindingEvent::VaultObserved {
                sha256, vault_path, ..
            },
        ) => {
            let moved = apply_rename(&mut next, vault_path.as_deref());
            match sha256 {
                None => {
                    next.state = SyncState::Detached;
                    done(next, Effect::FileMissing)
                }
                Some(sha) if Some(sha) == next.last_seen_sha256.as_ref() => {
                    done(next, unchanged_or_moved(moved))
                }
                Some(sha) => {
                    check_hash("sha256", sha)?;
                    next.last_seen_sha256 = Some(sha.clone());
                    next.app_sha256 = Some(sha.clone());
                    done(
                        next,
                        Effect::Reindex {
                            sha256: sha.clone(),
                        },
                    )
                }
            }
        }

        // ---- migrating -------------------------------------------------------------
        (
            SyncState::Migrating {
                to, preview_sha256, ..
            },
            BindingEvent::CommitMigration {
                operation_id,
                written_sha256,
            },
        ) => {
            check_hash("written_sha256", written_sha256)?;
            if written_sha256 != preview_sha256 {
                return Err(BindingError::PreviewMismatch {
                    preview: preview_sha256.clone(),
                    written: written_sha256.clone(),
                });
            }
            match to {
                Authority::Vault => {
                    let path = next.vault_path_or_err()?;
                    let creating = next.last_seen_sha256.is_none();
                    let kind = if creating {
                        OperationKind::Create
                    } else {
                        OperationKind::Write
                    };
                    next.push_operation(operation_id, kind, Some(written_sha256))?;
                    next.authority = Authority::Vault;
                    next.state = SyncState::VaultOwned;
                    next.last_seen_sha256 = Some(written_sha256.clone());
                    next.app_sha256 = Some(written_sha256.clone());
                    done(
                        next,
                        file_effect(creating, operation_id, path, written_sha256),
                    )
                }
                Authority::App => {
                    // The vault file stays where it is, as the published copy: nothing is
                    // deleted (specs/13: never silently delete unmanaged files).
                    next.push_operation(operation_id, OperationKind::Adopt, Some(written_sha256))?;
                    next.authority = Authority::App;
                    next.state = SyncState::AppOwned;
                    next.app_sha256 = Some(written_sha256.clone());
                    done(next, Effect::AuthorityChanged { to: Authority::App })
                }
            }
        }
        (SyncState::Migrating { .. }, BindingEvent::CancelMigration) => {
            next.state = next.owned_state();
            done(next, Effect::None)
        }
        (SyncState::Migrating { .. }, BindingEvent::VaultObserved { sha256, .. }) => {
            if *sha256 == next.last_seen_sha256 {
                return done(next, Effect::Unchanged);
            }
            Err(BindingError::MigrationInProgress)
        }

        // ---- conflict --------------------------------------------------------------
        (
            SyncState::Conflict {
                vault_sha256,
                cause,
                ..
            },
            BindingEvent::AppSaved { sha256 },
        ) => {
            check_hash("sha256", sha256)?;
            next.app_sha256 = Some(sha256.clone());
            next.state = SyncState::Conflict {
                app_sha256: sha256.clone(),
                vault_sha256: vault_sha256.clone(),
                cause: *cause,
            };
            done(next, Effect::None)
        }
        (
            SyncState::Conflict {
                app_sha256, cause, ..
            },
            BindingEvent::VaultObserved {
                sha256, vault_path, ..
            },
        ) => {
            let moved = apply_rename(&mut next, vault_path.as_deref());
            match sha256 {
                None => {
                    if next.authority == Authority::Vault {
                        next.state = SyncState::Detached;
                        done(next, Effect::FileMissing)
                    } else {
                        next.vault_id = None;
                        next.vault_path = None;
                        next.last_seen_sha256 = None;
                        next.state = SyncState::AppOwned;
                        done(next, Effect::PublishedCopyRemoved)
                    }
                }
                Some(sha) if Some(sha) == next.last_seen_sha256.as_ref() => {
                    done(next, unchanged_or_moved(moved))
                }
                Some(sha) => {
                    check_hash("sha256", sha)?;
                    next.last_seen_sha256 = Some(sha.clone());
                    next.state = SyncState::Conflict {
                        app_sha256: app_sha256.clone(),
                        vault_sha256: sha.clone(),
                        cause: *cause,
                    };
                    done(
                        next,
                        Effect::KeepBothBodies {
                            app_sha256: app_sha256.clone(),
                            vault_sha256: sha.clone(),
                        },
                    )
                }
            }
        }
        (
            SyncState::Conflict {
                app_sha256,
                vault_sha256,
                ..
            },
            BindingEvent::Resolve {
                operation_id,
                resolution,
            },
        ) => {
            let path = next.vault_path_or_err()?;
            match resolution {
                Resolution::ChooseVault => {
                    next.push_operation(operation_id, OperationKind::Adopt, Some(vault_sha256))?;
                    next.app_sha256 = Some(vault_sha256.clone());
                    next.last_seen_sha256 = Some(vault_sha256.clone());
                    next.state = next.owned_state();
                    done(
                        next,
                        Effect::Reindex {
                            sha256: vault_sha256.clone(),
                        },
                    )
                }
                Resolution::ChooseApp | Resolution::Merge { .. } => {
                    let body = match resolution {
                        Resolution::Merge { merged_sha256 } => {
                            check_hash("merged_sha256", merged_sha256)?;
                            merged_sha256.clone()
                        }
                        _ => app_sha256.clone(),
                    };
                    next.push_operation(operation_id, OperationKind::Write, Some(&body))?;
                    next.app_sha256 = Some(body.clone());
                    next.last_seen_sha256 = Some(body.clone());
                    next.state = next.owned_state();
                    done(
                        next,
                        Effect::WriteVaultFile {
                            operation_id: operation_id.clone(),
                            vault_path: path,
                            sha256: body,
                        },
                    )
                }
            }
        }

        // ---- detached --------------------------------------------------------------
        (SyncState::Detached, BindingEvent::AppSaved { .. }) => Err(BindingError::DetachedReadOnly),
        (
            SyncState::Detached,
            BindingEvent::Relink {
                vault_id,
                vault_path,
                sha256,
            },
        ) => relink(next, vault_id.as_deref(), Some(vault_path), sha256),
        (
            SyncState::Detached,
            BindingEvent::VaultObserved {
                sha256, vault_path, ..
            },
        ) => match sha256 {
            None => done(next, Effect::Unchanged),
            Some(sha) => relink(next, None, vault_path.as_deref(), sha),
        },
        (SyncState::Detached, BindingEvent::ConvertDetachedToApp) => {
            next.authority = Authority::App;
            next.state = SyncState::AppOwned;
            next.vault_id = None;
            next.vault_path = None;
            next.last_seen_sha256 = None;
            done(next, Effect::AuthorityChanged { to: Authority::App })
        }

        _ => Err(invalid(binding, event)),
    }
}

fn file_effect(creating: bool, operation_id: &str, vault_path: String, sha256: &str) -> Effect {
    if creating {
        Effect::CreateVaultFile {
            operation_id: operation_id.to_owned(),
            vault_path,
            sha256: sha256.to_owned(),
        }
    } else {
        Effect::WriteVaultFile {
            operation_id: operation_id.to_owned(),
            vault_path,
            sha256: sha256.to_owned(),
        }
    }
}

/// Applies a reported rename; returns the new path if it changed. A path that is not a
/// relative vault path is ignored (the watcher is outside the vault's contract there).
fn apply_rename(next: &mut NoteBinding, reported: Option<&str>) -> Option<String> {
    let path = reported?;
    if next.vault_path.is_none()
        || next.vault_path.as_deref() == Some(path)
        || !is_relative_vault_path(path)
    {
        return None;
    }
    next.vault_path = Some(path.to_owned());
    Some(path.to_owned())
}

fn unchanged_or_moved(moved: Option<String>) -> Effect {
    match moved {
        Some(vault_path) => Effect::PathUpdated { vault_path },
        None => Effect::Unchanged,
    }
}

/// A detached note's file found again: vault-owned at the new path, reindexed if its body
/// differs from the app's copy.
fn relink(
    mut next: NoteBinding,
    vault_id: Option<&str>,
    vault_path: Option<&str>,
    sha256: &str,
) -> Result<Transition, BindingError> {
    check_hash("sha256", sha256)?;
    if let Some(id) = vault_id {
        if id.is_empty() {
            return Err(malformed("vault_id", "empty"));
        }
        next.vault_id = Some(id.to_owned());
    }
    if let Some(path) = vault_path {
        check_path(path)?;
        next.vault_path = Some(path.to_owned());
    }
    let path = next.vault_path_or_err()?;
    let same_body = next.app_sha256.as_deref() == Some(sha256);
    next.last_seen_sha256 = Some(sha256.to_owned());
    next.app_sha256 = Some(sha256.to_owned());
    next.authority = Authority::Vault;
    next.state = SyncState::VaultOwned;
    let effect = if same_body {
        Effect::PathUpdated { vault_path: path }
    } else {
        Effect::Reindex {
            sha256: sha256.to_owned(),
        }
    };
    Ok(Transition {
        binding: next,
        effect,
    })
}
