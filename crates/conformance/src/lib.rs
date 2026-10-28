//! The shared conformance suite of docs/adr/0001-rust-core-native-ui.md. Small licensed or
//! synthetic fixtures (a Hebrew passage with cantillation and maqqef, a cross-verse selection,
//! a note with bidi isolation, Psalm 9/10 numbering, reference parsing) and their expected
//! anchors, offsets and mappings live as JSON under `fixtures/`, compiled in with
//! `include_str!` so they are available in WebAssembly and can be handed to Swift, Kotlin
//! and JavaScript hosts through [`fixtures_json`]. [`run_all`] runs every case through the
//! core and text crates and reports each expected and actual value, so every binding can
//! call one function and compare, asserting that a selection produces the same anchor and a
//! highlight paints the same scalar range on iPad, Mac, phone, web and Android.

use openlogos_core::anchor::{
    Anchor, ReanchorResult, Revision, ScalarRange, Segment, TokenId, TokenSpan, reanchor,
    split_selection,
};
use openlogos_core::reference::{AliasTable, ReferenceSet, parse};
use openlogos_core::versification::{
    MappingStatus, ReferenceMapping, SchemeId, SegmentSpan, SegmentTable, VerseMap, VerseMapEntry,
    VerseMapKind,
};
use openlogos_text::{
    GraphemeRange, OffsetError, Utf16Range, grapheme_len, grapheme_to_scalar, scalar_len,
    scalar_to_grapheme, scalar_to_utf16, slice_scalars, snap_to_graphemes, utf16_len,
    utf16_to_scalar,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod contracts;
pub mod identity;
pub mod layout;

pub use contracts::{
    AuthorityScenario, AuthorityStep, MigrationCase, MigrationLayout, NoteAuthorityFixture,
    WorkspaceMigrationFixture,
};

pub use layout::{ExpectedLayout, LayoutCase, PaneState, WorkspaceLayoutFixture};

pub use identity::{
    AlignedSegment, AlignmentFixture, ExpectedReanchor, FixtureAlignmentRecord, FixtureLineage,
    FixtureLineageEntry, FixtureRevision, InvalidAlignmentCase, InvalidLineageCase, LineageCase,
    LineageFixture, ReanchorCase, ReanchorFixture, RenderingCase, ReverseCase, RevisionPair,
    StaleCase,
};

/// Every fixture file, by id, as compiled in.
pub const FIXTURE_FILES: [(&str, &str); 13] = [
    (
        "hebrew-cantillation-genesis-1",
        include_str!("../fixtures/hebrew-cantillation-genesis-1.json"),
    ),
    (
        "cross-verse-selection-john-3",
        include_str!("../fixtures/cross-verse-selection-john-3.json"),
    ),
    (
        "bidi-isolation-note",
        include_str!("../fixtures/bidi-isolation-note.json"),
    ),
    (
        "psalm-9-10-numbering",
        include_str!("../fixtures/psalm-9-10-numbering.json"),
    ),
    (
        "reference-parsing-english",
        include_str!("../fixtures/reference-parsing-english.json"),
    ),
    (
        "joel-malachi-chapter-numbering",
        include_str!("../fixtures/joel-malachi-chapter-numbering.json"),
    ),
    (
        "omitted-and-bridged-verses",
        include_str!("../fixtures/omitted-and-bridged-verses.json"),
    ),
    (
        "token-lineage-genesis-1",
        include_str!("../fixtures/token-lineage-genesis-1.json"),
    ),
    (
        "alignment-john-3-16-sblgnt-bsb",
        include_str!("../fixtures/alignment-john-3-16-sblgnt-bsb.json"),
    ),
    (
        "edition-revision-reanchor",
        include_str!("../fixtures/edition-revision-reanchor.json"),
    ),
    (
        "workspace-layout-five-panes",
        include_str!("../fixtures/workspace-layout-five-panes.json"),
    ),
    (
        "workspace-migration-1-0-to-1-1",
        include_str!("../fixtures/workspace-migration-1-0-to-1-1.json"),
    ),
    (
        "note-authority-transitions",
        include_str!("../fixtures/note-authority-transitions.json"),
    ),
];

/// All fixtures as one JSON array, for hosts that want to read or display them.
#[must_use]
pub fn fixtures_json() -> &'static str {
    concat!(
        "[",
        include_str!("../fixtures/hebrew-cantillation-genesis-1.json"),
        ",",
        include_str!("../fixtures/cross-verse-selection-john-3.json"),
        ",",
        include_str!("../fixtures/bidi-isolation-note.json"),
        ",",
        include_str!("../fixtures/psalm-9-10-numbering.json"),
        ",",
        include_str!("../fixtures/reference-parsing-english.json"),
        ",",
        include_str!("../fixtures/joel-malachi-chapter-numbering.json"),
        ",",
        include_str!("../fixtures/omitted-and-bridged-verses.json"),
        ",",
        include_str!("../fixtures/token-lineage-genesis-1.json"),
        ",",
        include_str!("../fixtures/alignment-john-3-16-sblgnt-bsb.json"),
        ",",
        include_str!("../fixtures/edition-revision-reanchor.json"),
        ",",
        include_str!("../fixtures/workspace-layout-five-panes.json"),
        ",",
        include_str!("../fixtures/workspace-migration-1-0-to-1-1.json"),
        ",",
        include_str!("../fixtures/note-authority-transitions.json"),
        "]"
    )
}

