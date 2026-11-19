//! Cross references (B057) on the native surface: records mirroring
//! `openlogos_text::reading::cross_references` field for field, the `ReadingHandle` calls that
//! answer them, and `ShellHandle.set_data_layers`, which enables the B077 row once a pack is
//! open. Additive; nothing existing is renamed.
//!
//! A host opens the relation pack (`open_cross_references`, the `relations/openbible-xrefs.sqlite`
//! of a preview pack; `retrieval` feature), registers `cross_references` with
//! `ShellHandle.set_data_layers`, and on `nav.cross_references` asks `cross_references` for the
//! passage in the tab's edition, a page at a time. An item opens with `tab.open_edition`
//! (`resource_id`, `target`), which leaves the main passage where it is.

use openlogos_text::reading::{
    self as core, CrossReferenceError, CrossReferenceInfo, CrossReferenceOptions,
};

use super::reading::{ReadingCallError, ReadingChunk, ReadingHandle};
use super::shell::ShellHandle;
use super::versification::MappingStatus;

/// What the installed cross-reference source is (`CrossReferenceInfo`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReadingCrossReferenceSource {
    /// The rights record (`S09`).
    pub source_id: String,
    /// The data's edition.
    pub edition_id: String,
    /// The scheme its references are in (`eng`).
    pub scheme: String,
    /// SPDX licence.
    pub license: String,
    /// The attribution a reader must be shown.
    pub attribution: String,
    /// `cross_reference`.
    pub relation: String,
    /// `unspecified`: the source does not say what kind of link it is.
    pub kind: String,
    /// What the votes are, in the source's words.
    pub votes_meaning: String,
    /// `source_attested`.
    pub review_state: String,
    /// What the data covers.
    pub coverage: String,
    /// Published links.
    pub rows: u64,
}

impl From<CrossReferenceInfo> for ReadingCrossReferenceSource {
    fn from(i: CrossReferenceInfo) -> Self {
        ReadingCrossReferenceSource {
            source_id: i.source_id,
            edition_id: i.edition_id,
            scheme: i.scheme,
            license: i.license,
            attribution: i.attribution,
            relation: i.relation,
            kind: i.kind,
            votes_meaning: i.votes_meaning,
            review_state: i.review_state,
            coverage: i.coverage,
            rows: i.rows,
        }
    }
}

/// Whether the edition prints a target (`Availability`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ReadingCrossReferenceAvailability {
    /// Every verse is printed.
    Printed,
    /// Some verses are.
    PartlyPrinted,
    /// The edition has the book and prints none of the target.
    NotPrinted,
    /// The edition does not have the book.
    BookNotInEdition,
    /// The target does not map into the edition's scheme.
    Unmapped,
}

impl From<core::Availability> for ReadingCrossReferenceAvailability {
    fn from(a: core::Availability) -> Self {
        match a {
            core::Availability::Printed => ReadingCrossReferenceAvailability::Printed,
            core::Availability::PartlyPrinted => ReadingCrossReferenceAvailability::PartlyPrinted,
            core::Availability::NotPrinted => ReadingCrossReferenceAvailability::NotPrinted,
            core::Availability::BookNotInEdition => {
                ReadingCrossReferenceAvailability::BookNotInEdition
            }
            core::Availability::Unmapped => ReadingCrossReferenceAvailability::Unmapped,
        }
    }
}

/// Which way a link was published (`Direction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ReadingCrossReferenceDirection {
    /// Listed under the passage: what the source says.
    AsPublished,
    /// Listed under the target and turned round: a navigation aid, not a claim.
    ReverseDerived,
}

impl From<core::CrossReferenceDirection> for ReadingCrossReferenceDirection {
    fn from(d: core::CrossReferenceDirection) -> Self {
        match d {
            core::CrossReferenceDirection::AsPublished => {
                ReadingCrossReferenceDirection::AsPublished
            }
            core::CrossReferenceDirection::ReverseDerived => {
                ReadingCrossReferenceDirection::ReverseDerived
            }
        }
    }
}

