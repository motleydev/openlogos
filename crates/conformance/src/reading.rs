//! The continuous reading fixture (B078, specs/23): locating passages, chapter neighbours
//! across book and testament boundaries, chapter blocks with placeholder flags and the
//! fixture overlay, segment references, the canon table's Hebrew/English numbering (checked
//! against the `joel-malachi-chapter-numbering` and `psalm-9-10-numbering` fixtures as well),
//! the shell following the reading position, and the text pack reader on the fixture packs
//! under `fixtures/packs/`. Swift, Kotlin and JavaScript hosts run it through
//! [`crate::run_all`].
//!
//! [`reading_library`] is the placeholder library the bindings' reading handles start with:
//! BSB (66 books, `eng`), SBLGNT (New Testament, `org`) and WLC (Old Testament, `org`), each
//! generated from the canon table with the text fixtures' verses overlaid.
//! [`fixture_pack_library`] reads the same fixture verses from the three packs written in
//! the B078 pack format ([`fixture_packs`] writes them; a test keeps the files equal).

use std::collections::BTreeMap;

use openlogos_core::canon::{self, BookOrder};
use openlogos_core::reference::{BookId, Reference};
use openlogos_core::versification::SchemeId;
use openlogos_text::reading::{
    ChapterBlock, ChapterHeading, ChapterNeighbors, ChapterVerse, ChunkRef, Edition, FilePack,
    Location, MemorySource, OverlayVerse, PackWriter, PlaceholderPack, ReadingError,
    ReadingLibrary, VerseToken, WrittenPack, book_title, reference_for_segment,
};
use openlogos_workspaces::commands::{CommandArgs, CommandContext, Invocation, Shell};
use openlogos_workspaces::reading::TabPosition;
use openlogos_workspaces::shell::Session;
use serde::{Deserialize, Serialize};

use crate::{Fixture, Recorder, TextFixture};

/// The fixture pack files as committed, by path under `fixtures/packs/`.
pub const FIXTURE_PACK_FILES: [(&str, &[u8]); 6] = [
    (
        "bsb/manifest.json",
        include_bytes!("../fixtures/packs/bsb/manifest.json"),
    ),
    (
        "bsb/chapters/John/3.json",
        include_bytes!("../fixtures/packs/bsb/chapters/John/3.json"),
    ),
    (
        "sblgnt/manifest.json",
        include_bytes!("../fixtures/packs/sblgnt/manifest.json"),
    ),
    (
        "sblgnt/chapters/John/3.json",
        include_bytes!("../fixtures/packs/sblgnt/chapters/John/3.json"),
    ),
    (
        "wlc/manifest.json",
        include_bytes!("../fixtures/packs/wlc/manifest.json"),
    ),
    (
        "wlc/chapters.bin",
        include_bytes!("../fixtures/packs/wlc/chapters.bin"),
    ),
];

/// One fixture edition: its resource id, the text fixture it overlays, names, language,
/// direction, scheme and books.
struct FixtureEdition {
    resource_id: &'static str,
    text_fixture: &'static str,
    title: &'static str,
    name: &'static str,
    language: &'static str,
    direction: &'static str,
    scheme: &'static str,
    books: fn() -> Vec<BookId>,
    bundled: bool,
}

const EDITIONS: [FixtureEdition; 3] = [
    FixtureEdition {
        resource_id: "bsb",
        text_fixture: "cross-verse-selection-john-3",
        title: "BSB",
        name: "Berean Standard Bible",
        language: "en",
        direction: "ltr",
        scheme: "eng",
        books: BookId::all,
        bundled: false,
    },
    FixtureEdition {
        resource_id: "sblgnt",
        text_fixture: "greek-diacritics-john-3-sblgnt",
        title: "SBLGNT",
        name: "SBL Greek New Testament",
        language: "grc",
        direction: "ltr",
        scheme: "org",
        books: canon::new_testament,
        bundled: false,
    },
    FixtureEdition {
        resource_id: "wlc",
        text_fixture: "hebrew-cantillation-genesis-1",
        title: "WLC",
        name: "Westminster Leningrad Codex",
        language: "hbo",
        direction: "rtl",
        scheme: "org",
        books: canon::old_testament,
        bundled: true,
    },
];