/// A half-open range in whatever unit the field name says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// First unit of the range.
    pub start: u32,
    /// One past the last unit of the range.
    pub end: u32,
}

impl From<Span> for ScalarRange {
    fn from(span: Span) -> ScalarRange {
        ScalarRange {
            start: span.start,
            end: span.end,
        }
    }
}

impl From<Span> for Utf16Range {
    fn from(span: Span) -> Utf16Range {
        Utf16Range {
            start: span.start,
            end: span.end,
        }
    }
}

impl From<Span> for GraphemeRange {
    fn from(span: Span) -> GraphemeRange {
        GraphemeRange {
            start: span.start,
            end: span.end,
        }
    }
}

/// A token and its extent in a fixture segment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureToken {
    /// Token id in `edition:BBCCCVVVWWW[s]` form.
    pub id: String,
    /// Start in scalar values.
    pub start: u32,
    /// End in scalar values, exclusive.
    pub end: u32,
    /// The surface text the range must slice to, when the fixture states it.
    #[serde(default)]
    pub surface: Option<String>,
}

/// A segment of text with its expected lengths and token layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureSegment {
    /// The segment id.
    pub segment_id: String,
    /// The text, byte-exact.
    pub text: String,
    /// Expected length in scalar values.
    pub scalar_len: u32,
    /// Expected length in UTF-16 code units.
    pub utf16_len: u32,
    /// Expected length in grapheme clusters.
    pub grapheme_len: u32,
    /// Tokens in text order.
    pub tokens: Vec<FixtureToken>,
}

/// A selection within one segment and what every adapter must say about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureSelection {
    /// What the selection is.
    pub name: String,
    /// The segment selected in.
    pub segment_id: String,
    /// The selection in scalar values.
    pub scalar: Span,
    /// The same selection in UTF-16 code units.
    pub utf16: Span,
    /// The same selection in grapheme clusters, or `None` when it does not sit on cluster
    /// boundaries (then `snapped` says where it widens to).
    pub grapheme: Option<Span>,
    /// The scalar range after widening to cluster boundaries, when `grapheme` is `None`.
    #[serde(default)]
    pub snapped: Option<Span>,
    /// The selected text.
    pub exact: String,
    /// Ids of the tokens under the selection.
    pub tokens: Vec<String>,
    /// Free-form note for readers of the fixture.
    #[serde(default)]
    pub note: Option<String>,
}

/// One per-segment anchor expected from a paragraph selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedAnchor {
    /// The segment.
    pub segment_id: String,
    /// The range local to the segment, in scalar values.
    pub scalar: Span,
    /// The selected text within the segment.
    pub exact: String,
    /// Ids of the tokens under the selection.
    pub tokens: Vec<String>,
}

/// A selection over several segments rendered as one paragraph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParagraphSelection {
    /// What the selection is.
    pub name: String,
    /// Text placed between segments in the paragraph.
    pub joiner: String,
    /// Segment ids in paragraph order.
    pub segments: Vec<String>,
    /// The selection in UTF-16 units over the paragraph (what a platform text view reports).
    pub utf16: Span,
    /// The same selection in scalar values over the paragraph.
    pub scalar: Span,
    /// The same selection in grapheme clusters over the paragraph.
    pub grapheme: Span,
    /// The selected paragraph text.
    pub exact: String,
    /// The anchors the selection must split into.
    pub anchors: Vec<ExpectedAnchor>,
}

/// A text fixture: segments, selections and paragraph selections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextFixture {
    /// Fixture id (also the file name).
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License of the text.
    pub license: String,
    /// Where the text came from.
    pub source: String,
    /// Edition id used for anchors.
    pub edition_id: String,
    /// Content revision used for anchors.
    pub content_revision: String,
    /// The segments.
    pub segments: Vec<FixtureSegment>,
    /// Selections within single segments.
    pub selections: Vec<FixtureSelection>,
    /// Selections across segments.
    #[serde(default)]
    pub paragraph_selections: Vec<ParagraphSelection>,
}

/// A verse map row, with `from` and `to` as OSIS references or same-length ranges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureRow {
    /// Scheme of `from`.
    pub from_scheme: String,
    /// Source reference or range, e.g. `Ps.9.1-Ps.9.20`.
    pub from: String,
    /// Scheme of `to`.
    pub to_scheme: String,
    /// Target reference or range; `None` for an absent verse.
    pub to: Option<String>,
    /// The relation.
    pub kind: VerseMapKind,
    /// Where the row comes from.
    pub source: String,
}

/// One expected mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingCase {
    /// What the case shows.
    pub name: String,
    /// The reference to map, in OSIS form.
    pub reference: String,
    /// Source scheme.
    pub from: String,
    /// Target scheme.
    pub to: String,
    /// Expected status.
    pub status: MappingStatus,
    /// Expected targets in OSIS form.
    pub targets: Vec<String>,
    /// Expected schemes traversed.
    pub path: Vec<String>,
}