/// One assertion behind an item.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReadingCrossReferenceAssertion {
    /// The verse it is listed under, in the source's scheme.
    pub from: String,
    /// Its target as published.
    pub to: String,
    /// Votes as published.
    pub votes: i32,
    /// Line of the published file.
    pub line: u32,
    /// As published or turned round.
    pub direction: ReadingCrossReferenceDirection,
}

/// One item of the list (`CrossReference`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReadingCrossReference {
    /// The target in the edition's numbering, OSIS; what `tab.open_edition` opens.
    pub target: Option<String>,
    /// For display, in the edition's numbering when it maps.
    pub label: String,
    /// As published, OSIS.
    pub source_target: String,
    /// As published, for display.
    pub source_label: String,
    /// How the published target maps into the edition's scheme.
    pub mapping: MappingStatus,
    /// Whether the edition prints it.
    pub availability: ReadingCrossReferenceAvailability,
    /// The edition's text of its first verses.
    pub preview: Option<String>,
    /// Whether the preview stops before the end of the target.
    pub preview_truncated: bool,
    /// The chunk to supply before the preview can be read (a browser).
    pub preview_chunk: Option<ReadingChunk>,
    /// `cross_reference`.
    pub relation: String,
    /// `unspecified`.
    pub kind: String,
    /// As published if any assertion is.
    pub direction: ReadingCrossReferenceDirection,
    /// The best published votes shown this way round.
    pub votes: i32,
    /// Every assertion, best first.
    pub assertions: Vec<ReadingCrossReferenceAssertion>,
    /// Other installed editions that print it.
    pub available_in: Vec<String>,
    /// The attribution the licence asks for.
    pub attribution: String,
}

/// The answer for one passage (`CrossReferenceList`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReadingCrossReferenceList {
    /// The edition.
    pub resource_id: String,
    /// The passage, OSIS, in the edition.
    pub reference: String,
    /// Its verses in the source's scheme.
    pub source_verses: Vec<String>,
    /// How the passage maps into the source's scheme.
    pub mapping: MappingStatus,
    /// The page asked for.
    pub items: Vec<ReadingCrossReference>,
    /// Items in all.
    pub total: u64,
    /// Published items in all.
    pub published: u64,
    /// The source.
    pub source: ReadingCrossReferenceSource,
}

impl From<core::CrossReferenceList> for ReadingCrossReferenceList {
    fn from(l: core::CrossReferenceList) -> Self {
        ReadingCrossReferenceList {
            resource_id: l.resource_id,
            reference: l.reference,
            source_verses: l.source_verses,
            mapping: l.mapping.into(),
            items: l
                .items
                .into_iter()
                .map(|i| ReadingCrossReference {
                    target: i.target,
                    label: i.label,
                    source_target: i.source_target,
                    source_label: i.source_label,
                    mapping: i.mapping.into(),
                    availability: i.availability.into(),
                    preview: i.preview,
                    preview_truncated: i.preview_truncated,
                    preview_chunk: i.preview_chunk.map(Into::into),
                    relation: i.relation,
                    kind: i.kind,
                    direction: i.direction.into(),
                    votes: i.votes,
                    assertions: i
                        .assertions
                        .into_iter()
                        .map(|a| ReadingCrossReferenceAssertion {
                            from: a.from,
                            to: a.to,
                            votes: a.votes,
                            line: a.line,
                            direction: a.direction.into(),
                        })
                        .collect(),
                    available_in: i.available_in,
                    attribution: i.attribution,
                })
                .collect(),
            total: l.total as u64,
            published: l.published as u64,
            source: l.source.into(),
        }
    }
}

impl From<CrossReferenceError> for ReadingCallError {
    fn from(error: CrossReferenceError) -> Self {
        match error {
            CrossReferenceError::Reading { error } => error.into(),
            other => ReadingCallError::Refused {
                kind: serde_json::to_value(&other)
                    .ok()
                    .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_owned))
                    .unwrap_or_default(),
                reason: other.to_string(),
            },
        }
    }
}