fn text_fixture(id: &str) -> TextFixture {
    let json = crate::FIXTURE_FILES
        .iter()
        .find(|(fid, _)| *fid == id)
        .map(|(_, json)| *json)
        .expect("the text fixture is compiled in");
    match serde_json::from_str::<Fixture>(json) {
        Ok(Fixture::Text(fixture)) => fixture,
        _ => panic!("{id} is a text fixture"),
    }
}

fn edition(e: &FixtureEdition, fixture: &TextFixture) -> Edition {
    Edition {
        resource_id: e.resource_id.to_owned(),
        content_revision: fixture.content_revision.clone(),
        title: e.title.to_owned(),
        name: e.name.to_owned(),
        language: e.language.to_owned(),
        direction: e.direction.to_owned(),
        scheme: e.scheme.to_owned(),
        book_order: BookOrder::Protestant,
        placeholder: false,
        attribution: None,
    }
}

fn tokens(segment: &crate::FixtureSegment) -> Vec<VerseToken> {
    segment
        .tokens
        .iter()
        .map(|t| VerseToken {
            id: t.id.clone(),
            start: t.start,
            end: t.end,
        })
        .collect()
}

/// The placeholder library: BSB, SBLGNT and WLC generated from the canon table, with the
/// text fixtures' verses overlaid (BSB John 3:16–17, SBLGNT John 3:16, WLC Genesis 1:1–2).
#[must_use]
pub fn reading_library() -> ReadingLibrary {
    let mut library = ReadingLibrary::new();
    for e in &EDITIONS {
        let fixture = text_fixture(e.text_fixture);
        let overlay = fixture
            .segments
            .iter()
            .map(|s| OverlayVerse {
                segment_id: s.segment_id.clone(),
                text: s.text.clone(),
                tokens: tokens(s),
            })
            .collect();
        let pack = PlaceholderPack::new(edition(e, &fixture), &(e.books)(), overlay)
            .expect("the fixture schemes are in the canon table");
        library.insert(Box::new(pack));
    }
    library
}

/// Resource schemes as a host registers them from [`reading_library`]: `bsb` → `eng`,
/// `sblgnt` and `wlc` → `org`.
#[must_use]
pub fn fixture_resource_schemes() -> BTreeMap<String, String> {
    EDITIONS
        .iter()
        .map(|e| (e.resource_id.to_owned(), e.scheme.to_owned()))
        .collect()
}

/// The three fixture packs written from the text fixtures alone: one chapter each, BSB and
/// SBLGNT as chapter files, WLC as one bundle. By resource id.
#[must_use]
pub fn fixture_packs() -> Vec<(String, WrittenPack)> {
    EDITIONS
        .iter()
        .map(|e| {
            let fixture = text_fixture(e.text_fixture);
            let mut writer = PackWriter::new(
                &edition(e, &fixture),
                &fixture.license,
                &fixture.source,
                true,
            );
            if e.bundled {
                writer = writer.bundled();
            }
            let first = canon::segment_reference(&fixture.segments[0].segment_id)
                .expect("fixture segments are verses");
            let verses = fixture
                .segments
                .iter()
                .map(|s| {
                    let number = canon::segment_reference(&s.segment_id)
                        .and_then(|r| r.verse)
                        .unwrap_or(0);
                    ChapterVerse {
                        segment_id: s.segment_id.clone(),
                        number,
                        last_number: number,
                        label: number.to_string(),
                        text: s.text.clone(),
                        paragraph_start: number == 1
                            || s.segment_id == fixture.segments[0].segment_id,
                        poetry: false,
                        superscription: false,
                        placeholder: false,
                        tokens: tokens(s),
                    }
                })
                .collect();
            writer.chapter(first.book, book_title(first.book), first.chapter, verses);
            let written = writer.finish().expect("fixture chapters have verses");
            (e.resource_id.to_owned(), written)
        })
        .collect()
}

