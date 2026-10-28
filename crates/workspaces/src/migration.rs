//! Workspace schema migrations under ADR 0003: pure functions over the JSON value, forward
//! only, idempotent, one step per MINOR bump, each covered by a fixture
//! (docs/adr/0003-identity-and-schema-versioning.md, "Migrations").
//!
//! [`migrate_workspace`] takes a document, the version it is at and the version wanted, and
//! walks the registered [`STEPS`] in order. A step never invents data: a field it cannot
//! derive stays at its default, and everything the document already says (pane ids, link
//! groups, pinned state, anchors, the active pane) is carried byte for byte. The result
//! validates against the target version's schema and derives the same layout as the
//! original, which the `workspace-migration-1-0-to-1-1` conformance fixture checks on every
//! host.
//!
//! # Adding the next step
//!
//! A new MINOR version `1.(n+1)` means, in this order: the schema file gains the version in
//! its `schema_version` enum and the new optional fields with defaults; the current example
//! moves to `examples/archive/workspace-1.n.json` and a new example is written at
//! `1.(n+1)`; a `fn migrate_1_n_to_1_n1(Value) -> Result<Value, MigrationError>` is added
//! here and appended to [`STEPS`]; [`crate::SCHEMA_VERSIONS`] and
//! [`crate::CURRENT_SCHEMA_VERSION`] are bumped; the migration fixture gains a case from
//! `1.n` to `1.(n+1)` whose expected document is the archived example migrated by hand; the
//! unit test `steps_are_contiguous_and_end_at_the_current_version` keeps the registry honest.
//! A step that would rename, remove or re-type a field is a MAJOR bump and does not belong
//! in this registry.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{CURRENT_SCHEMA_VERSION, SCHEMA_VERSIONS};

/// One registered migration step.
pub struct Step {
    /// The version the step reads.
    pub from: &'static str,
    /// The version it writes.
    pub to: &'static str,
    /// The pure function.
    pub apply: fn(Value) -> Result<Value, MigrationError>,
}

/// Every forward step, in version order. Each entry's `to` is the next entry's `from`, and
/// the last entry ends at [`CURRENT_SCHEMA_VERSION`].
pub const STEPS: [Step; 1] = [Step {
    from: "1.0",
    to: "1.1",
    apply: migrate_1_0_to_1_1,
}];

/// Why a document could not be migrated.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MigrationError {
    /// A version this crate does not know.
    #[error("unknown workspace schema version {version}")]
    UnknownVersion {
        /// The version named.
        version: String,
    },
    /// Migrations are forward only.
    #[error("cannot migrate backwards from {from} to {to}")]
    Backwards {
        /// The version migrated from.
        from: String,
        /// The version asked for.
        to: String,
    },
    /// The document says it is at another version than the caller claims.
    #[error("document declares schema_version {declared}, not {expected}")]
    VersionMismatch {
        /// What the document says.
        declared: String,
        /// What the caller said.
        expected: String,
    },
    /// The document is not a JSON object with the fields a workspace has.
    #[error("not a workspace document: {reason}")]
    NotAWorkspace {
        /// What is wrong.
        reason: String,
    },
}

fn version_index(version: &str) -> Result<usize, MigrationError> {
    SCHEMA_VERSIONS
        .iter()
        .position(|v| *v == version)
        .ok_or_else(|| MigrationError::UnknownVersion {
            version: version.to_owned(),
        })
}

fn object(value: &Value) -> Result<&Map<String, Value>, MigrationError> {
    value
        .as_object()
        .ok_or_else(|| MigrationError::NotAWorkspace {
            reason: "the document is not a JSON object".to_owned(),
        })
}