#[uniffi::export]
impl ReadingHandle {
    /// The cross references of a passage in an edition (`John 3:16`, `Ps 23`), published
    /// items first, best first by their votes. `include_reverse` adds links listed under
    /// other verses that point at the passage, labelled `reverse_derived`. `offset` and
    /// `limit` page the list (25 fills a panel); preview text, `preview_verses` verses per
    /// item, is read only for the page. Fails with `not_installed`, `too_long`, `source` or a
    /// reading error's kind.
    pub fn cross_references(
        &self,
        resource_id: String,
        input: String,
        include_reverse: bool,
        offset: u64,
        limit: Option<u64>,
        preview_verses: u32,
    ) -> Result<ReadingCrossReferenceList, ReadingCallError> {
        let options = CrossReferenceOptions {
            include_reverse,
            offset: usize::try_from(offset).unwrap_or(usize::MAX),
            limit: limit.map(|l| usize::try_from(l).unwrap_or(usize::MAX)),
            preview_verses: preview_verses as usize,
        };
        Ok(self
            .lock()
            .cross_references(&resource_id, &input, &options)?
            .into())
    }

    /// The installed cross-reference source, if any.
    pub fn cross_reference_source(&self) -> Option<ReadingCrossReferenceSource> {
        self.lock().cross_reference_info().cloned().map(Into::into)
    }

    /// Removes the installed cross-reference source.
    pub fn clear_cross_references(&self) {
        self.lock().clear_cross_references();
    }
}

#[cfg(feature = "retrieval")]
#[uniffi::export]
impl ReadingHandle {
    /// Opens a cross-reference pack file (`relations/openbible-xrefs.sqlite`), read-only,
    /// replacing any source opened before.
    pub fn open_cross_references(
        &self,
        path: String,
    ) -> Result<ReadingCrossReferenceSource, ReadingCallError> {
        let pack =
            openlogos_retrieval_sqlite::CrossReferencePack::open(std::path::Path::new(&path))
                .map_err(|detail| CrossReferenceError::Source { detail })?;
        let info = core::CrossReferenceSource::info(&pack).clone();
        self.lock().set_cross_references(Box::new(pack));
        Ok(info.into())
    }
}

#[uniffi::export]
impl ShellHandle {
    /// Registers the data layers installed beside the editions, replacing any registered
    /// before: `cross_references` once `ReadingHandle.open_cross_references` succeeded enables
    /// the "Cross references" row and palette command (B057).
    pub fn set_data_layers(&self, layers: Vec<String>) {
        self.lock()
            .shell
            .set_data_layers(layers.into_iter().collect());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlogos_core::canon;
    use openlogos_text::reading::{CrossReferenceRow, MemoryCrossReferences};

    fn handle() -> std::sync::Arc<ReadingHandle> {
        let handle = ReadingHandle::placeholder();
        let r = |s: &str| canon::segment_reference(s).unwrap();
        let info = CrossReferenceInfo {
            source_id: "S09".into(),
            edition_id: "test".into(),
            scheme: "eng".into(),
            license: "CC-BY-4.0".into(),
            attribution: "Cross references from a test".into(),
            relation: "cross_reference".into(),
            kind: "unspecified".into(),
            votes_meaning: "votes".into(),
            review_state: "source_attested".into(),
            coverage: "test".into(),
            rows: 2,
        };
        let rows = vec![
            CrossReferenceRow {
                line: 2,
                from: r("John.3.16"),
                to_start: r("Rom.5.8"),
                to_end: r("Rom.5.8"),
                votes: 991,
            },
            CrossReferenceRow {
                line: 3,
                from: r("John.3.16"),
                to_start: r("Gen.22.2"),
                to_end: r("Gen.22.2"),
                votes: 0,
            },
        ];
        handle
            .lock()
            .set_cross_references(Box::new(MemoryCrossReferences::new(info, rows)));
        handle
    }

    #[test]
    fn the_handle_lists_pages_and_says_what_is_missing() {
        let handle = handle();
        assert_eq!(handle.cross_reference_source().unwrap().source_id, "S09");
        let list = handle
            .cross_references("sblgnt".into(), "John 3:16".into(), false, 0, None, 1)
            .unwrap();
        assert_eq!(list.total, 2);
        assert_eq!(list.items[0].label, "Rom 5:8");
        assert_eq!(
            list.items[0].availability,
            ReadingCrossReferenceAvailability::Printed
        );
        assert_eq!(
            list.items[0].direction,
            ReadingCrossReferenceDirection::AsPublished
        );
        assert_eq!(
            list.items[1].availability,
            ReadingCrossReferenceAvailability::BookNotInEdition
        );
        assert_eq!(list.items[1].available_in, ["bsb", "wlc"]);
        let page = handle
            .cross_references("sblgnt".into(), "John 3:16".into(), false, 1, Some(1), 1)
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].source_target, "Gen.22.2");