/// The library of the committed fixture packs, read through [`FilePack`].
#[must_use]
pub fn fixture_pack_library() -> ReadingLibrary {
    let mut library = ReadingLibrary::new();
    for e in &EDITIONS {
        let prefix = format!("{}/", e.resource_id);
        let files: Vec<(String, Vec<u8>)> = FIXTURE_PACK_FILES
            .iter()
            .filter_map(|(path, bytes)| {
                path.strip_prefix(&prefix)
                    .map(|p| (p.to_owned(), bytes.to_vec()))
            })
            .collect();
        let manifest = files
            .iter()
            .find(|(p, _)| p == "manifest.json")
            .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
            .expect("every fixture pack has a manifest");
        let pack = FilePack::from_manifest(&manifest, Some(Box::new(MemorySource::new(files))))
            .expect("the fixture packs read");
        library.insert(Box::new(pack));
    }
    library
}

/// Which library a case reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryKind {
    /// [`reading_library`].
    #[default]
    Placeholder,
    /// [`fixture_pack_library`].
    Packs,
}

/// An error as `kind` and its words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorView {
    /// The serde tag.
    pub kind: String,
    /// The `Display` text.
    pub text: String,
}

fn error_view(error: &ReadingError) -> ErrorView {
    ErrorView {
        kind: serde_json::to_value(error)
            .ok()
            .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_owned))
            .unwrap_or_default(),
        text: error.to_string(),
    }
}

/// `locate` with the location or the error expected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocateCase {
    /// Unique within the fixture.
    pub name: String,
    /// The library.
    #[serde(default)]
    pub library: LibraryKind,
    /// The edition.
    pub resource_id: String,
    /// What was typed.
    pub input: String,
    /// The location.
    #[serde(default)]
    pub expect: Option<Location>,
    /// Or the error.
    #[serde(default)]
    pub error: Option<ErrorView>,
}

/// `chapter_neighbors` with its answer or error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeighborCase {
    /// Unique within the fixture.
    pub name: String,
    /// The library.
    #[serde(default)]
    pub library: LibraryKind,
    /// The edition.
    pub resource_id: String,
    /// The chapter.
    pub chapter_key: String,
    /// The neighbours.
    #[serde(default)]
    pub expect: Option<ChapterNeighbors>,
    /// Or the error.
    #[serde(default)]
    pub error: Option<ErrorView>,
}

/// What one verse of a block must be; only the fields present are checked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VerseExpect {
    /// The segment, which identifies the verse.
    pub segment_id: String,
    /// Its label.
    pub label: Option<String>,
    /// Its text.
    pub text: Option<String>,
    /// Whether it is placeholder text.
    pub placeholder: Option<bool>,
    /// Whether it is a Psalm title.
    pub superscription: Option<bool>,
    /// Whether it is poetry.
    pub poetry: Option<bool>,
    /// Whether a paragraph starts at it.
    pub paragraph_start: Option<bool>,
    /// How many tokens it has.
    pub token_count: Option<usize>,
}

fn verse_view(expect: &VerseExpect, actual: Option<&ChapterVerse>) -> VerseExpect {
    let Some(v) = actual else {
        return VerseExpect::default();
    };
    VerseExpect {
        segment_id: v.segment_id.clone(),
        label: expect.label.as_ref().map(|_| v.label.clone()),
        text: expect.text.as_ref().map(|_| v.text.clone()),
        placeholder: expect.placeholder.map(|_| v.placeholder),
        superscription: expect.superscription.map(|_| v.superscription),
        poetry: expect.poetry.map(|_| v.poetry),
        paragraph_start: expect.paragraph_start.map(|_| v.paragraph_start),
        token_count: expect.token_count.map(|_| v.tokens.len()),
    }
}

