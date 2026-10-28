//! Every transition of the note-authority state machine, and every refusal.

use crate::binding::*;
use crate::credentials::find_credential_shaped;

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const D: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

fn app() -> NoteBinding {
    NoteBinding::app_owned("note-fixture-app", A)
}

fn vault() -> NoteBinding {
    NoteBinding::vault_owned(
        "note-fixture-vault",
        "vault-fixture-7f3a",
        "Study/John 3.md",
        A,
    )
}

/// An app-owned note with a published copy in the vault at body A.
fn published() -> NoteBinding {
    let step = apply(
        &app(),
        &BindingEvent::WriteToVault {
            operation_id: "op-publish".into(),
            body_sha256: A.into(),
            last_seen_sha256: None,
        },
    );
    // Publishing needs a vault first.
    assert_eq!(step.unwrap_err(), BindingError::NoVaultFile);
    let mut b = app();
    b.vault_id = Some("vault-fixture-7f3a".into());
    b.vault_path = Some("OpenLogos/note-fixture-app.md".into());
    let t = apply(
        &b,
        &BindingEvent::WriteToVault {
            operation_id: "op-publish".into(),
            body_sha256: A.into(),
            last_seen_sha256: None,
        },
    )
    .unwrap();
    assert_eq!(
        t.effect,
        Effect::CreateVaultFile {
            operation_id: "op-publish".into(),
            vault_path: "OpenLogos/note-fixture-app.md".into(),
            sha256: A.into()
        }
    );
    t.binding
}

fn observed(sha: Option<&str>) -> BindingEvent {
    BindingEvent::VaultObserved {
        operation_id: None,
        sha256: sha.map(str::to_owned),
        vault_path: None,
    }
}

fn write(op: &str, body: &str, last_seen: Option<&str>) -> BindingEvent {
    BindingEvent::WriteToVault {
        operation_id: op.into(),
        body_sha256: body.into(),
        last_seen_sha256: last_seen.map(str::to_owned),
    }
}

fn conflict_of(b: &NoteBinding) -> (String, String, ConflictCause) {
    match &b.state {
        SyncState::Conflict {
            app_sha256,
            vault_sha256,
            cause,
        } => (app_sha256.clone(), vault_sha256.clone(), *cause),
        other => panic!("not a conflict: {other:?}"),
    }
}

// ---- shape -----------------------------------------------------------------------------

#[test]
fn ids_hashes_and_paths() {
    assert!(is_note_id("note-7f3a"));
    assert!(is_note_id("0123"));
    for bad in ["abc", "Note-1", "note_1", "John 3", "", &"a".repeat(65)] {
        assert!(!is_note_id(bad), "{bad}");
    }
    assert!(is_sha256_hex(A));
    assert!(!is_sha256_hex(&A.to_uppercase()));
    assert!(!is_sha256_hex("abc"));
    for ok in ["Study/John 3.md", "note.md", "a/b/c.md", "Ψαλμοί/1.md"] {
        assert!(is_relative_vault_path(ok), "{ok}");
    }
    for bad in [
        "",
        "/abs.md",
        "../up.md",
        "a/../b.md",
        "a/./b.md",
        "a//b.md",
        "C:\\x.md",
        "a\\b.md",
        "a:b.md",
        "a/",
    ] {
        assert!(!is_relative_vault_path(bad), "{bad}");
    }
}

#[test]
fn constructors_validate_and_round_trip() {
    for b in [app(), vault(), published()] {
        b.validate().unwrap();
        let json = serde_json::to_value(&b).unwrap();
        let back: NoteBinding = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(back, b);
        assert!(find_credential_shaped(&json).is_empty(), "{json}");
    }
    assert_eq!(
        serde_json::to_value(vault()).unwrap()["state"],
        serde_json::json!({"state": "vault_owned"})
    );
}

