//! `VaultExportReceipt`: what an export to an Obsidian vault wrote, keyed by stable ids so
//! a re-export reconciles managed blocks instead of trusting file paths
//! (specs/03-data-model.md, specs/15-reformed-study-and-collections.md, "Obsidian handoff").
//!
//! A receipt maps collection and item ids to an opaque vault id, a relative resource-note
//! path, a block id, the hash of the block as last exported and the source revision it
//! quoted. A path is not identity: the item id and block id are. Where quotation was denied
//! or unknown, the entry says the block carries reference metadata only, with the reason.

use serde::{Deserialize, Serialize};

use openlogos_core::versioning::{ApiVersion, SchemaVersion};

use crate::collection::{StudyCollection, is_item_id};

/// What the exporter wrote for one item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    /// The block carries the quoted text with attribution.
    Quoted,
    /// The block carries reference metadata only; `reason` says why.
    MetadataOnly,
    /// The managed block in the vault was edited since the last export and was left as is;
    /// `reason` describes the conflict.
    Conflict,
    /// Not written; `reason` says why.
    Skipped,
}

/// The software that wrote the receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Writer {
    /// The app or plugin and its version.
    pub app: String,
    /// The core API version it embeds.
    pub core_api_version: ApiVersion,
}

/// One item's export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptEntry {
    /// The item.
    pub item_id: String,
    /// The item revision exported.
    pub item_revision: u64,
    /// Path of the resource note, relative to the vault's app-owned resource folder, with
    /// forward slashes and no `..`.
    pub resource_path: String,
    /// The block id written: `ol-` plus the item id.
    pub block_id: String,
    /// SHA-256 of the managed block's bytes as last exported, lower-case hex.
    pub last_export_sha256: String,
    /// The source content revision quoted, for source-bearing items.
    #[serde(default)]
    pub source_revision: Option<String>,
    /// What was written.
    pub disposition: Disposition,
    /// Why, for anything but `quoted`.
    #[serde(default)]
    pub reason: Option<String>,
}

/// The receipt of one export of one collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultExportReceipt {
    /// Schema version of this record (`1.0`).
    pub schema_version: SchemaVersion,
    /// Receipt id ([`crate::ITEM_ID_PATTERN`]).
    pub id: String,
    /// The collection exported.
    pub collection_id: String,
    /// Its revision at export.
    pub collection_revision: u64,
    /// Opaque id of the vault (never its filesystem path).
    pub vault_id: String,
    /// The app-owned resource folder inside the vault, relative, forward slashes.
    pub resource_folder: String,
    /// RFC 3339.
    pub exported_at: String,
    /// What wrote it.
    pub writer: Writer,
    /// One entry per item the export considered.
    pub entries: Vec<ReceiptEntry>,
}

/// Why a receipt is not well formed, or does not match its collection.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReceiptError {
    /// An id that does not match [`crate::ITEM_ID_PATTERN`].
    #[error("`{id}` is not a receipt, collection or item id")]
    BadId {
        /// The id.
        id: String,
    },
    /// A path that is absolute, escapes with `..`, or uses backslashes.
    #[error("`{path}` is not a relative vault path")]
    UnsafePath {
        /// The path.
        path: String,
    },
    /// A block id that is not `ol-` plus the item id.
    #[error("entry {item_id} has block id `{block_id}`, expected `ol-{item_id}`")]
    BlockIdMismatch {
        /// The item.
        item_id: String,
        /// The block id as given.
        block_id: String,
    },
    /// A hash that is not 64 lower-case hex digits.
    #[error("entry {item_id} hash `{hash}` is not 64 lower-case hex digits")]
    BadHash {
        /// The item.
        item_id: String,
        /// The hash.
        hash: String,
    },
    /// Two entries for one item.
    #[error("item {item_id} has two entries")]
    DuplicateEntry {
        /// The item.
        item_id: String,
    },
    /// A non-quoted disposition without a reason.
    #[error("entry {item_id} is {disposition:?} without a reason")]
    MissingReason {
        /// The item.
        item_id: String,
        /// The disposition.
        disposition: Disposition,
    },
    /// The receipt names another collection.
    #[error("receipt is for collection {receipt} but was checked against {collection}")]
    WrongCollection {
        /// Collection id in the receipt.
        receipt: String,
        /// Collection id checked against.
        collection: String,
    },
    /// An entry for an item the collection does not have.
    #[error("entry {item_id} is not an item of the collection")]
    UnknownItem {
        /// The item.
        item_id: String,
    },
    /// An entry at a revision newer than the collection knows.
    #[error("entry {item_id} is at revision {entry} but the item is at {item}")]
    FutureRevision {
        /// The item.
        item_id: String,
        /// Revision in the receipt.
        entry: u64,
        /// Revision in the collection.
        item: u64,
    },
    /// Quoted text written for an item whose rights snapshot does not allow it.
    #[error("entry {item_id} is quoted but the item does not permit quotation")]
    QuotedWithoutPermission {
        /// The item.
        item_id: String,
    },
}