/// What a chapter block must be; only the fields present are checked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChapterExpect {
    /// The heading.
    pub heading: Option<ChapterHeading>,
    /// How many verses.
    pub verse_count: Option<usize>,
    /// Whether any verse is placeholder text.
    pub placeholder: Option<bool>,
    /// How many verses are placeholder text.
    pub placeholder_count: Option<usize>,
    /// The chapter before, `""` for none.
    pub prev: Option<String>,
    /// The chapter after, `""` for none.
    pub next: Option<String>,
    /// Verses by segment id.
    pub verses: Vec<VerseExpect>,
}

/// `chapter_content` with what must hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChapterCase {
    /// Unique within the fixture.
    pub name: String,
    /// The library.
    #[serde(default)]
    pub library: LibraryKind,
    /// The edition.
    pub resource_id: String,
    /// The chapter.
    pub chapter_key: String,
    /// What must hold.
    #[serde(default)]
    pub expect: Option<ChapterExpect>,
    /// Or the error.
    #[serde(default)]
    pub error: Option<ErrorView>,
}

fn chapter_view(expect: &ChapterExpect, block: &ChapterBlock) -> ChapterExpect {
    let edge = |k: &Option<String>| k.clone().unwrap_or_default();
    ChapterExpect {
        heading: expect.heading.as_ref().map(|_| block.heading.clone()),
        verse_count: expect.verse_count.map(|_| block.verses.len()),
        placeholder: expect.placeholder.map(|_| block.placeholder),
        placeholder_count: expect
            .placeholder_count
            .map(|_| block.verses.iter().filter(|v| v.placeholder).count()),
        prev: expect.prev.as_ref().map(|_| edge(&block.prev)),
        next: expect.next.as_ref().map(|_| edge(&block.next)),
        verses: expect
            .verses
            .iter()
            .map(|v| {
                verse_view(
                    v,
                    block.verses.iter().find(|a| a.segment_id == v.segment_id),
                )
            })
            .collect(),
    }
}

/// `reference_for_segment`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentCase {
    /// The segment id.
    pub segment_id: String,
    /// Its verse, OSIS, or `None`.
    pub reference: Option<String>,
}

/// One verse through the canon table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonCase {
    /// Unique within the fixture.
    pub name: String,
    /// The verse, OSIS.
    pub reference: String,
    /// Source scheme.
    pub from: String,
    /// Target scheme.
    pub to: String,
    /// `exact`, `partial` or `unmapped`.
    pub status: String,
    /// Targets, OSIS.
    pub targets: Vec<String>,
}

/// Chapters of a book in a scheme, with some chapters' verse ranges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShapeCase {
    /// The scheme.
    pub scheme: String,
    /// The book, OSIS.
    pub book: String,
    /// Its chapters.
    pub chapters: u32,
    /// `[chapter, first verse, last verse]` rows.
    #[serde(default)]
    pub verses: Vec<[u32; 3]>,
}

/// A chunk located and handed back, as a browser does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkCase {
    /// Unique within the fixture.
    pub name: String,
    /// The pack.
    pub resource_id: String,
    /// The chapter.
    pub chapter_key: String,
    /// Where the chunk is.
    pub chunk: ChunkRef,
    /// The error supplying the chunk with one byte flipped.
    pub tampered: ErrorView,
}

/// The fixture packs' manifests and chunks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackCases {
    /// SHA-256 of each committed manifest, by resource id: what a catalog records.
    pub manifest_sha256: BTreeMap<String, String>,
    /// Chunks.
    pub chunks: Vec<ChunkCase>,
}

/// A tab's state after a step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabState {
    /// Its reference.
    pub reference: Option<String>,
    /// The marked verses.
    pub highlight_range: Option<String>,
    /// Its scroll anchor's segment.
    pub scroll_segment_id: Option<String>,
    /// How many visits Back has.
    pub back: usize,
    /// The identity row's label.
    pub reference_label: Option<String>,
    /// The tab strip's label.
    pub chapter_label: Option<String>,
}

