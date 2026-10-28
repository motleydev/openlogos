//! `StudyCollection` and `CollectionItem` (specs/03-data-model.md, "Collection and
//! source-selection contracts"; specs/15-reformed-study-and-collections.md, "Study
//! collections").
//!
//! A collection is a lightweight ordered set of evidence and observations with stable
//! identity. Membership is independent of a source's lifetime: removing an item never
//! deletes the note or library resource behind it. Source-bearing items keep the exact
//! edition, artifact, revision, locator, original anchor and a snapshot of the quotation
//! rights that applied; a personal observation and a research excerpt are distinct kinds
//! that never become a source quotation by being collected.

use serde::{Deserialize, Serialize};

use openlogos_core::anchor::Anchor;
use openlogos_core::versioning::SchemaVersion;

/// The shape of a collection or item id: lower-case ASCII letters, digits and dashes,
/// 4 to 64 characters, starting with a letter or digit. Stable, never derived from a title
/// or a position, and usable as an Obsidian block id with the `ol-` prefix.
pub const ITEM_ID_PATTERN: &str = "^[a-z0-9][a-z0-9-]{3,63}$";

/// Whether `id` matches [`ITEM_ID_PATTERN`].
#[must_use]
pub fn is_item_id(id: &str) -> bool {
    let len = id.len();
    (4..=64).contains(&len)
        && id
            .bytes()
            .enumerate()
            .all(|(i, b)| b.is_ascii_lowercase() || b.is_ascii_digit() || (i > 0 && b == b'-'))
}

/// What the collection is about, when it is about one thing.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StudyTarget {
    /// A primary passage in canonical form (`Rom.8.28-Rom.8.39`), if any.
    #[serde(default)]
    pub reference: Option<String>,
    /// A primary theme, if any.
    #[serde(default)]
    pub theme: Option<String>,
}

/// Whether the rights that applied at capture allow the quotation to be stored and exported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quotation {
    /// The text may be stored and exported with attribution.
    Allow,
    /// Only reference metadata may leave the reader.
    Deny,
    /// Not yet cleared: treated as deny until a rights record says otherwise.
    Unknown,
}

/// The rights that applied to the source when the item was captured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RightsSnapshot {
    /// The rights record consulted, if any (`schemas/rights-record.schema.json`).
    #[serde(default)]
    pub rights_record_id: Option<String>,
    /// Whether quotation was permitted.
    pub quotation: Quotation,
    /// SPDX identifier or the record's license label, if known.
    #[serde(default)]
    pub license_identifier: Option<String>,
    /// Attribution text to carry with any quotation, if known.
    #[serde(default)]
    pub attribution: Option<String>,
}

/// Page coordinates of a PDF source: the index and the printed label are kept apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfLocator {
    /// 0-based page index in the artifact.
    pub page_index: u32,
    /// The label printed on the page (`xiv`, `312`), if any.
    #[serde(default)]
    pub page_label: Option<String>,
}

/// Where in the work the item points.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Locator {
    /// A scripture reference in canonical form, for Bible editions.
    #[serde(default)]
    pub reference: Option<String>,
    /// A work-level locator (`Inst.3.2.7`, `WCF.1.6`), for non-Bible works.
    #[serde(default)]
    pub work_locator: Option<String>,
    /// Page coordinates, for PDF sources.
    #[serde(default)]
    pub pdf: Option<PdfLocator>,
}

impl Locator {
    /// Whether at least one coordinate is given.
    #[must_use]
    pub fn is_some(&self) -> bool {
        self.reference.is_some() || self.work_locator.is_some() || self.pdf.is_some()
    }
}

/// Display metadata shown with a source, when known.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Display {
    /// Title of the work or edition.
    #[serde(default)]
    pub title: Option<String>,
    /// Author.
    #[serde(default)]
    pub author: Option<String>,
    /// Translator or editor.
    #[serde(default)]
    pub translator_or_editor: Option<String>,
}

/// Exactly which text a source-bearing item came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceProvenance {
    /// The abstract work, when the catalog knows it.
    #[serde(default)]
    pub work_id: Option<String>,
    /// The edition the text was read in.
    pub edition_id: String,
    /// SHA-256 of the artifact the edition was built from, when a pack declares one.
    #[serde(default)]
    pub artifact_sha256: Option<String>,
    /// The content revision of the edition at capture.
    pub content_revision: String,
    /// Where in the work.
    pub locator: Locator,
    /// The original target anchor, for installed texts with a token or offset layer.
    #[serde(default)]
    pub anchor: Option<Anchor>,
    /// The rights that applied at capture.
    pub rights: RightsSnapshot,
    /// Display metadata.
    #[serde(default)]
    pub display: Display,
}

