//! `id_lineage`: how the token ids of one content revision relate to the next.
//!
//! A new upstream release produces a token diff; the pipeline records it as rows of kind
//! `same`, `split`, `merge`, `insert` or `delete` and never renumbers ids
//! (specs/19-search-and-retrieval.md, "Token identity"). Readers use the table to carry
//! anchors, alignments and edges across the revision: [`IdLineage::map_tokens`] is the
//! "revision mapping" step of reanchoring in specs/03-data-model.md, between exact ids and
//! quote matching.

use serde::{Deserialize, Serialize};

use crate::anchor::TokenId;
use crate::versification::MappingStatus;

/// How a set of old ids relates to a set of new ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageKind {
    /// One id, same token (its surface may have been corrected): 1 → 1, same id both sides.
    Same,
    /// One old token became several new ones: 1 → 2 or more.
    Split,
    /// Several old tokens became one new one: 2 or more → 1.
    Merge,
    /// New tokens with no predecessor: 0 → 1 or more.
    Insert,
    /// Old tokens with no successor: 1 or more → 0.
    Delete,
}

/// One row of the lineage table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageEntry {
    /// The relation.
    pub kind: LineageKind,
    /// Ids in the old revision.
    pub from: Vec<TokenId>,
    /// Ids in the new revision.
    pub to: Vec<TokenId>,
}

/// The lineage between two content revisions of one edition.
///
/// Ids not mentioned in any row are *unknown*, which is different from deleted: a lineage
/// may be partial (one book of a release), and a reader must not treat silence as deletion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdLineage {
    /// The edition both revisions belong to.
    pub edition_id: String,
    /// The older content revision.
    pub from_revision: String,
    /// The newer content revision.
    pub to_revision: String,
    /// The rows, in any order.
    pub entries: Vec<LineageEntry>,
}

/// Why a lineage table is not well formed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LineageError {
    /// The old and new revision are the same string.
    #[error("lineage from {revision} to itself")]
    SameRevision {
        /// The revision.
        revision: String,
    },
    /// A row whose side counts do not fit its kind.
    #[error("row {index} is {row_kind:?} with {from} old and {to} new ids")]
    WrongShape {
        /// Row index.
        index: usize,
        /// The kind.
        row_kind: LineageKind,
        /// Old ids in the row.
        from: usize,
        /// New ids in the row.
        to: usize,
    },
    /// A `same` row whose two ids differ.
    #[error("row {index} is same but maps {from} to {to}")]
    SameButDifferent {
        /// Row index.
        index: usize,
        /// Old id.
        from: String,
        /// New id.
        to: String,
    },
    /// An id of another edition.
    #[error("row {index} names {id}, which is not in edition {edition}")]
    ForeignEdition {
        /// Row index.
        index: usize,
        /// The id.
        id: String,
        /// The lineage's edition.
        edition: String,
    },
    /// An old id in more than one row.
    #[error("old id {id} appears in more than one row")]
    DuplicateFrom {
        /// The id.
        id: String,
    },
    /// A new id in more than one row.
    #[error("new id {id} appears in more than one row")]
    DuplicateTo {
        /// The id.
        id: String,
    },
}

/// The ids of the new revision that stand for a set of old ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageResult {
    /// `exact` when every old id has at least one successor, `partial` when some do,
    /// `unmapped` when none does.
    pub status: MappingStatus,
    /// Successor ids, without duplicates, in order of first appearance.
    pub tokens: Vec<TokenId>,
    /// Old ids a `delete` row covers.
    pub deleted: Vec<TokenId>,
    /// Old ids no row mentions.
    pub unknown: Vec<TokenId>,
    /// Whether every old id came through a `same` row (identity kept, not just text).
    pub identity_kept: bool,
}