/// What must hold after a step; only the fields present are checked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReadingStepExpect {
    /// `Effects.positions`, in order.
    pub positions: Option<Vec<TabPosition>>,
    /// Tab ids of the positions, in order: the short form.
    pub moved: Option<Vec<String>>,
    /// Whether the revision moved.
    pub revision_bumped: Option<bool>,
    /// Whether it went on the undo stack.
    pub recorded: Option<bool>,
    /// Tabs by id (of the active workspace).
    pub tabs: Option<BTreeMap<String, TabState>>,
}

/// One command and what must hold after it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadingStep {
    /// The command id.
    pub command: String,
    /// Its arguments.
    #[serde(default)]
    pub args: CommandArgs,
    /// What must hold.
    #[serde(default)]
    pub expect: ReadingStepExpect,
}

/// Steps from the fixture session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadingScenario {
    /// Unique within the fixture.
    pub name: String,
    /// The steps.
    pub steps: Vec<ReadingStep>,
}

/// The continuous reading fixture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadingFixture {
    /// Fixture id (also the file name).
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License of the data.
    pub license: String,
    /// Where the expectations come from.
    pub source: String,
    /// Resource schemes, as a host registers them.
    pub resource_schemes: BTreeMap<String, String>,
    /// The session every scenario starts from.
    pub session: Session,
    /// `locate` cases.
    pub locate: Vec<LocateCase>,
    /// `chapter_neighbors` cases.
    pub neighbors: Vec<NeighborCase>,
    /// `chapter_content` cases.
    pub chapters: Vec<ChapterCase>,
    /// `reference_for_segment` cases (not `segments`, which text fixtures use: hosts decode
    /// every fixture with one shape).
    pub segment_references: Vec<SegmentCase>,
    /// Canon mapping cases.
    pub canon: Vec<CanonCase>,
    /// Canon structure cases.
    pub shapes: Vec<ShapeCase>,
    /// Pack cases.
    pub packs: PackCases,
    /// Shell scenarios.
    pub scenarios: Vec<ReadingScenario>,
}

fn status_name(status: openlogos_core::versification::MappingStatus) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn tab_state(shell: &Shell, tab_id: &str) -> Option<TabState> {
    let ws = shell.session.active();
    let (_, tab) = ws.find_tab(tab_id)?;
    let (reference_label, chapter_label) =
        openlogos_workspaces::view::reference_labels(tab.reference.as_deref());
    Some(TabState {
        reference: tab.reference.clone(),
        highlight_range: tab.highlight_range.clone(),
        scroll_segment_id: tab.scroll_anchor.as_ref().map(|a| a.segment_id.clone()),
        back: tab.history.back.len(),
        reference_label,
        chapter_label,
    })
}

fn result_view<T: Clone>(result: &Result<T, ReadingError>) -> (Option<T>, Option<ErrorView>) {
    match result {
        Ok(v) => (Some(v.clone()), None),
        Err(e) => (None, Some(error_view(e))),
    }
}