/// A source the research answer rested on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// The evidence bundle entry id in the research thread.
    pub evidence_id: String,
    /// The source it came from.
    pub source: SourceProvenance,
}

/// The kinds of item, each with the fields its kind needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemBody {
    /// A reference to a passage or work locator, without quoted text.
    SourceReference {
        /// The source.
        source: SourceProvenance,
    },
    /// A quotation from an installed text or a page of a PDF.
    SourceExcerpt {
        /// The source.
        source: SourceProvenance,
        /// The quoted text, byte-exact; `None` when the rights snapshot does not allow
        /// quotation, in which case the item is reference metadata only.
        #[serde(default)]
        text: Option<String>,
    },
    /// A personal observation by the reader.
    Observation {
        /// The text, Markdown.
        text: String,
    },
    /// A selected research answer: machine output with its label and evidence.
    ResearchExcerpt {
        /// The label the reader sees, e.g. `Generated by <provider/model>`; required so the
        /// text is never shown as a source quotation.
        generated_label: String,
        /// The answer text.
        text: String,
        /// The thread the answer came from.
        thread_id: String,
        /// The message within the thread.
        message_id: String,
        /// The sources the answer cited, each with full provenance.
        #[serde(default)]
        evidence: Vec<EvidenceRef>,
    },
    /// A margin sheet or other attachment, by content hash.
    MarginAttachment {
        /// The attachment id in the reader's store.
        attachment_id: String,
        /// Media type.
        media_type: String,
        /// SHA-256 of the bytes.
        sha256: String,
        /// The reading snapshot it was drawn over, if any.
        #[serde(default)]
        snapshot_id: Option<String>,
    },
}

impl ItemBody {
    /// The source of a source-bearing item (`source_reference`, `source_excerpt`).
    #[must_use]
    pub fn source(&self) -> Option<&SourceProvenance> {
        match self {
            ItemBody::SourceReference { source } | ItemBody::SourceExcerpt { source, .. } => {
                Some(source)
            }
            _ => None,
        }
    }

    /// Whether the item quotes or references a source, as opposed to observation,
    /// machine output or an attachment.
    #[must_use]
    pub fn is_source_bearing(&self) -> bool {
        self.source().is_some()
    }
}

/// One ordered membership in a collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionItem {
    /// Stable id ([`ITEM_ID_PATTERN`]); survives reorders, re-exports and renames.
    pub id: String,
    /// Revision of this item, from 1; bumped on any change to the item.
    pub revision: u64,
    /// The group heading the item sits under, if any.
    #[serde(default)]
    pub group_id: Option<String>,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// When the item was added, RFC 3339.
    pub added_at: String,
    /// What the item is.
    #[serde(flatten)]
    pub body: ItemBody,
}

/// A user-named heading items can be grouped under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    /// Stable id ([`ITEM_ID_PATTERN`]).
    pub id: String,
    /// The heading.
    pub title: String,
}

/// A named, ordered set of evidence and observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyCollection {
    /// Schema version of this record (`1.0`).
    pub schema_version: SchemaVersion,
    /// Stable id ([`ITEM_ID_PATTERN`]).
    pub id: String,
    /// Title.
    pub title: String,
    /// Opaque owner id (an account or device identity); collections are private.
    pub owner_id: String,
    /// What it is about, if one thing.
    #[serde(default)]
    pub target: StudyTarget,
    /// Revision of the collection as a whole, from 1; bumped on any membership, order,
    /// group or title change.
    pub revision: u64,
    /// RFC 3339.
    pub created_at: String,
    /// RFC 3339.
    pub updated_at: String,
    /// Group headings, in display order.
    #[serde(default)]
    pub groups: Vec<Group>,
    /// Tags on the collection.
    #[serde(default)]
    pub tags: Vec<String>,
    /// The items, in order.
    pub items: Vec<CollectionItem>,
}