#[test]
fn malformed_bindings_are_refused() {
    let mut b = vault();
    b.note_id = "Bad Id".into();
    assert!(
        matches!(b.validate(), Err(BindingError::Malformed { field, .. }) if field == "note_id")
    );
    let mut b = vault();
    b.vault_path = Some("../escape.md".into());
    assert!(
        matches!(b.validate(), Err(BindingError::Malformed { field, .. }) if field == "vault_path")
    );
    let mut b = vault();
    b.vault_id = None;
    assert!(
        matches!(b.validate(), Err(BindingError::Malformed { field, .. }) if field == "vault_id")
    );
    let mut b = vault();
    b.last_seen_sha256 = Some("xyz".into());
    assert!(
        matches!(b.validate(), Err(BindingError::Malformed { field, .. }) if field == "last_seen_sha256")
    );
    let mut b = vault();
    b.authority = Authority::App;
    assert!(
        matches!(b.validate(), Err(BindingError::Malformed { field, .. }) if field == "authority")
    );
    let mut b = app();
    b.state = SyncState::VaultOwned;
    assert!(b.validate().is_err());
    let mut b = app();
    b.state = SyncState::Detached;
    assert!(b.validate().is_err());
    let mut b = vault();
    b.state = SyncState::Migrating {
        to: Authority::Vault,
        preview_sha256: A.into(),
        backup_id: "rev-1".into(),
    };
    assert!(b.validate().is_err(), "migrating to the current authority");
    let mut b = app();
    b.state = SyncState::Conflict {
        app_sha256: A.into(),
        vault_sha256: B.into(),
        cause: ConflictCause::ExternalEdit,
    };
    assert!(b.validate().is_err(), "a conflict needs a vault file");
    let mut b = vault();
    b.operations = vec![
        OperationRecord {
            id: "op".into(),
            kind: OperationKind::Write,
            sha256: Some(A.into()),
            echoed: true,
        },
        OperationRecord {
            id: "op".into(),
            kind: OperationKind::Write,
            sha256: Some(A.into()),
            echoed: true,
        },
    ];
    assert!(b.validate().is_err(), "repeated operation id");
    let mut b = vault();
    b.schema_version = openlogos_core::versioning::SchemaVersion::new(2, 0);
    assert!(b.validate().is_err());
}

// ---- app_owned -------------------------------------------------------------------------

#[test]
fn app_owned_saves_locally_and_publishes_once() {
    let t = apply(&app(), &BindingEvent::AppSaved { sha256: B.into() }).unwrap();
    assert_eq!(t.effect, Effect::None);
    assert_eq!(t.binding.app_sha256.as_deref(), Some(B));
    assert_eq!(t.binding.state, SyncState::AppOwned);

    let p = published();
    assert_eq!(p.state, SyncState::AppOwned);
    assert_eq!(p.authority, Authority::App);
    assert_eq!(p.last_seen_sha256.as_deref(), Some(A));
    assert_eq!(p.operations.len(), 1);
    assert_eq!(p.operations[0].kind, OperationKind::Create);
    assert!(!p.operations[0].echoed);

    // Replaying the publish is a no-op.
    let again = apply(&p, &write("op-publish", A, None)).unwrap();
    assert_eq!(
        again.effect,
        Effect::Duplicate {
            operation_id: "op-publish".into()
        }
    );
    assert_eq!(again.binding, p);
    // The same id with another body is a reuse.
    assert_eq!(
        apply(&p, &write("op-publish", B, Some(A))).unwrap_err(),
        BindingError::OperationReused {
            operation_id: "op-publish".into()
        }
    );
}

#[test]
fn republish_carries_the_last_seen_hash() {
    let p = published();
    let edited = apply(&p, &BindingEvent::AppSaved { sha256: B.into() })
        .unwrap()
        .binding;
    let t = apply(&edited, &write("op-2", B, Some(A))).unwrap();
    assert_eq!(
        t.effect,
        Effect::WriteVaultFile {
            operation_id: "op-2".into(),
            vault_path: "OpenLogos/note-fixture-app.md".into(),
            sha256: B.into()
        }
    );
    assert_eq!(t.binding.last_seen_sha256.as_deref(), Some(B));
    assert_eq!(t.binding.state, SyncState::AppOwned);
}