/// Migrates a workspace document from `from` to `to`, applying every registered step in
/// between. `from` must equal the document's own `schema_version`; `to` equal to `from`
/// returns the document unchanged, so migrating twice is the same as migrating once.
pub fn migrate_workspace(value: Value, from: &str, to: &str) -> Result<Value, MigrationError> {
    let start = version_index(from)?;
    let end = version_index(to)?;
    if end < start {
        return Err(MigrationError::Backwards {
            from: from.to_owned(),
            to: to.to_owned(),
        });
    }
    let declared = object(&value)?
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| MigrationError::NotAWorkspace {
            reason: "schema_version is missing or not a string".to_owned(),
        })?;
    if declared != from {
        return Err(MigrationError::VersionMismatch {
            declared: declared.to_owned(),
            expected: from.to_owned(),
        });
    }
    let mut current = value;
    for step in STEPS.iter().filter(|s| {
        let index = version_index(s.from).unwrap_or(usize::MAX);
        index >= start && index < end
    }) {
        current = (step.apply)(current)?;
    }
    Ok(current)
}

/// Migrates a document at any known version to [`CURRENT_SCHEMA_VERSION`].
pub fn migrate_workspace_to_current(value: Value) -> Result<Value, MigrationError> {
    let from = object(&value)?
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| MigrationError::NotAWorkspace {
            reason: "schema_version is missing or not a string".to_owned(),
        })?
        .to_owned();
    migrate_workspace(value, &from, CURRENT_SCHEMA_VERSION)
}