/// Why a collection is not well formed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CollectionError {
    /// An id that does not match [`ITEM_ID_PATTERN`].
    #[error("`{id}` is not a collection, group or item id ({ITEM_ID_PATTERN})")]
    BadId {
        /// The id.
        id: String,
    },
    /// Two items or two groups with one id.
    #[error("id `{id}` is used twice")]
    DuplicateId {
        /// The id.
        id: String,
    },
    /// An item under a group the collection does not have.
    #[error("item {item_id} is under unknown group {group_id}")]
    UnknownGroup {
        /// The item.
        item_id: String,
        /// The group.
        group_id: String,
    },
    /// A revision of 0.
    #[error("{what} {id} has revision 0; revisions start at 1")]
    ZeroRevision {
        /// `collection` or `item`.
        what: String,
        /// The id.
        id: String,
    },
    /// A source-bearing item with no locator at all.
    #[error("item {item_id} has a source with no reference, work locator or page")]
    NoLocator {
        /// The item.
        item_id: String,
    },
    /// Quoted text stored although the rights snapshot does not allow quotation.
    #[error("item {item_id} stores quoted text but its rights snapshot says {quotation:?}")]
    QuotationNotAllowed {
        /// The item.
        item_id: String,
        /// The snapshot's value.
        quotation: Quotation,
    },
    /// A research excerpt with an empty label.
    #[error("item {item_id} is machine output without a generated label")]
    MissingGeneratedLabel {
        /// The item.
        item_id: String,
    },
    /// An anchor whose edition or revision differs from the provenance.
    #[error(
        "item {item_id}: anchor is in {anchor_edition}@{anchor_revision}, provenance says {edition}@{revision}"
    )]
    AnchorMismatch {
        /// The item.
        item_id: String,
        /// Edition in the anchor.
        anchor_edition: String,
        /// Revision in the anchor.
        anchor_revision: String,
        /// Edition in the provenance.
        edition: String,
        /// Revision in the provenance.
        revision: String,
    },
}

impl StudyCollection {
    /// Checks identity and provenance rules: id shapes, unique ids, known groups, revisions
    /// from 1, a locator on every source, no quoted text without permission, a label on
    /// machine output, and anchors that agree with their provenance.
    pub fn validate(&self) -> Result<(), CollectionError> {
        check_id(&self.id)?;
        if self.revision == 0 {
            return Err(CollectionError::ZeroRevision {
                what: "collection".to_owned(),
                id: self.id.clone(),
            });
        }
        let mut seen: Vec<&str> = Vec::new();
        for group in &self.groups {
            check_id(&group.id)?;
            if seen.contains(&group.id.as_str()) {
                return Err(CollectionError::DuplicateId {
                    id: group.id.clone(),
                });
            }
            seen.push(&group.id);
        }
        let mut item_ids: Vec<&str> = Vec::new();
        for item in &self.items {
            check_id(&item.id)?;
            if item_ids.contains(&item.id.as_str()) {
                return Err(CollectionError::DuplicateId {
                    id: item.id.clone(),
                });
            }
            item_ids.push(&item.id);
            if item.revision == 0 {
                return Err(CollectionError::ZeroRevision {
                    what: "item".to_owned(),
                    id: item.id.clone(),
                });
            }
            if let Some(group_id) = &item.group_id
                && !self.groups.iter().any(|g| &g.id == group_id)
            {
                return Err(CollectionError::UnknownGroup {
                    item_id: item.id.clone(),
                    group_id: group_id.clone(),
                });
            }
            item.validate_body()?;
        }
        Ok(())
    }

    /// The item with this id.
    #[must_use]
    pub fn item(&self, id: &str) -> Option<&CollectionItem> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Items that quote or reference a source.
    #[must_use]
    pub fn source_items(&self) -> Vec<&CollectionItem> {
        self.items
            .iter()
            .filter(|i| i.body.is_source_bearing())
            .collect()
    }
}

impl CollectionItem {
    /// The Obsidian block id the item exports as: `ol-` plus the item id, never a position.
    #[must_use]
    pub fn block_id(&self) -> String {
        format!("ol-{}", self.id)
    }

    /// Whether export may include quoted text for this item.
    #[must_use]
    pub fn quotation_permitted(&self) -> bool {
        match &self.body {
            ItemBody::SourceExcerpt { source, text } => {
                text.is_some() && source.rights.quotation == Quotation::Allow
            }
            _ => false,
        }
    }