#[test]
fn a_stale_republish_is_rejected_into_a_conflict_keeping_both_bodies() {
    let p = published();
    // The vault copy was edited outside (observed as C) while the app saved B.
    let drifted = apply(&p, &observed(Some(C))).unwrap();
    assert_eq!(
        drifted.effect,
        Effect::KeepBothBodies {
            app_sha256: A.into(),
            vault_sha256: C.into()
        }
    );
    assert_eq!(
        conflict_of(&drifted.binding),
        (A.into(), C.into(), ConflictCause::ExternalEdit)
    );
    // A writer that still believes the vault holds A cannot overwrite C.
    let stale_base = {
        let mut b = p.clone();
        b.last_seen_sha256 = Some(C.into());
        b
    };
    let t = apply(&stale_base, &write("op-stale", B, Some(A))).unwrap();
    assert_eq!(
        t.effect,
        Effect::StaleWriteRejected {
            attempted_sha256: B.into(),
            vault_sha256: C.into()
        }
    );
    assert_eq!(
        conflict_of(&t.binding),
        (B.into(), C.into(), ConflictCause::StaleWrite)
    );
    assert_eq!(
        t.binding.last_seen_sha256.as_deref(),
        Some(C),
        "the vault hash is untouched"
    );
    assert!(
        t.binding.operations.iter().all(|op| op.id != "op-stale"),
        "nothing was written"
    );
    assert_eq!(t.binding.authority, Authority::App);
}

#[test]
fn app_owned_observations() {
    let p = published();
    assert_eq!(
        apply(&p, &observed(Some(A))).unwrap().effect,
        Effect::Unchanged
    );
    // The published copy disappears: the note is unaffected.
    let gone = apply(&p, &observed(None)).unwrap();
    assert_eq!(gone.effect, Effect::PublishedCopyRemoved);
    assert_eq!(gone.binding.state, SyncState::AppOwned);
    assert!(gone.binding.vault_path.is_none() && gone.binding.last_seen_sha256.is_none());
    assert_eq!(gone.binding.app_sha256.as_deref(), Some(A));
    // A rename of the published copy updates the path and nothing else.
    let moved = apply(
        &p,
        &BindingEvent::VaultObserved {
            operation_id: None,
            sha256: Some(A.into()),
            vault_path: Some("OpenLogos/Renamed.md".into()),
        },
    )
    .unwrap();
    assert_eq!(
        moved.effect,
        Effect::PathUpdated {
            vault_path: "OpenLogos/Renamed.md".into()
        }
    );
    assert_eq!(moved.binding.note_id, p.note_id);
    // An observation on a note with no vault copy is a host error.
    assert_eq!(
        apply(&app(), &observed(Some(A))).unwrap_err(),
        BindingError::NoVaultFile
    );
}

#[test]
fn unpublish_deletes_the_vault_copy_idempotently() {
    let p = published();
    let t = apply(
        &p,
        &BindingEvent::DeleteFromVault {
            operation_id: "op-del".into(),
        },
    )
    .unwrap();
    assert_eq!(
        t.effect,
        Effect::DeleteVaultFile {
            operation_id: "op-del".into(),
            vault_path: "OpenLogos/note-fixture-app.md".into()
        }
    );
    assert_eq!(t.binding.state, SyncState::AppOwned);
    assert!(t.binding.vault_path.is_none());
    let again = apply(
        &t.binding,
        &BindingEvent::DeleteFromVault {
            operation_id: "op-del".into(),
        },
    )
    .unwrap();
    assert_eq!(
        again.effect,
        Effect::Duplicate {
            operation_id: "op-del".into()
        }
    );
    assert_eq!(again.binding, t.binding);
    // A fresh delete with nothing bound is refused, not silently ignored.
    assert_eq!(
        apply(
            &t.binding,
            &BindingEvent::DeleteFromVault {
                operation_id: "op-del-2".into()
            }
        )
        .unwrap_err(),
        BindingError::NoVaultFile
    );
    // Deleting a bound but never-created copy is refused too.
    let mut bound = app();
    bound.vault_id = Some("vault-fixture-7f3a".into());
    bound.vault_path = Some("x.md".into());
    assert_eq!(
        apply(
            &bound,
            &BindingEvent::DeleteFromVault {
                operation_id: "op-del-3".into()
            }
        )
        .unwrap_err(),
        BindingError::NoVaultFile
    );
}

// ---- vault_owned -----------------------------------------------------------------------