/// One expected set mapping (`VerseMap::map_many`), with references or same-chapter ranges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetMappingCase {
    /// What the case shows.
    pub name: String,
    /// The source verses, as references or ranges.
    pub references: Vec<String>,
    /// Source scheme.
    pub from: String,
    /// Target scheme.
    pub to: String,
    /// Expected status.
    pub status: MappingStatus,
    /// Expected target verses, as references or ranges.
    pub to_references: Vec<String>,
    /// Expected absent source verses.
    #[serde(default)]
    pub absent: Vec<String>,
    /// Expected unknown source verses.
    #[serde(default)]
    pub unknown: Vec<String>,
    /// Expected schemes traversed.
    pub path: Vec<String>,
}

/// One printed segment of an edition, with its verses in OSIS form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureSegmentSpan {
    /// Segment id.
    pub segment_id: String,
    /// The printed label.
    pub native_label: String,
    /// The verses it holds.
    pub verses: Vec<String>,
}

/// An edition's segment table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureSegmentTable {
    /// The edition.
    pub edition_id: String,
    /// Its scheme.
    pub scheme: String,
    /// The segments.
    pub segments: Vec<FixtureSegmentSpan>,
}

/// One expected segment resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentCase {
    /// What the case shows.
    pub name: String,
    /// The edition whose table to resolve in.
    pub edition_id: String,
    /// The verse, in OSIS form.
    pub reference: String,
    /// Expected status.
    pub status: MappingStatus,
    /// Expected segment ids.
    pub segment_ids: Vec<String>,
    /// Expected native labels.
    pub native_labels: Vec<String>,
    /// Expected bridge flag.
    pub bridged: bool,
}

/// A versification fixture: rows and the mappings they must produce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersificationFixture {
    /// Fixture id.
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License.
    pub license: String,
    /// Where the numbering facts come from, in general; each row has its own note.
    pub source: String,
    /// The rows.
    pub rows: Vec<FixtureRow>,
    /// The expected mappings.
    pub cases: Vec<MappingCase>,
    /// Expected set mappings.
    #[serde(default)]
    pub set_cases: Vec<SetMappingCase>,
    /// Segment tables of editions.
    #[serde(default)]
    pub segment_tables: Vec<FixtureSegmentTable>,
    /// Expected segment resolutions.
    #[serde(default)]
    pub segment_cases: Vec<SegmentCase>,
}

impl VersificationFixture {
    /// The rows expanded verse by verse into a [`VerseMap`], for a host that wants to map
    /// with the fixture table itself. The first row that does not expand is the error;
    /// [`run_all`] reports every such row as a failing case instead.
    pub fn verse_map(&self) -> Result<VerseMap, String> {
        let table = AliasTable::english();
        let mut entries = Vec::new();
        for row in &self.rows {
            entries.extend(expand_row(row, &table)?);
        }
        Ok(VerseMap::new(entries))
    }
}

/// One citation and its expected parse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceCase {
    /// The citation as a person would write it.
    pub input: String,
    /// Expected canonical form.
    pub osis: String,
    /// Expected human form.
    pub human: String,
    /// Expected review flags, as the core serializes them.
    pub flags: Value,
}

/// One citation that must be rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceErrorCase {
    /// The input.
    pub input: String,
    /// Expected error kind, as the core serializes it.
    pub error: String,
}

/// A reference-parsing fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceFixture {
    /// Fixture id.
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License.
    pub license: String,
    /// Source.
    pub source: String,
    /// Alias table to parse with; only `english` exists.
    pub alias_table: String,
    /// Citations that parse.
    pub cases: Vec<ReferenceCase>,
    /// Citations that fail.
    pub errors: Vec<ReferenceErrorCase>,
}

/// One fixture file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Fixture {
    /// Offsets, selections and anchors over text.
    Text(TextFixture),
    /// Verse map rows and mappings.
    Versification(VersificationFixture),
    /// Citation parsing.
    References(ReferenceFixture),
    /// `id_lineage` tables between content revisions.
    Lineage(LineageFixture),
    /// Standoff alignment records.
    Alignment(Box<AlignmentFixture>),
    /// Anchors carried across a content revision.
    Reanchor(ReanchorFixture),
    /// A workspace opened on every device class (B062 layout profiles).
    WorkspaceLayout(Box<WorkspaceLayoutFixture>),
    /// A workspace document migrated between schema versions (B040, ADR 0003).
    WorkspaceMigration(WorkspaceMigrationFixture),
    /// Note bindings taken through the authority state machine (B040).
    NoteAuthority(Box<NoteAuthorityFixture>),
}

impl Fixture {
    /// The fixture id.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Fixture::Text(f) => &f.id,
            Fixture::Versification(f) => &f.id,
            Fixture::References(f) => &f.id,
            Fixture::Lineage(f) => &f.id,
            Fixture::Alignment(f) => &f.id,
            Fixture::Reanchor(f) => &f.id,
            Fixture::WorkspaceLayout(f) => &f.id,
            Fixture::WorkspaceMigration(f) => &f.id,
            Fixture::NoteAuthority(f) => &f.id,
        }
    }
}