    fn validate_body(&self) -> Result<(), CollectionError> {
        if let Some(source) = self.body.source() {
            if !source.locator.is_some() {
                return Err(CollectionError::NoLocator {
                    item_id: self.id.clone(),
                });
            }
            if let Some(anchor) = &source.anchor
                && (anchor.edition_id != source.edition_id
                    || anchor.content_revision != source.content_revision)
            {
                return Err(CollectionError::AnchorMismatch {
                    item_id: self.id.clone(),
                    anchor_edition: anchor.edition_id.clone(),
                    anchor_revision: anchor.content_revision.clone(),
                    edition: source.edition_id.clone(),
                    revision: source.content_revision.clone(),
                });
            }
        }
        match &self.body {
            ItemBody::SourceExcerpt {
                source,
                text: Some(_),
            } if source.rights.quotation != Quotation::Allow => {
                Err(CollectionError::QuotationNotAllowed {
                    item_id: self.id.clone(),
                    quotation: source.rights.quotation,
                })
            }
            ItemBody::ResearchExcerpt {
                generated_label, ..
            } if generated_label.trim().is_empty() => Err(CollectionError::MissingGeneratedLabel {
                item_id: self.id.clone(),
            }),
            _ => Ok(()),
        }
    }
}

fn check_id(id: &str) -> Result<(), CollectionError> {
    if is_item_id(id) {
        Ok(())
    } else {
        Err(CollectionError::BadId { id: id.to_owned() })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use openlogos_core::anchor::{ScalarRange, TextQuoteSelector, TokenId};

    pub(crate) fn source(quotation: Quotation, with_anchor: bool) -> SourceProvenance {
        SourceProvenance {
            work_id: Some("work:john".to_owned()),
            edition_id: "bsb".to_owned(),
            artifact_sha256: None,
            content_revision: "bsb-2024".to_owned(),
            locator: Locator {
                reference: Some("John.3.16".to_owned()),
                work_locator: None,
                pdf: None,
            },
            anchor: with_anchor.then(|| Anchor {
                edition_id: "bsb".to_owned(),
                content_revision: "bsb-2024".to_owned(),
                segment_id: "John.3.16".to_owned(),
                range: ScalarRange { start: 11, end: 26 },
                tokens: vec![TokenId::parse("bsb:44003016004").unwrap()],
                quote: Some(TextQuoteSelector {
                    exact: "loved the world".to_owned(),
                    prefix: "For God so ".to_owned(),
                    suffix: " that He gave".to_owned(),
                }),
            }),
            rights: RightsSnapshot {
                rights_record_id: Some("rights-bsb".to_owned()),
                quotation,
                license_identifier: Some("CC0-1.0".to_owned()),
                attribution: Some("Berean Standard Bible".to_owned()),
            },
            display: Display {
                title: Some("Berean Standard Bible".to_owned()),
                author: None,
                translator_or_editor: None,
            },
        }
    }

    pub(crate) fn item(id: &str, body: ItemBody) -> CollectionItem {
        CollectionItem {
            id: id.to_owned(),
            revision: 1,
            group_id: None,
            tags: Vec::new(),
            added_at: "2026-10-28T09:00:00Z".to_owned(),
            body,
        }
    }

    pub(crate) fn collection() -> StudyCollection {
        StudyCollection {
            schema_version: SchemaVersion::new(1, 0),
            id: "col-john-3".to_owned(),
            title: "John 3:16 study".to_owned(),
            owner_id: "device-a".to_owned(),
            target: StudyTarget {
                reference: Some("John.3.16".to_owned()),
                theme: None,
            },
            revision: 3,
            created_at: "2026-10-28T09:00:00Z".to_owned(),
            updated_at: "2026-10-28T10:00:00Z".to_owned(),
            groups: vec![Group {
                id: "grp-text".to_owned(),
                title: "The text".to_owned(),
            }],
            tags: vec!["sermon-prep".to_owned()],
            items: vec![
                CollectionItem {
                    group_id: Some("grp-text".to_owned()),
                    ..item(
                        "item-excerpt",
                        ItemBody::SourceExcerpt {
                            source: source(Quotation::Allow, true),
                            text: Some("loved the world".to_owned()),
                        },
                    )
                },
                item(
                    "item-ref",
                    ItemBody::SourceReference {
                        source: source(Quotation::Deny, false),
                    },
                ),
                item(
                    "item-restricted",
                    ItemBody::SourceExcerpt {
                        source: source(Quotation::Deny, false),
                        text: None,
                    },
                ),
                item(
                    "item-obs",
                    ItemBody::Observation {
                        text: "The verb is aorist.".to_owned(),
                    },
                ),
                item(
                    "item-research",
                    ItemBody::ResearchExcerpt {
                        generated_label: "Generated by provider/model, 2026-10-28".to_owned(),
                        text: "Summary text.".to_owned(),
                        thread_id: "thread-1".to_owned(),
                        message_id: "msg-4".to_owned(),
                        evidence: vec![EvidenceRef {
                            evidence_id: "ev-1".to_owned(),
                            source: source(Quotation::Allow, false),
                        }],
                    },
                ),
                item(
                    "item-margin",
                    ItemBody::MarginAttachment {
                        attachment_id: "att-1".to_owned(),
                        media_type: "image/png".to_owned(),
                        sha256: "a".repeat(64),
                        snapshot_id: None,
                    },
                ),
            ],
        }
    }

    #[test]
    fn a_well_formed_collection_validates_and_classifies_items() {
        let c = collection();
        c.validate().unwrap();
        assert_eq!(c.source_items().len(), 3);
        assert!(c.item("item-excerpt").unwrap().quotation_permitted());
        assert!(!c.item("item-restricted").unwrap().quotation_permitted());
        assert!(!c.item("item-obs").unwrap().quotation_permitted());
        assert!(!c.item("item-research").unwrap().body.is_source_bearing());
        assert_eq!(c.item("item-obs").unwrap().block_id(), "ol-item-obs");
        assert!(is_item_id("col-john-3") && !is_item_id("Col") && !is_item_id("-abc"));
        assert!(!is_item_id("abc") && !is_item_id(&"a".repeat(65)));
    }

    #[test]
    fn identity_and_provenance_rules_are_enforced() {
        let mut c = collection();
        c.items[1].id = c.items[0].id.clone();
        assert_eq!(
            c.validate(),
            Err(CollectionError::DuplicateId {
                id: "item-excerpt".to_owned()
            })
        );

        let mut c = collection();
        c.items[3].id = "Observation 1".to_owned();
        assert!(matches!(c.validate(), Err(CollectionError::BadId { .. })));

        let mut c = collection();
        c.items[3].group_id = Some("grp-missing".to_owned());
        assert!(matches!(
            c.validate(),
            Err(CollectionError::UnknownGroup { .. })
        ));

        let mut c = collection();
        c.items[3].revision = 0;
        assert!(matches!(
            c.validate(),
            Err(CollectionError::ZeroRevision { what, .. }) if what == "item"
        ));

        let mut c = collection();
        if let ItemBody::SourceReference { source } = &mut c.items[1].body {
            source.locator = Locator::default();
        }
        assert_eq!(
            c.validate(),
            Err(CollectionError::NoLocator {
                item_id: "item-ref".to_owned()
            })
        );

        let mut c = collection();
        if let ItemBody::SourceExcerpt { text, .. } = &mut c.items[2].body {
            *text = Some("copied anyway".to_owned());
        }
        assert_eq!(
            c.validate(),
            Err(CollectionError::QuotationNotAllowed {
                item_id: "item-restricted".to_owned(),
                quotation: Quotation::Deny
            })
        );

        let mut c = collection();
        if let ItemBody::ResearchExcerpt {
            generated_label, ..
        } = &mut c.items[4].body
        {
            *generated_label = " ".to_owned();
        }
        assert!(matches!(
            c.validate(),
            Err(CollectionError::MissingGeneratedLabel { .. })
        ));

        let mut c = collection();
        if let ItemBody::SourceExcerpt { source, .. } = &mut c.items[0].body {
            source.content_revision = "bsb-2025".to_owned();
        }
        assert!(matches!(
            c.validate(),
            Err(CollectionError::AnchorMismatch { .. })
        ));
    }

    #[test]
    fn collections_serialize_with_kind_tagged_items() {
        let c = collection();
        let json = serde_json::to_string_pretty(&c).unwrap();
        assert!(json.contains("\"kind\": \"source_excerpt\""));
        assert!(json.contains("\"kind\": \"research_excerpt\""));
        assert!(json.contains("\"schema_version\": \"1.0\""));
        assert!(json.contains("\"quotation\": \"deny\""));
        let back: StudyCollection = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
        // Optional fields may be left out of a file.
        let minimal = r#"{"id":"item-min","revision":1,"added_at":"2026-10-28T09:00:00Z","kind":"observation","text":"x"}"#;
        let item: CollectionItem = serde_json::from_str(minimal).unwrap();
        assert_eq!(item.group_id, None);
        assert!(item.tags.is_empty());
    }
}