#[test]
fn vault_owned_follows_the_file_and_never_saves_in_the_app() {
    let v = vault();
    assert_eq!(
        apply(&v, &BindingEvent::AppSaved { sha256: B.into() }).unwrap_err(),
        BindingError::AppSaveUnderVaultAuthority
    );
    let t = apply(&v, &observed(Some(B))).unwrap();
    assert_eq!(t.effect, Effect::Reindex { sha256: B.into() });
    assert_eq!(t.binding.last_seen_sha256.as_deref(), Some(B));
    assert_eq!(t.binding.app_sha256.as_deref(), Some(B));
    assert_eq!(t.binding.state, SyncState::VaultOwned);
    assert_eq!(
        apply(&v, &observed(Some(A))).unwrap().effect,
        Effect::Unchanged
    );
}

#[test]
fn vault_owned_write_through_and_echo_suppression() {
    let v = vault();
    let t = apply(&v, &write("op-w", B, Some(A))).unwrap();
    assert_eq!(
        t.effect,
        Effect::WriteVaultFile {
            operation_id: "op-w".into(),
            vault_path: "Study/John 3.md".into(),
            sha256: B.into()
        }
    );
    let written = t.binding;
    assert_eq!(written.last_seen_sha256.as_deref(), Some(B));
    // The watcher reports our own write: suppressed by id, nothing reindexed.
    let echo = apply(
        &written,
        &BindingEvent::VaultObserved {
            operation_id: Some("op-w".into()),
            sha256: Some(B.into()),
            vault_path: None,
        },
    )
    .unwrap();
    assert_eq!(
        echo.effect,
        Effect::SuppressedEcho {
            operation_id: "op-w".into()
        }
    );
    assert!(echo.binding.operations[0].echoed);
    assert_eq!(echo.binding.state, SyncState::VaultOwned);
    // A later external edit under no operation id is not ignored.
    let external = apply(&echo.binding, &observed(Some(C))).unwrap();
    assert_eq!(external.effect, Effect::Reindex { sha256: C.into() });
    // An observation tagged with our id but carrying another body is an external change.
    let odd = apply(
        &written,
        &BindingEvent::VaultObserved {
            operation_id: Some("op-w".into()),
            sha256: Some(D.into()),
            vault_path: None,
        },
    )
    .unwrap();
    assert_eq!(odd.effect, Effect::Reindex { sha256: D.into() });
    assert!(odd.binding.operations[0].echoed);
}

#[test]
fn vault_owned_stale_write_is_a_conflict_and_resolutions_end_it() {
    let v = vault();
    let moved_on = apply(&v, &observed(Some(B))).unwrap().binding; // vault now B
    let t = apply(&moved_on, &write("op-stale", C, Some(A))).unwrap();
    assert_eq!(
        t.effect,
        Effect::StaleWriteRejected {
            attempted_sha256: C.into(),
            vault_sha256: B.into()
        }
    );
    let conflict = t.binding;
    assert_eq!(conflict.authority, Authority::Vault);
    assert_eq!(
        conflict_of(&conflict),
        (C.into(), B.into(), ConflictCause::StaleWrite)
    );

    // Nothing but save, observe and resolve is allowed in a conflict.
    for event in [
        write("op-x", D, Some(B)),
        BindingEvent::DeleteFromVault {
            operation_id: "op-x".into(),
        },
        BindingEvent::BeginMigration {
            to: Authority::App,
            preview_sha256: C.into(),
            backup_id: "rev".into(),
            vault_id: None,
            vault_path: None,
        },
        BindingEvent::CancelMigration,
        BindingEvent::ConvertDetachedToApp,
    ] {
        assert!(
            matches!(
                apply(&conflict, &event),
                Err(BindingError::InvalidTransition { .. })
            ),
            "{}",
            event.name()
        );
    }

    // Choose the vault: reindex, no write.
    let cv = apply(
        &conflict,
        &BindingEvent::Resolve {
            operation_id: "op-cv".into(),
            resolution: Resolution::ChooseVault,
        },
    )
    .unwrap();
    assert_eq!(cv.effect, Effect::Reindex { sha256: B.into() });
    assert_eq!(cv.binding.state, SyncState::VaultOwned);
    assert_eq!(cv.binding.app_sha256.as_deref(), Some(B));
    let replay = apply(
        &cv.binding,
        &BindingEvent::Resolve {
            operation_id: "op-cv".into(),
            resolution: Resolution::ChooseVault,
        },
    )
    .unwrap();
    assert_eq!(
        replay.effect,
        Effect::Duplicate {
            operation_id: "op-cv".into()
        }
    );

    // Choose the app: the app body is written over the vault's.
    let ca = apply(
        &conflict,
        &BindingEvent::Resolve {
            operation_id: "op-ca".into(),
            resolution: Resolution::ChooseApp,
        },
    )
    .unwrap();
    assert_eq!(
        ca.effect,
        Effect::WriteVaultFile {
            operation_id: "op-ca".into(),
            vault_path: "Study/John 3.md".into(),
            sha256: C.into()
        }
    );
    assert_eq!(ca.binding.state, SyncState::VaultOwned);
    assert_eq!(ca.binding.last_seen_sha256.as_deref(), Some(C));

    // Merge: a third body is written and held.
    let m = apply(
        &conflict,
        &BindingEvent::Resolve {
            operation_id: "op-m".into(),
            resolution: Resolution::Merge {
                merged_sha256: D.into(),
            },
        },
    )
    .unwrap();
    assert_eq!(
        m.effect,
        Effect::WriteVaultFile {
            operation_id: "op-m".into(),
            vault_path: "Study/John 3.md".into(),
            sha256: D.into()
        }
    );
    assert_eq!(m.binding.app_sha256.as_deref(), Some(D));
    assert_eq!(m.binding.last_seen_sha256.as_deref(), Some(D));
    assert_eq!(m.binding.state, SyncState::VaultOwned);
}