/// Runs every case of the fixture.
pub(crate) fn run_reading(fixture: &ReadingFixture, recorder: &mut Recorder) {
    let placeholder = reading_library();
    let packs = fixture_pack_library();
    let library = |kind: LibraryKind| match kind {
        LibraryKind::Placeholder => &placeholder,
        LibraryKind::Packs => &packs,
    };

    for case in &fixture.locate {
        let actual = result_view(&library(case.library).locate(&case.resource_id, &case.input));
        recorder.check(
            &format!("locate: {}", case.name),
            &(case.expect.clone(), case.error.clone()),
            &actual,
        );
    }
    for case in &fixture.neighbors {
        let actual = result_view(
            &library(case.library).chapter_neighbors(&case.resource_id, &case.chapter_key),
        );
        recorder.check(
            &format!("neighbors: {}", case.name),
            &(case.expect.clone(), case.error.clone()),
            &actual,
        );
    }
    for case in &fixture.chapters {
        let actual = library(case.library).chapter_content(&case.resource_id, &case.chapter_key);
        let actual = match (&case.expect, &actual) {
            (Some(expect), Ok(block)) => (Some(chapter_view(expect, block)), None),
            (_, Ok(_)) => (Some(ChapterExpect::default()), None),
            (_, Err(e)) => (None, Some(error_view(e))),
        };
        recorder.check(
            &format!("chapter: {}", case.name),
            &(case.expect.clone(), case.error.clone()),
            &actual,
        );
    }
    let segments: Vec<SegmentCase> = fixture
        .segment_references
        .iter()
        .map(|c| SegmentCase {
            segment_id: c.segment_id.clone(),
            reference: reference_for_segment(&c.segment_id),
        })
        .collect();
    recorder.check(
        "reference_for_segment",
        &fixture.segment_references,
        &segments,
    );

    let canon_case = |name: &str, reference: &str, from: &str, to: &str| -> CanonCase {
        let parsed = openlogos_workspaces::palette::parse_tab_reference(reference)
            .map(|s| s.ranges[0].start.clone());
        let result = match parsed {
            Some(r) => canon::map(&r, &SchemeId::new(from), &SchemeId::new(to)),
            None => canon::map(
                &Reference::chapter(BookId::Gen, 1),
                &SchemeId::new(""),
                &SchemeId::new(""),
            ),
        };
        CanonCase {
            name: name.to_owned(),
            reference: reference.to_owned(),
            from: from.to_owned(),
            to: to.to_owned(),
            status: status_name(result.status),
            targets: result.targets.iter().map(Reference::to_osis).collect(),
        }
    };
    for case in &fixture.canon {
        let actual = canon_case(&case.name, &case.reference, &case.from, &case.to);
        recorder.check(&format!("canon: {}", case.name), case, &actual);
    }
    // The table agrees with the hand-written eng/org fixtures.
    for id in ["joel-malachi-chapter-numbering", "psalm-9-10-numbering"] {
        let json = crate::FIXTURE_FILES
            .iter()
            .find(|(fid, _)| *fid == id)
            .map(|(_, j)| *j)
            .unwrap_or_default();
        let Ok(Fixture::Versification(v)) = serde_json::from_str::<Fixture>(json) else {
            recorder.fail(&format!("canon agrees with {id}"), "a fixture", "none");
            continue;
        };
        // Only verses the fixture's rows speak about: outside them it asserts nothing about
        // the numbering, only about its own table.
        let rows = crate::verse_map_from_rows(&v.rows).unwrap_or_default();
        for case in v.cases {
            let schemes = [case.from.as_str(), case.to.as_str()];
            if !schemes.iter().all(|s| canon::SCHEMES.contains(s)) {
                continue;
            }
            let covered = openlogos_workspaces::palette::parse_tab_reference(&case.reference)
                .map(|set| set.ranges[0].start.clone())
                .is_some_and(|r| {
                    rows.entries
                        .iter()
                        .any(|e| e.from == r || e.to.as_ref() == Some(&r))
                });
            if !covered {
                continue;
            }
            let expected = CanonCase {
                name: case.name.clone(),
                reference: case.reference.clone(),
                from: case.from.clone(),
                to: case.to.clone(),
                status: status_name(case.status),
                targets: case.targets.clone(),
            };
            let actual = canon_case(&case.name, &case.reference, &case.from, &case.to);
            recorder.check(
                &format!("canon agrees with {id}: {}", case.name),
                &expected,
                &actual,
            );
        }
    }
    for case in &fixture.shapes {
        let scheme = SchemeId::new(&case.scheme);
        let book = BookId::from_osis(&case.book);
        let actual = ShapeCase {
            scheme: case.scheme.clone(),
            book: case.book.clone(),
            chapters: book
                .and_then(|b| canon::chapter_count(&scheme, b))
                .unwrap_or(0),
            verses: case
                .verses
                .iter()
                .map(|[c, _, _]| {
                    let first = book.and_then(|b| canon::first_verse(&scheme, b, *c));
                    let last = book.and_then(|b| canon::last_verse(&scheme, b, *c));
                    [*c, first.unwrap_or(u32::MAX), last.unwrap_or(0)]
                })
                .collect(),
        };
        recorder.check(
            &format!("shape: {} {}", case.scheme, case.book),
            case,
            &actual,
        );
    }

    run_packs(fixture, recorder, &packs);
    run_scenarios(fixture, recorder);
}