        let error = handle
            .cross_references("sblgnt".into(), "Gen 1:1".into(), false, 0, None, 1)
            .unwrap_err();
        let ReadingCallError::Refused { kind, .. } = error;
        assert_eq!(kind, "book_not_in_edition");
        handle.clear_cross_references();
        let ReadingCallError::Refused { kind, reason } = handle
            .cross_references("bsb".into(), "John 3:16".into(), false, 0, None, 1)
            .unwrap_err();
        assert_eq!(kind, "not_installed");
        assert_eq!(reason, "Needs cross-reference data");
    }

    #[cfg(feature = "retrieval")]
    #[test]
    fn a_pack_file_opens_and_answers() {
        let r = |s: &str| canon::segment_reference(s).unwrap();
        let path = std::env::temp_dir().join(format!("b057-ffi-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let info = CrossReferenceInfo {
            source_id: "S09".into(),
            edition_id: "test".into(),
            scheme: "eng".into(),
            license: "CC-BY-4.0".into(),
            attribution: "Cross references from a test".into(),
            relation: "cross_reference".into(),
            kind: "unspecified".into(),
            votes_meaning: "votes".into(),
            review_state: "source_attested".into(),
            coverage: "test".into(),
            rows: 1,
        };
        let manifest = openlogos_retrieval_sqlite::PackManifest {
            edition: "xref-test".into(),
            scheme: "eng".into(),
            source: "hand-written".into(),
            license: "CC-BY-4.0".into(),
            attribution: "Cross references from a test".into(),
            work: None,
            locator_scheme: None,
        };
        let rows = vec![CrossReferenceRow {
            line: 2,
            from: r("Mal.4.5"),
            to_start: r("Mal.4.6"),
            to_end: r("Mal.4.6"),
            votes: 5,
        }];
        openlogos_retrieval_sqlite::write_cross_reference_pack(&path, &manifest, &info, &[], &rows)
            .unwrap();
        let handle = ReadingHandle::placeholder();
        let source = handle
            .open_cross_references(path.display().to_string())
            .unwrap();
        assert_eq!(source.attribution, "Cross references from a test");
        let list = handle
            .cross_references("wlc".into(), "Mal 3:23".into(), false, 0, None, 1)
            .unwrap();
        assert_eq!(list.items[0].target.as_deref(), Some("Mal.3.24"));
        assert_eq!(list.items[0].source_label, "Mal 4:6");
        assert!(
            handle
                .open_cross_references("/nonexistent/x.sqlite".into())
                .is_err()
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn the_shell_enables_the_row_when_the_layer_is_registered() {
        let shell = ShellHandle::fixture();
        let disabled = |shell: &ShellHandle| {
            shell
                .commands(false)
                .into_iter()
                .find(|c| c.id == "nav.cross_references")
                .unwrap()
                .disabled_reason
        };
        assert_eq!(
            disabled(&shell).as_deref(),
            Some("Needs cross-reference data")
        );
        shell.set_data_layers(vec!["cross_references".into()]);
        assert_eq!(disabled(&shell), None);
    }
}