impl IdLineage {
    /// Checks the table's shape: distinct revisions, every row shaped like its kind, `same`
    /// rows keeping their id, every id in this edition, no id on the same side of two rows.
    pub fn validate(&self) -> Result<(), LineageError> {
        if self.from_revision == self.to_revision {
            return Err(LineageError::SameRevision {
                revision: self.from_revision.clone(),
            });
        }
        let mut seen_from: Vec<&TokenId> = Vec::new();
        let mut seen_to: Vec<&TokenId> = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            let (from, to) = (entry.from.len(), entry.to.len());
            let shape_ok = match entry.kind {
                LineageKind::Same => from == 1 && to == 1,
                LineageKind::Split => from == 1 && to >= 2,
                LineageKind::Merge => from >= 2 && to == 1,
                LineageKind::Insert => from == 0 && to >= 1,
                LineageKind::Delete => from >= 1 && to == 0,
            };
            if !shape_ok {
                return Err(LineageError::WrongShape {
                    index,
                    row_kind: entry.kind,
                    from,
                    to,
                });
            }
            if entry.kind == LineageKind::Same && entry.from[0] != entry.to[0] {
                return Err(LineageError::SameButDifferent {
                    index,
                    from: entry.from[0].to_string(),
                    to: entry.to[0].to_string(),
                });
            }
            for id in entry.from.iter().chain(&entry.to) {
                if id.edition != self.edition_id {
                    return Err(LineageError::ForeignEdition {
                        index,
                        id: id.to_string(),
                        edition: self.edition_id.clone(),
                    });
                }
            }
            for id in &entry.from {
                if seen_from.contains(&id) {
                    return Err(LineageError::DuplicateFrom { id: id.to_string() });
                }
                seen_from.push(id);
            }
            for id in &entry.to {
                if seen_to.contains(&id) {
                    return Err(LineageError::DuplicateTo { id: id.to_string() });
                }
                seen_to.push(id);
            }
        }
        Ok(())
    }

    /// The row whose old side holds `id`, if any.
    #[must_use]
    pub fn row_for(&self, id: &TokenId) -> Option<&LineageEntry> {
        self.entries.iter().find(|e| e.from.contains(id))
    }

    /// The successors of one old id: `None` when no row mentions it, `Some(empty)` when
    /// it was deleted.
    #[must_use]
    pub fn map_token(&self, id: &TokenId) -> Option<Vec<TokenId>> {
        self.row_for(id).map(|e| e.to.clone())
    }

    /// The successors of a set of old ids, in the new revision.
    #[must_use]
    pub fn map_tokens(&self, ids: &[TokenId]) -> LineageResult {
        let mut tokens: Vec<TokenId> = Vec::new();
        let mut deleted = Vec::new();
        let mut unknown = Vec::new();
        let mut identity_kept = true;
        let mut reached = 0usize;
        for id in ids {
            match self.row_for(id) {
                None => {
                    unknown.push(id.clone());
                    identity_kept = false;
                }
                Some(entry) if entry.to.is_empty() => {
                    deleted.push(id.clone());
                    identity_kept = false;
                }
                Some(entry) => {
                    reached += 1;
                    if entry.kind != LineageKind::Same {
                        identity_kept = false;
                    }
                    for to in &entry.to {
                        if !tokens.contains(to) {
                            tokens.push(to.clone());
                        }
                    }
                }
            }
        }
        let status = if ids.is_empty() || reached == 0 {
            MappingStatus::Unmapped
        } else if reached < ids.len() {
            MappingStatus::Partial
        } else {
            MappingStatus::Exact
        };
        LineageResult {
            status,
            tokens,
            deleted,
            unknown,
            identity_kept: identity_kept && !ids.is_empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> TokenId {
        TokenId::parse(text).unwrap()
    }

    fn ids(texts: &[&str]) -> Vec<TokenId> {
        texts.iter().map(|t| id(t)).collect()
    }

    fn row(kind: LineageKind, from: &[&str], to: &[&str]) -> LineageEntry {
        LineageEntry {
            kind,
            from: ids(from),
            to: ids(to),
        }
    }

    fn lineage() -> IdLineage {
        IdLineage {
            edition_id: "wlc".to_owned(),
            from_revision: "4.20".to_owned(),
            to_revision: "4.22".to_owned(),
            entries: vec![
                row(
                    LineageKind::Same,
                    &["wlc:01001001001"],
                    &["wlc:01001001001"],
                ),
                row(
                    LineageKind::Split,
                    &["wlc:01001001002"],
                    &["wlc:01001001002a", "wlc:01001001002b"],
                ),
                row(
                    LineageKind::Merge,
                    &["wlc:01001001003a", "wlc:01001001003b"],
                    &["wlc:01001001003"],
                ),
                row(LineageKind::Insert, &[], &["wlc:01001001008"]),
                row(LineageKind::Delete, &["wlc:01001001004"], &[]),
            ],
        }
    }

    #[test]
    fn a_well_formed_table_validates_and_maps_each_kind() {
        let l = lineage();
        l.validate().unwrap();
        assert_eq!(
            l.map_token(&id("wlc:01001001001")),
            Some(ids(&["wlc:01001001001"]))
        );
        assert_eq!(
            l.map_token(&id("wlc:01001001002")),
            Some(ids(&["wlc:01001001002a", "wlc:01001001002b"]))
        );
        assert_eq!(
            l.map_token(&id("wlc:01001001003b")),
            Some(ids(&["wlc:01001001003"]))
        );
        assert_eq!(l.map_token(&id("wlc:01001001004")), Some(Vec::new()));
        assert_eq!(l.map_token(&id("wlc:01001001005")), None);

        let same = l.map_tokens(&ids(&["wlc:01001001001"]));
        assert_eq!(same.status, MappingStatus::Exact);
        assert!(same.identity_kept);

        let moved = l.map_tokens(&ids(&["wlc:01001001001", "wlc:01001001002"]));
        assert_eq!(moved.status, MappingStatus::Exact);
        assert!(!moved.identity_kept);
        assert_eq!(
            moved.tokens,
            ids(&["wlc:01001001001", "wlc:01001001002a", "wlc:01001001002b"])
        );

        let partial = l.map_tokens(&ids(&[
            "wlc:01001001003a",
            "wlc:01001001003b",
            "wlc:01001001004",
            "wlc:01001001005",
        ]));
        assert_eq!(partial.status, MappingStatus::Partial);
        assert_eq!(partial.tokens, ids(&["wlc:01001001003"]));
        assert_eq!(partial.deleted, ids(&["wlc:01001001004"]));
        assert_eq!(partial.unknown, ids(&["wlc:01001001005"]));

        let gone = l.map_tokens(&ids(&["wlc:01001001004"]));
        assert_eq!(gone.status, MappingStatus::Unmapped);
        assert!(gone.tokens.is_empty() && !gone.identity_kept);
        let none = l.map_tokens(&[]);
        assert_eq!(none.status, MappingStatus::Unmapped);
        assert!(!none.identity_kept);
    }

    #[test]
    fn malformed_tables_are_rejected_with_the_row_index() {
        let mut l = lineage();
        l.to_revision = "4.20".to_owned();
        assert!(matches!(
            l.validate(),
            Err(LineageError::SameRevision { .. })
        ));

        let mut l = lineage();
        l.entries[1].to.truncate(1);
        assert_eq!(
            l.validate(),
            Err(LineageError::WrongShape {
                index: 1,
                row_kind: LineageKind::Split,
                from: 1,
                to: 1
            })
        );

        let mut l = lineage();
        l.entries[0].to = ids(&["wlc:01001001009"]);
        assert!(matches!(
            l.validate(),
            Err(LineageError::SameButDifferent { index: 0, .. })
        ));

        let mut l = lineage();
        l.entries[3].to = ids(&["bsb:01001001008"]);
        assert!(matches!(
            l.validate(),
            Err(LineageError::ForeignEdition { index: 3, .. })
        ));

        let mut l = lineage();
        l.entries
            .push(row(LineageKind::Delete, &["wlc:01001001001"], &[]));
        assert_eq!(
            l.validate(),
            Err(LineageError::DuplicateFrom {
                id: "wlc:01001001001".to_owned()
            })
        );

        let mut l = lineage();
        l.entries
            .push(row(LineageKind::Insert, &[], &["wlc:01001001008"]));
        assert_eq!(
            l.validate(),
            Err(LineageError::DuplicateTo {
                id: "wlc:01001001008".to_owned()
            })
        );
    }

    #[test]
    fn tables_and_results_serialize() {
        let l = lineage();
        let json = serde_json::to_string(&l).unwrap();
        assert!(json.contains("\"kind\":\"split\""));
        let back: IdLineage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, l);
        let json = serde_json::to_string(&l.map_tokens(&ids(&["wlc:01001001004"]))).unwrap();
        assert!(json.contains("\"status\":\"unmapped\""));
        assert!(json.contains("\"identity_kept\":false"));
        let json =
            serde_json::to_string(&LineageError::DuplicateFrom { id: "x".to_owned() }).unwrap();
        assert!(json.starts_with("{\"kind\":\"duplicate_from\""));
    }
}