/// Whether `path` is relative, forward-slashed and non-traversing.
#[must_use]
pub fn is_safe_vault_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn is_sha256_hex(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl VaultExportReceipt {
    /// Checks the receipt on its own: id shapes, safe paths, block ids derived from item
    /// ids, hash forms, one entry per item, and a reason on every non-quoted entry.
    pub fn validate(&self) -> Result<(), ReceiptError> {
        for id in [&self.id, &self.collection_id] {
            if !is_item_id(id) {
                return Err(ReceiptError::BadId { id: id.clone() });
            }
        }
        if !is_safe_vault_path(&self.resource_folder) {
            return Err(ReceiptError::UnsafePath {
                path: self.resource_folder.clone(),
            });
        }
        let mut seen: Vec<&str> = Vec::new();
        for entry in &self.entries {
            if !is_item_id(&entry.item_id) {
                return Err(ReceiptError::BadId {
                    id: entry.item_id.clone(),
                });
            }
            if seen.contains(&entry.item_id.as_str()) {
                return Err(ReceiptError::DuplicateEntry {
                    item_id: entry.item_id.clone(),
                });
            }
            seen.push(&entry.item_id);
            if !is_safe_vault_path(&entry.resource_path) {
                return Err(ReceiptError::UnsafePath {
                    path: entry.resource_path.clone(),
                });
            }
            if entry.block_id != format!("ol-{}", entry.item_id) {
                return Err(ReceiptError::BlockIdMismatch {
                    item_id: entry.item_id.clone(),
                    block_id: entry.block_id.clone(),
                });
            }
            if !is_sha256_hex(&entry.last_export_sha256) {
                return Err(ReceiptError::BadHash {
                    item_id: entry.item_id.clone(),
                    hash: entry.last_export_sha256.clone(),
                });
            }
            if entry.disposition != Disposition::Quoted
                && entry.reason.as_deref().is_none_or(|r| r.trim().is_empty())
            {
                return Err(ReceiptError::MissingReason {
                    item_id: entry.item_id.clone(),
                    disposition: entry.disposition,
                });
            }
        }
        Ok(())
    }

    /// Checks the receipt against the collection it claims to export: same collection,
    /// every entry an item, no entry ahead of its item, and no quoted block for an item
    /// whose rights do not permit quotation.
    pub fn validate_against(&self, collection: &StudyCollection) -> Result<(), ReceiptError> {
        self.validate()?;
        if self.collection_id != collection.id {
            return Err(ReceiptError::WrongCollection {
                receipt: self.collection_id.clone(),
                collection: collection.id.clone(),
            });
        }
        for entry in &self.entries {
            let Some(item) = collection.item(&entry.item_id) else {
                return Err(ReceiptError::UnknownItem {
                    item_id: entry.item_id.clone(),
                });
            };
            if entry.item_revision > item.revision {
                return Err(ReceiptError::FutureRevision {
                    item_id: entry.item_id.clone(),
                    entry: entry.item_revision,
                    item: item.revision,
                });
            }
            if entry.disposition == Disposition::Quoted && !item.quotation_permitted() {
                return Err(ReceiptError::QuotedWithoutPermission {
                    item_id: entry.item_id.clone(),
                });
            }
        }
        Ok(())
    }

    /// Items of `collection` that changed since this export, or were never exported, by id.
    #[must_use]
    pub fn stale_items(&self, collection: &StudyCollection) -> Vec<String> {
        collection
            .items
            .iter()
            .filter(|item| {
                self.entries
                    .iter()
                    .find(|e| e.item_id == item.id)
                    .is_none_or(|e| e.item_revision < item.revision)
            })
            .map(|item| item.id.clone())
            .collect()
    }

    /// Entries whose item is no longer in `collection`: their blocks are managed blocks to
    /// remove or mark, never to retarget.
    #[must_use]
    pub fn orphaned_entries(&self, collection: &StudyCollection) -> Vec<&ReceiptEntry> {
        self.entries
            .iter()
            .filter(|e| collection.item(&e.item_id).is_none())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collection::tests::collection;

    fn entry(
        item_id: &str,
        revision: u64,
        disposition: Disposition,
        reason: Option<&str>,
    ) -> ReceiptEntry {
        ReceiptEntry {
            item_id: item_id.to_owned(),
            item_revision: revision,
            resource_path: format!("OpenLogos/John 3-16 study/{item_id}.md"),
            block_id: format!("ol-{item_id}"),
            last_export_sha256: "b".repeat(64),
            source_revision: Some("bsb-2024".to_owned()),
            disposition,
            reason: reason.map(str::to_owned),
        }
    }

    fn receipt() -> VaultExportReceipt {
        VaultExportReceipt {
            schema_version: SchemaVersion::new(1, 0),
            id: "rcpt-0001".to_owned(),
            collection_id: "col-john-3".to_owned(),
            collection_revision: 3,
            vault_id: "vault-7f3a".to_owned(),
            resource_folder: "OpenLogos".to_owned(),
            exported_at: "2026-10-28T10:05:00Z".to_owned(),
            writer: Writer {
                app: "openlogos-obsidian 0.1.0".to_owned(),
                core_api_version: ApiVersion::current(),
            },
            entries: vec![
                entry("item-excerpt", 1, Disposition::Quoted, None),
                entry(
                    "item-ref",
                    1,
                    Disposition::MetadataOnly,
                    Some("quotation denied by rights record rights-bsb"),
                ),
                entry(
                    "item-restricted",
                    1,
                    Disposition::MetadataOnly,
                    Some("quotation denied"),
                ),
                entry("item-obs", 1, Disposition::Quoted, None),
            ],
        }
    }

    #[test]
    fn a_receipt_validates_alone_and_against_its_collection() {
        let r = receipt();
        r.validate().unwrap();
        let c = collection();
        // Observations are the reader's own text, which the exporter writes in full; the
        // `quoted` disposition is reserved for source quotations, so this entry is wrong.
        let err = r.validate_against(&c).unwrap_err();
        assert_eq!(
            err,
            ReceiptError::QuotedWithoutPermission {
                item_id: "item-obs".to_owned()
            }
        );
        let mut r = receipt();
        r.entries[3].disposition = Disposition::Skipped;
        r.entries[3].reason = Some("observations are written into the index note".to_owned());
        r.validate_against(&c).unwrap();
        assert_eq!(
            r.stale_items(&c),
            vec!["item-research".to_owned(), "item-margin".to_owned()]
        );
        assert!(r.orphaned_entries(&c).is_empty());

        let mut later = c.clone();
        later.items[0].revision = 2;
        later.items.remove(1);
        assert_eq!(
            r.stale_items(&later),
            vec![
                "item-excerpt".to_owned(),
                "item-research".to_owned(),
                "item-margin".to_owned()
            ]
        );
        assert_eq!(r.orphaned_entries(&later)[0].item_id, "item-ref");
    }

    #[test]
    fn paths_and_ids_are_not_identity_and_are_checked() {
        let mut r = receipt();
        r.entries[0].resource_path = "../outside.md".to_owned();
        assert!(matches!(r.validate(), Err(ReceiptError::UnsafePath { .. })));
        let mut r = receipt();
        r.entries[0].resource_path = "C:\\vault\\x.md".to_owned();
        assert!(matches!(r.validate(), Err(ReceiptError::UnsafePath { .. })));
        let mut r = receipt();
        r.resource_folder = "/abs".to_owned();
        assert!(matches!(r.validate(), Err(ReceiptError::UnsafePath { .. })));
        let mut r = receipt();
        r.entries[0].block_id = "^abc123".to_owned();
        assert!(matches!(
            r.validate(),
            Err(ReceiptError::BlockIdMismatch { .. })
        ));
        let mut r = receipt();
        r.entries[0].last_export_sha256 = "ABC".to_owned();
        assert!(matches!(r.validate(), Err(ReceiptError::BadHash { .. })));
        let mut r = receipt();
        r.entries
            .push(entry("item-excerpt", 1, Disposition::Quoted, None));
        assert!(matches!(
            r.validate(),
            Err(ReceiptError::DuplicateEntry { .. })
        ));
        let mut r = receipt();
        r.entries[1].reason = None;
        assert!(matches!(
            r.validate(),
            Err(ReceiptError::MissingReason { .. })
        ));
        let mut r = receipt();
        r.id = "Receipt 1".to_owned();
        assert!(matches!(r.validate(), Err(ReceiptError::BadId { .. })));
        assert!(is_safe_vault_path("a/b.md") && !is_safe_vault_path("a//b.md"));
        assert!(!is_safe_vault_path("./a.md") && !is_safe_vault_path(""));
    }

    #[test]
    fn mismatches_with_the_collection_are_named() {
        let c = collection();
        let mut r = receipt();
        r.entries[3].disposition = Disposition::Skipped;
        r.entries[3].reason = Some("x".to_owned());
        let base = r.clone();

        let mut r = base.clone();
        r.collection_id = "col-other".to_owned();
        assert!(matches!(
            r.validate_against(&c),
            Err(ReceiptError::WrongCollection { .. })
        ));
        let mut r = base.clone();
        r.entries[0].item_id = "item-gone".to_owned();
        r.entries[0].block_id = "ol-item-gone".to_owned();
        assert!(matches!(
            r.validate_against(&c),
            Err(ReceiptError::UnknownItem { .. })
        ));
        let mut r = base.clone();
        r.entries[0].item_revision = 9;
        assert!(matches!(
            r.validate_against(&c),
            Err(ReceiptError::FutureRevision {
                entry: 9,
                item: 1,
                ..
            })
        ));
        let mut r = base;
        r.entries[2].disposition = Disposition::Quoted;
        r.entries[2].reason = None;
        assert!(matches!(
            r.validate_against(&c),
            Err(ReceiptError::QuotedWithoutPermission { .. })
        ));
    }

    #[test]
    fn receipts_serialize() {
        let r = receipt();
        let json = serde_json::to_string_pretty(&r).unwrap();
        assert!(json.contains("\"disposition\": \"metadata_only\""));
        assert!(json.contains(&format!(
            "\"core_api_version\": \"{}\"",
            openlogos_core::API_VERSION
        )));
        let back: VaultExportReceipt = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }
}