fn run_packs(fixture: &ReadingFixture, recorder: &mut Recorder, packs: &ReadingLibrary) {
    let actual: BTreeMap<String, String> = FIXTURE_PACK_FILES
        .iter()
        .filter_map(|(path, bytes)| {
            path.strip_suffix("/manifest.json").map(|id| {
                (
                    id.to_owned(),
                    openlogos_text::reading::pack::sha256_hex(bytes),
                )
            })
        })
        .collect();
    recorder.check(
        "pack manifests are the committed ones",
        &fixture.packs.manifest_sha256,
        &actual,
    );
    // Every pack verse is its text fixture's segment, byte for byte, with its tokens.
    for e in &EDITIONS {
        let text = text_fixture(e.text_fixture);
        let first = canon::segment_reference(&text.segments[0].segment_id)
            .map(|r| openlogos_text::reading::chapter_key(r.book, r.chapter))
            .unwrap_or_default();
        let expected: Vec<(String, String, usize)> = text
            .segments
            .iter()
            .map(|s| (s.segment_id.clone(), s.text.clone(), s.tokens.len()))
            .collect();
        let actual: Vec<(String, String, usize)> = packs
            .chapter_content(e.resource_id, &first)
            .map(|b| {
                b.verses
                    .into_iter()
                    .map(|v| (v.segment_id, v.text, v.tokens.len()))
                    .collect()
            })
            .unwrap_or_default();
        recorder.check(
            &format!("pack {} reads its text fixture back", e.resource_id),
            &expected,
            &actual,
        );
    }
    for case in &fixture.packs.chunks {
        let mut browser = ReadingLibrary::new();
        let prefix = format!("{}/", case.resource_id);
        let file = |name: &str| {
            FIXTURE_PACK_FILES
                .iter()
                .find(|(p, _)| p.strip_prefix(&prefix) == Some(name))
                .map(|(_, b)| b.to_vec())
                .unwrap_or_default()
        };
        let manifest = String::from_utf8(file("manifest.json")).unwrap_or_default();
        let Ok(pack) = FilePack::from_manifest(&manifest, None) else {
            recorder.fail(&format!("chunk: {}", case.name), "a manifest", "none");
            continue;
        };
        browser.insert(Box::new(pack));
        let chunk = browser
            .chunk_for(&case.resource_id, &case.chapter_key)
            .ok()
            .flatten();
        recorder.check(
            &format!("chunk: {}: where it is", case.name),
            &Some(case.chunk.clone()),
            &chunk,
        );
        let not_loaded = browser
            .chapter_content(&case.resource_id, &case.chapter_key)
            .err()
            .map(|e| error_view(&e).kind);
        recorder.check(
            &format!("chunk: {}: not loaded before it is supplied", case.name),
            &Some("chunk_not_loaded".to_owned()),
            &not_loaded,
        );
        let Some(chunk) = chunk else { continue };
        let whole = file(&chunk.path);
        let start = usize::try_from(chunk.offset.unwrap_or(0)).unwrap_or(0);
        let end = start + usize::try_from(chunk.length).unwrap_or(0);
        let bytes = whole
            .get(start..end)
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        let mut tampered = bytes.clone();
        if let Some(b) = tampered.get_mut(20) {
            *b ^= 1;
        }
        let refused = browser
            .supply_chunk(&case.resource_id, &case.chapter_key, &tampered)
            .err()
            .map(|e| error_view(&e));
        recorder.check(
            &format!("chunk: {}: a tampered chunk is refused", case.name),
            &Some(case.tampered.clone()),
            &refused,
        );
        let supplied = browser
            .supply_chunk(&case.resource_id, &case.chapter_key, &bytes)
            .map_err(|e| error_view(&e))
            .and_then(|()| {
                browser
                    .chapter_content(&case.resource_id, &case.chapter_key)
                    .map_err(|e| error_view(&e))
            })
            .map(|b| b.verses.len());
        let expected = packs
            .chapter_content(&case.resource_id, &case.chapter_key)
            .map(|b| b.verses.len())
            .map_err(|e| error_view(&e));
        recorder.check(
            &format!("chunk: {}: reads once supplied", case.name),
            &expected,
            &supplied,
        );
    }
}