/// 1.0 → 1.1 (B062): `layout_profiles` becomes required and is set to the empty object, so
/// every device class is derived from the pane order exactly as the 1.0 reader did; no
/// pane gains a `scroll_anchor` (absent means the top of the passage, as before). Nothing
/// else moves.
fn migrate_1_0_to_1_1(value: Value) -> Result<Value, MigrationError> {
    let mut doc = match value {
        Value::Object(map) => map,
        _ => {
            return Err(MigrationError::NotAWorkspace {
                reason: "the document is not a JSON object".to_owned(),
            });
        }
    };
    if !doc.get("panes").is_some_and(Value::is_array) {
        return Err(MigrationError::NotAWorkspace {
            reason: "panes is missing or not an array".to_owned(),
        });
    }
    doc.insert("schema_version".to_owned(), Value::String("1.1".to_owned()));
    doc.entry("layout_profiles")
        .or_insert_with(|| Value::Object(Map::new()));
    Ok(Value::Object(doc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeviceClass, ProfileSource, TextSize, Workspace, derive_profile};
    use serde_json::json;

    fn one_point_zero() -> Value {
        json!({
            "schema_version": "1.0",
            "fixture_only": true,
            "id": "fixture:workspace:1",
            "title": "Three texts plus optional notes dock",
            "panes": [
                {"id": "p1", "kind": "reading", "content_id": "fixture:edition-1", "content_revision": "0", "reference": "fixture:passage:1", "link_group": "A", "pinned": false, "width_fraction": 0.3333333333333333},
                {"id": "p2", "kind": "reading", "content_id": "fixture:edition-2", "content_revision": "0", "reference": "fixture:passage:1", "link_group": "A", "pinned": false, "width_fraction": 0.3333333333333333},
                {"id": "p3", "kind": "reading", "content_id": "fixture:edition-3", "content_revision": "0", "reference": "fixture:passage:1", "link_group": null, "pinned": true, "width_fraction": 0.3333333333333333}
            ],
            "notes_dock_id": "fixture:note:1",
            "active_pane_id": "p1"
        })
    }

    #[test]
    fn steps_are_contiguous_and_end_at_the_current_version() {
        assert_eq!(STEPS.len(), SCHEMA_VERSIONS.len() - 1);
        for (i, step) in STEPS.iter().enumerate() {
            assert_eq!(step.from, SCHEMA_VERSIONS[i]);
            assert_eq!(step.to, SCHEMA_VERSIONS[i + 1]);
        }
        assert_eq!(STEPS[STEPS.len() - 1].to, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn one_point_zero_migrates_to_one_point_one_with_empty_profiles() {
        let migrated = migrate_workspace(one_point_zero(), "1.0", "1.1").unwrap();
        assert_eq!(migrated["schema_version"], "1.1");
        assert_eq!(migrated["layout_profiles"], json!({}));
        // Everything else is byte for byte the 1.0 document.
        let mut expected = one_point_zero();
        expected["schema_version"] = json!("1.1");
        expected["layout_profiles"] = json!({});
        assert_eq!(migrated, expected);
        for pane in migrated["panes"].as_array().unwrap() {
            assert!(pane.get("scroll_anchor").is_none(), "no anchor is invented");
        }
    }

    #[test]
    fn migrating_is_idempotent() {
        let once = migrate_workspace(one_point_zero(), "1.0", "1.1").unwrap();
        let twice = migrate_workspace(once.clone(), "1.1", "1.1").unwrap();
        assert_eq!(once, twice);
        let again = migrate_workspace_to_current(once.clone()).unwrap();
        assert_eq!(once, again);
    }

    #[test]
    fn same_version_is_the_identity() {
        let doc = one_point_zero();
        assert_eq!(migrate_workspace(doc.clone(), "1.0", "1.0").unwrap(), doc);
    }

    #[test]
    fn the_migrated_document_parses_and_derives_the_same_layout() {
        let before: Workspace = serde_json::from_value(one_point_zero()).unwrap();
        let after: Workspace =
            serde_json::from_value(migrate_workspace(one_point_zero(), "1.0", "1.1").unwrap())
                .unwrap();
        assert_eq!(
            after.layout_profiles,
            Some(crate::LayoutProfiles::default())
        );
        for class in DeviceClass::ALL {
            for (width, size) in [
                (1440.0, TextSize::Standard),
                (1024.0, TextSize::Standard),
                (1024.0, TextSize::Accessibility),
                (390.0, TextSize::Standard),
            ] {
                let a = derive_profile(&before, class, width, size).unwrap();
                let b = derive_profile(&after, class, width, size).unwrap();
                assert_eq!(a, b, "{class} at {width}");
                assert_eq!(b.profile_source, ProfileSource::PaneOrder);
            }
        }
        // The 1.0 three-pane example on an iPad: three columns, as B062's reader derived.
        let ipad =
            derive_profile(&after, DeviceClass::Regular, 1024.0, TextSize::Standard).unwrap();
        assert_eq!(ipad.shown_pane_ids(), vec!["p1", "p2", "p3"]);
        assert!(ipad.tabs.is_empty());
    }

    #[test]
    fn backwards_unknown_and_mismatched_versions_are_refused() {
        let migrated = migrate_workspace(one_point_zero(), "1.0", "1.1").unwrap();
        assert_eq!(
            migrate_workspace(migrated, "1.1", "1.0").unwrap_err(),
            MigrationError::Backwards {
                from: "1.1".into(),
                to: "1.0".into()
            }
        );
        assert_eq!(
            migrate_workspace(one_point_zero(), "1.0", "2.0").unwrap_err(),
            MigrationError::UnknownVersion {
                version: "2.0".into()
            }
        );
        assert_eq!(
            migrate_workspace(one_point_zero(), "1.1", "1.1").unwrap_err(),
            MigrationError::VersionMismatch {
                declared: "1.0".into(),
                expected: "1.1".into()
            }
        );
        assert!(matches!(
            migrate_workspace(json!([]), "1.0", "1.1").unwrap_err(),
            MigrationError::NotAWorkspace { .. }
        ));
        assert!(matches!(
            migrate_workspace(json!({"schema_version": "1.0"}), "1.0", "1.1").unwrap_err(),
            MigrationError::NotAWorkspace { .. }
        ));
    }

    #[test]
    fn a_document_that_already_has_profiles_keeps_them() {
        // A 1.0 reader never writes profiles, but a step must not clobber data it finds.
        let mut doc = one_point_zero();
        doc["layout_profiles"] = json!({"compact": {"visible_pane_ids": ["p2"], "tab_order": ["p1", "p3"], "notes_dock_open": false}});
        let migrated = migrate_workspace(doc.clone(), "1.0", "1.1").unwrap();
        assert_eq!(migrated["layout_profiles"], doc["layout_profiles"]);
    }
}