#[test]
fn a_conflict_keeps_tracking_both_sides_until_resolved() {
    let v = vault();
    let moved_on = apply(&v, &observed(Some(B))).unwrap().binding;
    let conflict = apply(&moved_on, &write("op-stale", C, Some(A)))
        .unwrap()
        .binding;
    // The reader keeps editing in the app: the app side moves.
    let saved = apply(&conflict, &BindingEvent::AppSaved { sha256: D.into() }).unwrap();
    assert_eq!(saved.effect, Effect::None);
    assert_eq!(
        conflict_of(&saved.binding),
        (D.into(), B.into(), ConflictCause::StaleWrite)
    );
    // The vault keeps changing: the vault side moves, both still kept.
    let vault_again = apply(&saved.binding, &observed(Some(A))).unwrap();
    assert_eq!(
        vault_again.effect,
        Effect::KeepBothBodies {
            app_sha256: D.into(),
            vault_sha256: A.into()
        }
    );
    assert_eq!(vault_again.binding.last_seen_sha256.as_deref(), Some(A));
    assert_eq!(
        apply(&vault_again.binding, &observed(Some(A)))
            .unwrap()
            .effect,
        Effect::Unchanged
    );
    // The file vanishes during a vault-authority conflict: detached, bodies already kept.
    let gone = apply(&vault_again.binding, &observed(None)).unwrap();
    assert_eq!(gone.effect, Effect::FileMissing);
    assert_eq!(gone.binding.state, SyncState::Detached);
    // During an app-authority conflict the published copy vanishing ends the conflict.
    let p = published();
    let drifted = apply(&p, &observed(Some(C))).unwrap().binding;
    let gone = apply(&drifted, &observed(None)).unwrap();
    assert_eq!(gone.effect, Effect::PublishedCopyRemoved);
    assert_eq!(gone.binding.state, SyncState::AppOwned);
    // Resolving an app-authority conflict returns to app_owned.
    let ca = apply(
        &drifted,
        &BindingEvent::Resolve {
            operation_id: "op-ca".into(),
            resolution: Resolution::ChooseApp,
        },
    )
    .unwrap();
    assert_eq!(ca.binding.state, SyncState::AppOwned);
    assert_eq!(ca.binding.authority, Authority::App);
}

// ---- migrating -------------------------------------------------------------------------