fn run_scenarios(fixture: &ReadingFixture, recorder: &mut Recorder) {
    for scenario in &fixture.scenarios {
        let Ok(mut shell) = Shell::new(fixture.session.clone()) else {
            recorder.fail(&scenario.name, "a session that checks", "it does not");
            return;
        };
        shell.set_resource_schemes(fixture.resource_schemes.clone());
        for (i, step) in scenario.steps.iter().enumerate() {
            let name = format!("{} · step {} {}", scenario.name, i + 1, step.command);
            let before = shell.revision;
            let outcome = shell.apply(
                &Invocation::with(&step.command, step.args.clone()),
                &CommandContext::default(),
            );
            let outcome = match outcome {
                Ok(o) => o,
                Err(e) => {
                    recorder.fail(&name, "the command runs", &e.to_string());
                    break;
                }
            };
            let e = &step.expect;
            let positions = &outcome.effects.positions;
            let actual = ReadingStepExpect {
                positions: e.positions.as_ref().map(|_| positions.clone()),
                moved: e
                    .moved
                    .as_ref()
                    .map(|_| positions.iter().map(|p| p.tab_id.clone()).collect()),
                revision_bumped: e.revision_bumped.map(|_| outcome.revision != before),
                recorded: e.recorded.map(|_| outcome.recorded),
                tabs: e.tabs.as_ref().map(|tabs| {
                    tabs.keys()
                        .filter_map(|id| tab_state(&shell, id).map(|t| (id.clone(), t)))
                        .collect()
                }),
            };
            recorder.check(&name, e, &actual);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed packs are what [`fixture_packs`] writes; `OPENLOGOS_WRITE_PACKS=1`
    /// rewrites them.
    #[test]
    fn the_committed_packs_are_written_from_the_text_fixtures() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/packs");
        let write = std::env::var_os("OPENLOGOS_WRITE_PACKS").is_some();
        let mut written = Vec::new();
        for (id, pack) in fixture_packs() {
            written.push((
                format!("{id}/manifest.json"),
                pack.manifest_json.into_bytes(),
            ));
            for (path, bytes) in pack.files {
                written.push((format!("{id}/{path}"), bytes));
            }
        }
        if write {
            for (path, bytes) in &written {
                let full = root.join(path);
                std::fs::create_dir_all(full.parent().unwrap()).unwrap();
                std::fs::write(full, bytes).unwrap();
            }
        }
        let committed: Vec<(String, Vec<u8>)> = FIXTURE_PACK_FILES
            .iter()
            .map(|(p, b)| ((*p).to_owned(), b.to_vec()))
            .collect();
        if !write {
            assert_eq!(written, committed, "run with OPENLOGOS_WRITE_PACKS=1");
        }
    }

    #[test]
    fn the_libraries_have_the_three_editions() {
        let ids: Vec<String> = reading_library()
            .editions()
            .into_iter()
            .map(|e| e.resource_id)
            .collect();
        assert_eq!(ids, ["bsb", "sblgnt", "wlc"]);
        assert_eq!(fixture_pack_library().editions().len(), 3);
        assert_eq!(fixture_resource_schemes()["wlc"], "org");
    }
}