/// Every fixture, parsed. A file that does not parse is left out; [`run_all`] reports it.
#[must_use]
pub fn fixtures() -> Vec<Fixture> {
    FIXTURE_FILES
        .iter()
        .filter_map(|(_, json)| serde_json::from_str(json).ok())
        .collect()
}

/// One checked value: what the fixture expects and what the core produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseResult {
    /// Fixture id.
    pub fixture: String,
    /// Case name, unique within the fixture.
    pub name: String,
    /// Whether `actual` equals `expected`.
    pub passed: bool,
    /// Expected value as JSON.
    pub expected: String,
    /// Actual value as JSON.
    pub actual: String,
}

/// The outcome of [`run_all`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceReport {
    /// [`openlogos_core::API_VERSION`] the suite ran against.
    pub api_version: String,
    /// Number of passing cases.
    pub passed: u32,
    /// Number of failing cases.
    pub failed: u32,
    /// Every case.
    pub cases: Vec<CaseResult>,
}

impl ConformanceReport {
    /// Whether every case passed.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.failed == 0 && self.passed > 0
    }

    /// The failing cases.
    #[must_use]
    pub fn failures(&self) -> Vec<&CaseResult> {
        self.cases.iter().filter(|c| !c.passed).collect()
    }
}

/// Collects case results for one fixture.
struct Recorder {
    fixture: String,
    cases: Vec<CaseResult>,
}

impl Recorder {
    fn check<T: Serialize + PartialEq>(&mut self, name: &str, expected: &T, actual: &T) {
        let to_json =
            |v: &T| serde_json::to_string(v).unwrap_or_else(|e| format!("unserializable: {e}"));
        self.cases.push(CaseResult {
            fixture: self.fixture.clone(),
            name: name.to_owned(),
            passed: expected == actual,
            expected: to_json(expected),
            actual: to_json(actual),
        });
    }

    fn fail(&mut self, name: &str, expected: &str, actual: &str) {
        self.cases.push(CaseResult {
            fixture: self.fixture.clone(),
            name: name.to_owned(),
            passed: false,
            expected: expected.to_owned(),
            actual: actual.to_owned(),
        });
    }
}

/// Runs every case of every fixture and reports expected and actual values.
#[must_use]
pub fn run_all() -> ConformanceReport {
    let mut cases = Vec::new();
    for (id, json) in FIXTURE_FILES {
        let mut recorder = Recorder {
            fixture: id.to_owned(),
            cases: Vec::new(),
        };
        match serde_json::from_str::<Fixture>(json) {
            Ok(Fixture::Text(fixture)) => run_text(&fixture, &mut recorder),
            Ok(Fixture::Versification(fixture)) => run_versification(&fixture, &mut recorder),
            Ok(Fixture::References(fixture)) => run_references(&fixture, &mut recorder),
            Ok(Fixture::Lineage(fixture)) => identity::run_lineage(&fixture, &mut recorder),
            Ok(Fixture::Alignment(fixture)) => identity::run_alignment(&fixture, &mut recorder),
            Ok(Fixture::Reanchor(fixture)) => identity::run_reanchor(&fixture, &mut recorder),
            Ok(Fixture::WorkspaceLayout(fixture)) => {
                layout::run_workspace_layout(&fixture, &mut recorder);
            }
            Ok(Fixture::WorkspaceMigration(fixture)) => {
                contracts::run_workspace_migration(&fixture, &mut recorder);
            }
            Ok(Fixture::NoteAuthority(fixture)) => {
                contracts::run_note_authority(&fixture, &mut recorder);
            }
            Err(error) => recorder.fail(
                "fixture parses",
                "a fixture of a known kind",
                &error.to_string(),
            ),
        }
        cases.extend(recorder.cases);
    }
    let passed = cases.iter().filter(|c| c.passed).count() as u32;
    let failed = cases.len() as u32 - passed;
    ConformanceReport {
        api_version: openlogos_core::api_version().to_owned(),
        passed,
        failed,
        cases,
    }
}

/// Lengths of a text in the three units, for one comparison.
#[derive(Serialize, PartialEq)]
struct Lengths {
    scalar: u32,
    utf16: u32,
    grapheme: u32,
}

/// The anchor fields a fixture pins down.
#[derive(Serialize, PartialEq)]
struct AnchorShape {
    segment_id: String,
    scalar: Span,
    exact: Option<String>,
    tokens: Vec<String>,
}

impl From<&Anchor> for AnchorShape {
    fn from(anchor: &Anchor) -> AnchorShape {
        AnchorShape {
            segment_id: anchor.segment_id.clone(),
            scalar: Span {
                start: anchor.range.start,
                end: anchor.range.end,
            },
            exact: anchor.quote.as_ref().map(|q| q.exact.clone()),
            tokens: anchor.tokens.iter().map(ToString::to_string).collect(),
        }
    }
}