#[test]
fn migration_app_to_vault_previews_backs_up_and_commits_the_previewed_body() {
    let a = app();
    assert_eq!(
        apply(
            &a,
            &BindingEvent::BeginMigration {
                to: Authority::Vault,
                preview_sha256: A.into(),
                backup_id: "rev-backup-1".into(),
                vault_id: None,
                vault_path: None,
            }
        )
        .unwrap_err(),
        BindingError::MissingVault
    );
    assert_eq!(
        apply(
            &a,
            &BindingEvent::BeginMigration {
                to: Authority::App,
                preview_sha256: A.into(),
                backup_id: "rev-backup-1".into(),
                vault_id: None,
                vault_path: None,
            }
        )
        .unwrap_err(),
        BindingError::SameAuthority {
            authority: Authority::App
        }
    );
    let begin = BindingEvent::BeginMigration {
        to: Authority::Vault,
        preview_sha256: A.into(),
        backup_id: "rev-backup-1".into(),
        vault_id: Some("vault-fixture-7f3a".into()),
        vault_path: Some("Study/note-fixture-app.md".into()),
    };
    let t = apply(&a, &begin).unwrap();
    assert_eq!(
        t.effect,
        Effect::PreviewMigration {
            to: Authority::Vault,
            preview_sha256: A.into(),
            backup_id: "rev-backup-1".into()
        }
    );
    let migrating = t.binding;
    assert_eq!(
        migrating.authority,
        Authority::App,
        "authority moves on commit, not before"
    );
    assert!(matches!(
        migrating.state,
        SyncState::Migrating {
            to: Authority::Vault,
            ..
        }
    ));
    migrating.validate().unwrap();

    // The body is frozen while the preview is open.
    assert!(matches!(
        apply(&migrating, &BindingEvent::AppSaved { sha256: B.into() }),
        Err(BindingError::InvalidTransition { .. })
    ));
    assert!(matches!(
        apply(&migrating, &write("op", B, None)),
        Err(BindingError::InvalidTransition { .. })
    ));
    // An external change during the migration is refused, not folded in.
    assert_eq!(
        apply(&migrating, &observed(Some(B))).unwrap_err(),
        BindingError::MigrationInProgress
    );
    assert_eq!(
        apply(&migrating, &observed(None)).unwrap().effect,
        Effect::Unchanged
    );

    // Committing another body than the preview is refused.
    assert_eq!(
        apply(
            &migrating,
            &BindingEvent::CommitMigration {
                operation_id: "op-commit".into(),
                written_sha256: B.into()
            }
        )
        .unwrap_err(),
        BindingError::PreviewMismatch {
            preview: A.into(),
            written: B.into()
        }
    );
    let commit = BindingEvent::CommitMigration {
        operation_id: "op-commit".into(),
        written_sha256: A.into(),
    };
    let c = apply(&migrating, &commit).unwrap();
    assert_eq!(
        c.effect,
        Effect::CreateVaultFile {
            operation_id: "op-commit".into(),
            vault_path: "Study/note-fixture-app.md".into(),
            sha256: A.into()
        }
    );
    assert_eq!(c.binding.authority, Authority::Vault);
    assert_eq!(c.binding.state, SyncState::VaultOwned);
    assert_eq!(c.binding.last_seen_sha256.as_deref(), Some(A));
    c.binding.validate().unwrap();
    // Replaying the commit after a crash is a no-op.
    let replay = apply(&c.binding, &commit).unwrap();
    assert_eq!(
        replay.effect,
        Effect::Duplicate {
            operation_id: "op-commit".into()
        }
    );
    assert_eq!(replay.binding, c.binding);

    // Cancelling returns to the owned state with nothing written.
    let cancelled = apply(&migrating, &BindingEvent::CancelMigration).unwrap();
    assert_eq!(cancelled.effect, Effect::None);
    assert_eq!(cancelled.binding.state, SyncState::AppOwned);
    assert!(cancelled.binding.operations.is_empty());
}

#[test]
fn migration_vault_to_app_keeps_the_file_and_moves_authority() {
    let v = vault();
    let t = apply(
        &v,
        &BindingEvent::BeginMigration {
            to: Authority::App,
            preview_sha256: A.into(),
            backup_id: "rev-backup-2".into(),
            vault_id: None,
            vault_path: None,
        },
    )
    .unwrap();
    assert_eq!(t.binding.authority, Authority::Vault);
    let c = apply(
        &t.binding,
        &BindingEvent::CommitMigration {
            operation_id: "op-commit-2".into(),
            written_sha256: A.into(),
        },
    )
    .unwrap();
    assert_eq!(c.effect, Effect::AuthorityChanged { to: Authority::App });
    assert_eq!(c.binding.authority, Authority::App);
    assert_eq!(c.binding.state, SyncState::AppOwned);
    assert_eq!(
        c.binding.vault_path.as_deref(),
        Some("Study/John 3.md"),
        "the file stays as the published copy; nothing is deleted"
    );
    c.binding.validate().unwrap();
    // From here the app saves locally and the file's edits become conflicts, not imports.
    let saved = apply(&c.binding, &BindingEvent::AppSaved { sha256: B.into() }).unwrap();
    assert_eq!(saved.binding.app_sha256.as_deref(), Some(B));
    let external = apply(&saved.binding, &observed(Some(C))).unwrap();
    assert_eq!(
        external.effect,
        Effect::KeepBothBodies {
            app_sha256: B.into(),
            vault_sha256: C.into()
        }
    );
    // The commit's operation is recorded as an adopt and replays as a duplicate.
    let replay = apply(
        &c.binding,
        &BindingEvent::CommitMigration {
            operation_id: "op-commit-2".into(),
            written_sha256: A.into(),
        },
    )
    .unwrap();
    assert!(matches!(replay.effect, Effect::Duplicate { .. }));
}

// ---- detached --------------------------------------------------------------------------

#[test]
fn detached_is_read_only_until_relinked_or_converted() {
    let v = vault();
    let gone = apply(&v, &observed(None)).unwrap();
    assert_eq!(gone.effect, Effect::FileMissing);
    let d = gone.binding;
    assert_eq!(d.state, SyncState::Detached);
    assert_eq!(
        d.vault_path.as_deref(),
        Some("Study/John 3.md"),
        "the last path is kept"
    );
    d.validate().unwrap();
    assert_eq!(
        apply(&d, &BindingEvent::AppSaved { sha256: B.into() }).unwrap_err(),
        BindingError::DetachedReadOnly
    );
    assert!(matches!(
        apply(&d, &write("op", B, Some(A))),
        Err(BindingError::InvalidTransition { .. })
    ));
    assert_eq!(
        apply(&d, &observed(None)).unwrap().effect,
        Effect::Unchanged
    );

    // Relinked to the moved file with the same body: just the path.
    let r = apply(
        &d,
        &BindingEvent::Relink {
            vault_id: None,
            vault_path: "Archive/John 3.md".into(),
            sha256: A.into(),
        },
    )
    .unwrap();
    assert_eq!(
        r.effect,
        Effect::PathUpdated {
            vault_path: "Archive/John 3.md".into()
        }
    );
    assert_eq!(r.binding.state, SyncState::VaultOwned);
    assert_eq!(r.binding.note_id, v.note_id, "a move is not a new note");
    // Relinked to a file that was edited meanwhile: the vault wins, reindex.
    let r2 = apply(
        &d,
        &BindingEvent::Relink {
            vault_id: Some("vault-other".into()),
            vault_path: "John 3.md".into(),
            sha256: B.into(),
        },
    )
    .unwrap();
    assert_eq!(r2.effect, Effect::Reindex { sha256: B.into() });
    assert_eq!(r2.binding.vault_id.as_deref(), Some("vault-other"));
    // The file reappears where it was: same as a relink.
    let back = apply(&d, &observed(Some(B))).unwrap();
    assert_eq!(back.effect, Effect::Reindex { sha256: B.into() });
    assert_eq!(back.binding.state, SyncState::VaultOwned);
    // Or the reader converts the note to app-owned with the app's copy.
    let conv = apply(&d, &BindingEvent::ConvertDetachedToApp).unwrap();
    assert_eq!(conv.effect, Effect::AuthorityChanged { to: Authority::App });
    assert_eq!(conv.binding.state, SyncState::AppOwned);
    assert_eq!(conv.binding.authority, Authority::App);
    assert!(conv.binding.vault_path.is_none());
    assert_eq!(conv.binding.app_sha256.as_deref(), Some(A));
    conv.binding.validate().unwrap();
    // Relink paths are checked.
    assert!(matches!(
        apply(
            &d,
            &BindingEvent::Relink {
                vault_id: None,
                vault_path: "../x.md".into(),
                sha256: A.into()
            }
        ),
        Err(BindingError::Malformed { .. })
    ));
}

// ---- everything else is refused --------------------------------------------------------