fn to_segment(segment: &FixtureSegment) -> Result<Segment, String> {
    let tokens = segment
        .tokens
        .iter()
        .map(|t| {
            TokenId::parse(&t.id).map(|id| TokenSpan {
                id,
                range: ScalarRange {
                    start: t.start,
                    end: t.end,
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(Segment {
        segment_id: segment.segment_id.clone(),
        text: segment.text.clone(),
        tokens,
    })
}

fn run_text(fixture: &TextFixture, r: &mut Recorder) {
    for segment in &fixture.segments {
        r.check(
            &format!("{}: lengths", segment.segment_id),
            &Lengths {
                scalar: segment.scalar_len,
                utf16: segment.utf16_len,
                grapheme: segment.grapheme_len,
            },
            &Lengths {
                scalar: scalar_len(&segment.text),
                utf16: utf16_len(&segment.text),
                grapheme: grapheme_len(&segment.text),
            },
        );
        for token in &segment.tokens {
            r.check(
                &format!(
                    "{}: token {} parses and formats",
                    segment.segment_id, token.id
                ),
                &Some(token.id.clone()),
                &TokenId::parse(&token.id).ok().map(|id| id.to_string()),
            );
        }
    }

    for selection in &fixture.selections {
        let name = |what: &str| format!("{}: {what}", selection.name);
        let Some(segment) = fixture
            .segments
            .iter()
            .find(|s| s.segment_id == selection.segment_id)
        else {
            r.fail(
                &name("segment exists"),
                &selection.segment_id,
                "no such segment",
            );
            continue;
        };
        let text = &segment.text;
        let scalar: ScalarRange = selection.scalar.into();
        let utf16: Utf16Range = selection.utf16.into();

        r.check(
            &name("utf16 to scalar"),
            &Ok(scalar),
            &utf16_to_scalar(text, utf16),
        );
        r.check(
            &name("scalar to utf16"),
            &Ok(utf16),
            &scalar_to_utf16(text, scalar),
        );
        r.check(
            &name("exact text"),
            &Ok(selection.exact.clone()),
            &slice_scalars(text, scalar),
        );

        match selection.grapheme {
            Some(grapheme) => {
                let grapheme: GraphemeRange = grapheme.into();
                r.check(
                    &name("scalar to grapheme"),
                    &Ok(grapheme),
                    &scalar_to_grapheme(text, scalar),
                );
                r.check(
                    &name("grapheme to scalar"),
                    &Ok(scalar),
                    &grapheme_to_scalar(text, grapheme),
                );
                r.check(
                    &name("snapping leaves a cluster-aligned range alone"),
                    &Ok(scalar),
                    &snap_to_graphemes(text, scalar),
                );
            }
            None => {
                let actual = scalar_to_grapheme(text, scalar);
                r.check(
                    &name("scalar inside a cluster is rejected"),
                    &true,
                    &matches!(actual, Err(OffsetError::NotGraphemeBoundary { .. })),
                );
                if let Some(snapped) = selection.snapped {
                    r.check(
                        &name("snaps outward to cluster boundaries"),
                        &Ok(snapped.into()),
                        &snap_to_graphemes(text, scalar),
                    );
                }
            }
        }

        let core_segment = match to_segment(segment) {
            Ok(s) => s,
            Err(e) => {
                r.fail(
                    &name("segment tokens parse"),
                    "token ids in edition:BBCCCVVVWWW form",
                    &e,
                );
                continue;
            }
        };
        let anchors = split_selection(
            &fixture.edition_id,
            &fixture.content_revision,
            std::slice::from_ref(&core_segment),
            "",
            scalar,
        );
        let expected = AnchorShape {
            segment_id: selection.segment_id.clone(),
            scalar: selection.scalar,
            exact: Some(selection.exact.clone()),
            tokens: selection.tokens.clone(),
        };
        match anchors.as_deref() {
            Ok([anchor]) => {
                r.check(
                    &name("anchor from the selection"),
                    &expected,
                    &AnchorShape::from(anchor),
                );
                // The round trip a renderer makes: UTF-16 selection -> anchor -> UTF-16 highlight.
                r.check(
                    &name("utf16 selection round-trips through the anchor"),
                    &Ok(utf16),
                    &utf16_to_scalar(text, utf16).and_then(|range| scalar_to_utf16(text, range)),
                );
                let revision = Revision {
                    content_revision: fixture.content_revision.clone(),
                    text: text.clone(),
                    tokens: core_segment.tokens.clone(),
                };
                let found = match reanchor(anchor, &revision, &revision) {
                    ReanchorResult::Exact(found) => Some(AnchorShape::from(&found)),
                    _ => None,
                };
                r.check(
                    &name("reanchoring onto the same revision is exact"),
                    &Some(expected),
                    &found,
                );
            }
            other => r.fail(
                &name("anchor from the selection"),
                "exactly one anchor",
                &format!("{other:?}"),
            ),
        }
    }

    for paragraph in &fixture.paragraph_selections {
        let name = |what: &str| format!("{}: {what}", paragraph.name);
        let segments: Vec<&FixtureSegment> = paragraph
            .segments
            .iter()
            .filter_map(|id| fixture.segments.iter().find(|s| &s.segment_id == id))
            .collect();
        if segments.len() != paragraph.segments.len() {
            r.fail(
                &name("segments exist"),
                &paragraph.segments.join(","),
                "missing segment",
            );
            continue;
        }
        let text = segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(&paragraph.joiner);
        let scalar: ScalarRange = paragraph.scalar.into();
        let utf16: Utf16Range = paragraph.utf16.into();
        r.check(
            &name("utf16 to scalar over the paragraph"),
            &Ok(scalar),
            &utf16_to_scalar(&text, utf16),
        );
        r.check(
            &name("scalar to utf16 over the paragraph"),
            &Ok(utf16),
            &scalar_to_utf16(&text, scalar),
        );
        r.check(
            &name("scalar to grapheme over the paragraph"),
            &Ok(paragraph.grapheme.into()),
            &scalar_to_grapheme(&text, scalar),
        );
        r.check(
            &name("exact paragraph text"),
            &Ok(paragraph.exact.clone()),
            &slice_scalars(&text, scalar),
        );
        let core_segments: Result<Vec<Segment>, String> =
            segments.iter().map(|s| to_segment(s)).collect();
        match core_segments {
            Ok(core_segments) => {
                let expected: Vec<AnchorShape> = paragraph
                    .anchors
                    .iter()
                    .map(|a| AnchorShape {
                        segment_id: a.segment_id.clone(),
                        scalar: a.scalar,
                        exact: Some(a.exact.clone()),
                        tokens: a.tokens.clone(),
                    })
                    .collect();
                let actual = split_selection(
                    &fixture.edition_id,
                    &fixture.content_revision,
                    &core_segments,
                    &paragraph.joiner,
                    scalar,
                )
                .map(|anchors| anchors.iter().map(AnchorShape::from).collect::<Vec<_>>())
                .map_err(|e| e.to_string());
                r.check(
                    &name("splits into one anchor per segment"),
                    &Ok(expected),
                    &actual,
                );
            }
            Err(e) => r.fail(
                &name("segment tokens parse"),
                "token ids in edition:BBCCCVVVWWW form",
                &e,
            ),
        }
    }
}

/// Expands a fixture row into verse-by-verse map entries.
fn expand_row(row: &FixtureRow, table: &AliasTable) -> Result<Vec<VerseMapEntry>, String> {
    let from = single_range(&row.from, table)?;
    let make = |from, to| VerseMapEntry {
        from_scheme: SchemeId::new(&row.from_scheme),
        from,
        to_scheme: SchemeId::new(&row.to_scheme),
        to,
        kind: row.kind,
        source: row.source.clone(),
    };
    let Some(to) = &row.to else {
        // An absent row may cover a range: every verse of it is absent.
        return Ok(from.into_iter().map(|f| make(f, None)).collect());
    };
    let to = single_range(to, table)?;
    if from.len() != to.len() {
        return Err(format!(
            "row `{}` -> `{}` has {} and {} verses",
            row.from,
            to.len(),
            from.len(),
            to.len()
        ));
    }
    Ok(from
        .into_iter()
        .zip(to)
        .map(|(f, t)| make(f, Some(t)))
        .collect())
}

/// The verses of one reference or one same-chapter range, in order.
fn single_range(
    text: &str,
    table: &AliasTable,
) -> Result<Vec<openlogos_core::reference::Reference>, String> {
    let set: ReferenceSet = parse(text, table).map_err(|e| e.to_string())?;
    let [range] = set.ranges.as_slice() else {
        return Err(format!("`{text}` must be one reference or range"));
    };
    let (Some(first), Some(last)) = (range.start.verse, range.end.verse) else {
        return Err(format!("`{text}` must name verses"));
    };
    if range.start.book != range.end.book || range.start.chapter != range.end.chapter {
        return Err(format!("`{text}` must stay within one chapter"));
    }
    Ok((first..=last)
        .map(|verse| {
            openlogos_core::reference::Reference::verse(
                range.start.book,
                range.start.chapter,
                verse,
            )
        })
        .collect())
}

/// The verse map that `rows` (the shape of a versification fixture's `rows` array) describe,
/// with every same-chapter range expanded verse by verse. This is how a binding or a host
/// loads the Psalm 9/10 table, or its own rows in the same JSON shape, into a [`VerseMap`];
/// the error names the first row that is not a verse or a same-length pair of ranges.
pub fn verse_map_from_rows(rows: &[FixtureRow]) -> Result<VerseMap, String> {
    let table = AliasTable::english();
    let mut entries = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let expanded =
            expand_row(row, &table).map_err(|e| format!("row {index} `{}`: {e}", row.from))?;
        entries.extend(expanded);
    }
    Ok(VerseMap::new(entries))
}

/// The rows of every versification fixture, in file order: the table [`run_all`] maps with.
#[must_use]
pub fn fixture_rows() -> Vec<FixtureRow> {
    fixtures()
        .into_iter()
        .filter_map(|fixture| match fixture {
            Fixture::Versification(f) => Some(f.rows),
            _ => None,
        })
        .flatten()
        .collect()
}

/// Status, targets and path of a mapping, for one comparison.
#[derive(Serialize, PartialEq)]
struct MappingShape {
    status: MappingStatus,
    targets: Vec<String>,
    path: Vec<String>,
}

fn run_versification(fixture: &VersificationFixture, r: &mut Recorder) {
    let table = AliasTable::english();
    let mut entries = Vec::new();
    for (index, row) in fixture.rows.iter().enumerate() {
        match expand_row(row, &table) {
            Ok(expanded) => entries.extend(expanded),
            Err(e) => r.fail(
                &format!("row {index} `{}` expands", row.from),
                "same-length verse ranges",
                &e,
            ),
        }
    }
    let map = VerseMap::new(entries);
    for case in &fixture.cases {
        let reference = match parse(&case.reference, &table) {
            Ok(set) if set.ranges.len() == 1 => set.ranges[0].start.clone(),
            other => {
                r.fail(
                    &format!("{}: reference parses", case.name),
                    &case.reference,
                    &format!("{other:?}"),
                );
                continue;
            }
        };
        let result = map.map(
            &reference,
            &SchemeId::new(&case.from),
            &SchemeId::new(&case.to),
        );
        r.check(
            &case.name,
            &MappingShape {
                status: case.status,
                targets: case.targets.clone(),
                path: case.path.clone(),
            },
            &MappingShape {
                status: result.status,
                targets: result.targets.iter().map(|t| t.to_osis()).collect(),
                path: result.path.iter().map(|s| s.as_str().to_owned()).collect(),
            },
        );
    }
    for case in &fixture.set_cases {
        let expand = |texts: &[String]| -> Result<Vec<String>, String> {
            let mut out = Vec::new();
            for text in texts {
                out.extend(single_range(text, &table)?.iter().map(|r| r.to_osis()));
            }
            Ok(out)
        };
        let (Ok(references), Ok(to_references), Ok(absent), Ok(unknown)) = (
            expand(&case.references),
            expand(&case.to_references),
            expand(&case.absent),
            expand(&case.unknown),
        ) else {
            r.fail(
                &format!("{}: references parse", case.name),
                "references or same-chapter ranges",
                &format!("{:?}", case.references),
            );
            continue;
        };
        let refs: Vec<openlogos_core::reference::Reference> = references
            .iter()
            .filter_map(|t| parse(t, &table).ok())
            .map(|set| set.ranges[0].start.clone())
            .collect();
        let result = map.map_many(&refs, &SchemeId::new(&case.from), &SchemeId::new(&case.to));
        r.check(
            &case.name,
            &SetShape {
                status: case.status,
                to: to_references,
                absent,
                unknown,
                path: case.path.clone(),
            },
            &SetShape {
                status: result.status,
                to: result.to.iter().map(|t| t.to_osis()).collect(),
                absent: result.absent.iter().map(|t| t.to_osis()).collect(),
                unknown: result.unknown.iter().map(|t| t.to_osis()).collect(),
                path: result.path.iter().map(|s| s.as_str().to_owned()).collect(),
            },
        );
        // The record round-trips through JSON (the reference-mapping schema shape).
        let round = serde_json::to_string(&result)
            .ok()
            .and_then(|json| serde_json::from_str::<ReferenceMapping>(&json).ok());
        r.check(
            &format!("{}: record round-trips", case.name),
            &Some(result.clone()),
            &round,
        );
    }
    let tables: Vec<Result<SegmentTable, String>> = fixture
        .segment_tables
        .iter()
        .map(|t| {
            Ok(SegmentTable {
                edition_id: t.edition_id.clone(),
                scheme: SchemeId::new(&t.scheme),
                segments: t
                    .segments
                    .iter()
                    .map(|s| {
                        Ok(SegmentSpan {
                            segment_id: s.segment_id.clone(),
                            native_label: s.native_label.clone(),
                            verses: s
                                .verses
                                .iter()
                                .map(|v| single_reference(v, &table))
                                .collect::<Result<Vec<_>, _>>()?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            })
        })
        .collect();
    for case in &fixture.segment_cases {
        let found = tables.iter().find_map(|t| match t {
            Ok(t) if t.edition_id == case.edition_id => Some(t),
            _ => None,
        });
        let (Some(segments), Ok(reference)) = (found, single_reference(&case.reference, &table))
        else {
            r.fail(
                &format!("{}: table and reference exist", case.name),
                &case.edition_id,
                &format!("{tables:?}"),
            );
            continue;
        };
        let resolution = segments.resolve(&reference);
        r.check(
            &case.name,
            &SegmentShape {
                status: case.status,
                segment_ids: case.segment_ids.clone(),
                native_labels: case.native_labels.clone(),
                bridged: case.bridged,
            },
            &SegmentShape {
                status: resolution.status,
                segment_ids: resolution.segment_ids,
                native_labels: resolution.native_labels,
                bridged: resolution.bridged,
            },
        );
    }
}

/// One verse reference in OSIS form.
fn single_reference(
    text: &str,
    table: &AliasTable,
) -> Result<openlogos_core::reference::Reference, String> {
    let verses = single_range(text, table)?;
    match verses.as_slice() {
        [one] => Ok(one.clone()),
        _ => Err(format!("`{text}` must be one verse")),
    }
}

/// Status, targets, absent, unknown and path of a set mapping, for one comparison.
#[derive(Serialize, PartialEq)]
struct SetShape {
    status: MappingStatus,
    to: Vec<String>,
    absent: Vec<String>,
    unknown: Vec<String>,
    path: Vec<String>,
}

/// The fields of a segment resolution a fixture pins down.
#[derive(Serialize, PartialEq)]
struct SegmentShape {
    status: MappingStatus,
    segment_ids: Vec<String>,
    native_labels: Vec<String>,
    bridged: bool,
}

/// Canonical form, human form and flags of a parse, for one comparison.
#[derive(Serialize, PartialEq)]
struct ParseShape {
    osis: String,
    human: String,
    flags: Value,
}

fn run_references(fixture: &ReferenceFixture, r: &mut Recorder) {
    if fixture.alias_table != "english" {
        r.fail("alias table exists", "english", &fixture.alias_table);
        return;
    }
    let table = AliasTable::english();
    for case in &fixture.cases {
        let actual = parse(&case.input, &table).map(|set| ParseShape {
            osis: set.to_osis(),
            human: set.to_human(&table),
            flags: serde_json::to_value(&set.flags).unwrap_or(Value::Null),
        });
        r.check(
            &format!("parses `{}`", case.input),
            &Ok(ParseShape {
                osis: case.osis.clone(),
                human: case.human.clone(),
                flags: case.flags.clone(),
            }),
            &actual.map_err(|e| e.to_string()),
        );
        // The canonical form must read back to the same ranges.
        let reparsed = parse(&case.osis, &table).map(|set| set.to_osis());
        r.check(
            &format!("canonical form of `{}` is stable", case.input),
            &Ok(case.osis.clone()),
            &reparsed.map_err(|e| e.to_string()),
        );
    }
    for case in &fixture.errors {
        let actual = match parse(&case.input, &table) {
            Ok(set) => format!("parsed as {}", set.to_osis()),
            Err(error) => serde_json::to_value(&error)
                .ok()
                .and_then(|v| v.get("kind").and_then(Value::as_str).map(str::to_owned))
                .unwrap_or_else(|| error.to_string()),
        };
        r.check(&format!("rejects `{}`", case.input), &case.error, &actual);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fixture_file_parses() {
        assert_eq!(fixtures().len(), FIXTURE_FILES.len());
        let ids: Vec<String> = fixtures().iter().map(|f| f.id().to_owned()).collect();
        assert_eq!(ids, FIXTURE_FILES.map(|(id, _)| id.to_owned()).to_vec());
        let array: Vec<Value> = serde_json::from_str(fixtures_json()).unwrap();
        assert_eq!(array.len(), FIXTURE_FILES.len());
        for (fixture, (id, _)) in array.iter().zip(FIXTURE_FILES) {
            assert_eq!(fixture["id"], id);
            assert!(
                fixture["description"]
                    .as_str()
                    .is_some_and(|d| !d.is_empty())
            );
            assert!(fixture["license"].as_str().is_some_and(|l| !l.is_empty()));
        }
    }

    #[test]
    fn the_suite_passes() {
        let report = run_all();
        let failures: Vec<String> = report
            .failures()
            .iter()
            .map(|c| {
                format!(
                    "{}/{}: expected {} got {}",
                    c.fixture, c.name, c.expected, c.actual
                )
            })
            .collect();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        assert!(report.all_passed());
        eprintln!(
            "conformance cases: {} passed, {} failed",
            report.passed, report.failed
        );
        for (id, _) in FIXTURE_FILES {
            let count = report.cases.iter().filter(|c| c.fixture == id).count();
            eprintln!("  {id}: {count} cases");
            assert!(count > 0, "{id} ran no cases");
        }
        assert!(report.passed > 100, "{}", report.passed);
        assert_eq!(report.api_version, openlogos_core::api_version());
    }

    #[test]
    fn case_names_are_unique_within_a_fixture() {
        let report = run_all();
        let mut seen = std::collections::HashSet::new();
        for case in &report.cases {
            assert!(
                seen.insert((case.fixture.clone(), case.name.clone())),
                "{}/{}",
                case.fixture,
                case.name
            );
        }
    }

    #[test]
    fn the_report_serializes() {
        let report = run_all();
        let json = serde_json::to_string(&report).unwrap();
        let back: ConformanceReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, report);
    }

    #[test]
    fn hebrew_fixture_is_not_nfc_stable_by_construction() {
        // The fixture stores WLC bytes as encoded; shin dot and hiriq, or dagesh and sheva,
        // are in an order NFC would change. This test pins the first word's scalar sequence
        // so an accidental normalization of the file shows up.
        let Fixture::Text(fixture) = &fixtures()[0] else {
            panic!("first fixture is the Hebrew text");
        };
        let first: Vec<u32> = fixture.segments[0]
            .text
            .chars()
            .take(10)
            .map(|c| c as u32)
            .collect();
        assert_eq!(
            first,
            vec![
                0x05D1, 0x05BC, 0x05B0, 0x05E8, 0x05B5, 0x05D0, 0x05E9, 0x05C1, 0x05B4, 0x0596
            ]
        );
    }
}