#[test]
fn undefined_transitions_are_refused_and_leave_the_binding_alone() {
    let a = app();
    let v = vault();
    let refused: Vec<(&NoteBinding, BindingEvent)> = vec![
        (&a, BindingEvent::CancelMigration),
        (&v, BindingEvent::CancelMigration),
        (
            &a,
            BindingEvent::CommitMigration {
                operation_id: "op".into(),
                written_sha256: A.into(),
            },
        ),
        (
            &v,
            BindingEvent::Resolve {
                operation_id: "op".into(),
                resolution: Resolution::ChooseApp,
            },
        ),
        (
            &a,
            BindingEvent::Relink {
                vault_id: None,
                vault_path: "x.md".into(),
                sha256: A.into(),
            },
        ),
        (&v, BindingEvent::ConvertDetachedToApp),
        (&a, BindingEvent::ConvertDetachedToApp),
    ];
    for (binding, event) in refused {
        let error = apply(binding, &event).unwrap_err();
        assert!(
            matches!(error, BindingError::InvalidTransition { .. }),
            "{}: {error}",
            event.name()
        );
    }
    // Malformed inputs are refused before anything moves.
    assert!(matches!(
        apply(
            &a,
            &BindingEvent::AppSaved {
                sha256: "nope".into()
            }
        ),
        Err(BindingError::Malformed { .. })
    ));
    assert!(matches!(
        apply(&v, &write("", B, Some(A))),
        Err(BindingError::Malformed { .. })
    ));
    // A malformed binding is refused whatever the event.
    let mut broken = v.clone();
    broken.vault_path = None;
    assert!(matches!(
        apply(&broken, &observed(Some(A))),
        Err(BindingError::Malformed { .. })
    ));
}

#[test]
fn events_and_effects_serialize_with_tags() {
    let event = write("op-1", A, Some(B));
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event"], "write_to_vault");
    assert_eq!(serde_json::from_value::<BindingEvent>(json).unwrap(), event);
    let effect = Effect::Reindex { sha256: A.into() };
    assert_eq!(serde_json::to_value(&effect).unwrap()["effect"], "reindex");
    let resolution = Resolution::Merge {
        merged_sha256: A.into(),
    };
    assert_eq!(
        serde_json::to_value(&resolution).unwrap()["choice"],
        "merge"
    );
    let error = BindingError::NoVaultFile;
    assert_eq!(
        serde_json::to_value(&error).unwrap()["kind"],
        "no_vault_file"
    );
}

/// The example under `examples/` at the repository root, or `None` when the tree is
/// filtered (the nix checks build from `crates/` alone).
fn example(name: &str) -> Option<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/").to_owned() + name;
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(_) => {
            eprintln!("skipping {path}: not present in this source tree");
            None
        }
    }
}

#[test]
fn the_schema_examples_parse_validate_and_round_trip() {
    for name in ["note-binding.json", "note-binding-conflict.json"] {
        let Some(text) = example(name) else { return };
        let binding: NoteBinding = serde_json::from_str(&text).unwrap();
        binding.validate().unwrap();
        assert_eq!(
            serde_json::to_value(&binding).unwrap(),
            serde_json::from_str::<serde_json::Value>(&text).unwrap(),
            "{name}"
        );
    }
}

#[test]
fn no_portable_record_carries_a_credential_shaped_field() {
    // The four record kinds specs/12 and specs/03 keep credentials out of, as their
    // examples; the same check runs in tools/validate.py over the JSON.
    for name in [
        "workspace.json",
        "archive/workspace-1.0.json",
        "note-binding.json",
        "note-binding-conflict.json",
        "reading-snapshot.json",
        "research-thread.json",
    ] {
        let Some(text) = example(name) else { return };
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(find_credential_shaped(&value).is_empty(), "{name}");
    }
    // And the Rust types, serialized with every optional field present.
    let mut full = vault();
    full.state = SyncState::Conflict {
        app_sha256: A.into(),
        vault_sha256: B.into(),
        cause: ConflictCause::StaleWrite,
    };
    full.operations.push(OperationRecord {
        id: "op".into(),
        kind: OperationKind::Write,
        sha256: Some(A.into()),
        echoed: false,
    });
    assert!(find_credential_shaped(&serde_json::to_value(&full).unwrap()).is_empty());
}
