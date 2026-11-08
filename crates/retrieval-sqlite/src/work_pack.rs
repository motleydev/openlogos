//! The work pack (B016): a non-Bible work in one B068 corpus pack, read section by section.
//!
//! A work is not verse-shaped. Its places are the canonical locators of its scheme
//! (`institutes:3.2.7`), its front matter has no number at all, a section is several
//! paragraphs long, and it carries notes. The text pack of specs/23 §4 is keyed by Bible book,
//! chapter and verse, so a work does not fit it. specs/19 already says what a work ships as:
//! "one read-only SQLite file per edition or work" with text, FTS5, edges and a `section_map`.
//! This module adds only what reading such a pack needs on top of that schema
//! ([`crate::pack::PACK_SCHEMA_VERSION`] is unchanged, every addition is a new table or a new
//! manifest key, so the engine, the link index, the catalog and the edition search read the
//! same file):
//!
//! - manifest `work_format` = [`WORK_FORMAT`], `work_info` ([`WorkInfo`] as JSON) and
//!   `work_structure` (the [`WorkDivision`] list as JSON);
//! - `text`: one row per section, `verse_id` = the section's ordinal in reading order, `body`
//!   = its paragraphs joined by a blank line (`\n\n`; a block keeps its own line breaks), so
//!   `text_fts` searches the work through the shared tokenizer;
//! - `work_section(ordinal, locator, chapter, meta)`: the locator of each ordinal and its
//!   paragraph styles, label and notes as JSON;
//! - `work_citation`: every citation found in the notes, resolved or not (B016: "explicit
//!   citation links retain unresolved cases");
//! - `cite_span(first, last, citation)`: each resolved citation's Bible ranges cut at chapter
//!   ends, as decimal verse ids, which answers "which sections cite this verse" for verses
//!   inside a range;
//! - `edge`: one `explicitly_cites` row per resolved range (`src` = the work and the section's
//!   locator, `dst` = `bible` and a verse id or `first-last`, as the B057 relation pack writes
//!   them), so the device link index installs them like any pack's;
//! - `section_map`: every numbered section maps `equal` to its own segment.
//!
//! Bible locators are in the `eng` scheme (the work cites the English Bible's numbering).

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::Path;

use openlogos_core::canon;
use openlogos_core::locator::{GrammarTable, WorkLocator, parse_work_citation};
use openlogos_core::reference::{AliasTable, BookId, Reference, parse};
use openlogos_core::versification::SchemeId;
use openlogos_retrieval::{SectionMapKind, SectionMapRow, VerseId};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::fts::register_tokenizer;
use crate::pack::PACK_SCHEMA_VERSION;

/// The manifest's `work_format` value: what this module reads.
pub const WORK_FORMAT: &str = "openlogos-work-1";

/// The work every Bible locator of a citation belongs to (as B057's relation pack).
pub const BIBLE_WORK: &str = "bible";

/// The versification scheme the citations' Bible locators are in.
pub const CITATION_SCHEME: &str = "eng";

/// The relation of a citation edge (specs/03, "Typed claims").
pub const CITATION_RELATION: &str = "explicitly_cites";

/// Separator of paragraphs inside a section's stored body.
pub const PARAGRAPH_SEPARATOR: &str = "\n\n";

const SCHEMA: &str = "
CREATE TABLE work_section(ordinal INTEGER PRIMARY KEY, locator TEXT NOT NULL UNIQUE,
    chapter TEXT NOT NULL, meta TEXT NOT NULL);
CREATE TABLE work_citation(id INTEGER PRIMARY KEY, ordinal INTEGER NOT NULL, note TEXT NOT NULL,
    text TEXT NOT NULL, start INTEGER NOT NULL, end INTEGER NOT NULL, status TEXT NOT NULL,
    target TEXT, class TEXT, detail TEXT);
CREATE INDEX work_citation_ordinal ON work_citation(ordinal);
CREATE TABLE cite_span(first INTEGER NOT NULL, last INTEGER NOT NULL, citation INTEGER NOT NULL,
    PRIMARY KEY(first, citation)) WITHOUT ROWID;
";

/// One printed volume behind the edition, from its digitizer's record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkVolume {
    /// `1`, `2`.
    pub volume: u32,
    /// The record's title.
    pub title: String,
    /// The digitizer's identifier (`eBook #45001`).
    pub identifier: String,
    /// Release date as the record gives it.
    pub release_date: String,
    /// Last update as the record gives it.
    pub last_update: String,
    /// Who produced the digital text, as the record credits them; empty when it names no one.
    pub credits: String,
}

/// The bibliographic evidence of an edition: what B016's catalog entry shows offline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WorkBibliography {
    /// Author, as the record names them (`Calvin, Jean, 1509-1564`).
    pub author: String,
    /// Translator, as the record names them (`Allen, John, 1771-1839`).
    pub translator: String,
    /// The edition statement of the title page.
    pub edition: String,
    /// Place of publication as printed.
    pub place: String,
    /// Publisher as printed.
    pub publisher: String,
    /// The date the title page prints, as printed.
    pub date_printed: String,
    /// The date of the original the translation follows, as the edition names it (the
    /// author's last Latin edition, 1559), kept apart from the printing's date (specs/04).
    /// Empty in a pack written before the field existed.
    #[serde(default)]
    pub original_date: String,
    /// Subjects from the record.
    pub subjects: Vec<String>,
    /// The volumes.
    pub volumes: Vec<WorkVolume>,
    /// Anything a reader of the evidence should know about these fields.
    pub note: String,
}

/// What a work pack is: identity, rights, bibliography and counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkInfo {
    /// The edition's resource id (`institutes-allen`).
    pub resource_id: String,
    /// The work (`institutes`).
    pub work: String,
    /// The work's canonical locator scheme (`institutes`).
    pub locator_scheme: String,
    /// Changes whenever any byte of the text or its structure changes.
    pub content_revision: String,
    /// Full title.
    pub title: String,
    /// Short name for tabs and palette rows (`Institutes (Allen)`).
    pub short_title: String,
    /// BCP 47 language tag.
    pub language: String,
    /// The rights record (`S75`).
    pub source_id: String,
    /// Licence identifier of the rights record.
    pub license: String,
    /// What a reader must be shown.
    pub attribution: String,
    /// Where the text came from.
    pub source: String,
    /// The catalog's bibliographic evidence.
    pub bibliography: WorkBibliography,
    /// Numbered sections plus front-matter and argument units.
    pub sections: u32,
    /// Notes.
    pub notes: u32,
    /// Citations found in the notes, resolved or not.
    pub citations: u32,
    /// Of which resolved to a Bible passage.
    pub resolved: u32,
}

/// What a top-level division is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DivisionKind {
    /// Title pages, prefaces, dedication, syllabus.
    Front,
    /// A numbered book.
    Book,
}

/// What a chapter-level unit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChapterKind {
    /// One front-matter piece; one unnumbered section.
    Front,
    /// A book's argument; one unnumbered section.
    Argument,
    /// A numbered chapter of numbered sections.
    Chapter,
}

/// One chapter-level unit and the sections in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkChapterInfo {
    /// `3.2`, `1.argument`, `dedication`.
    pub locator: String,
    /// Its kind.
    pub kind: ChapterKind,
    /// The label as printed (`Chapter II.`, `CHAPTER XIV.`, `Argument.`), if any.
    pub label: Option<String>,
    /// The title as printed, if any.
    pub title: Option<String>,
    /// The locators of its sections in order.
    pub sections: Vec<String>,
}

/// One top-level division.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkDivision {
    /// `front`, `1` … `4`.
    pub locator: String,
    /// Its kind.
    pub kind: DivisionKind,
    /// The label as printed (`BOOK I.`), if any.
    pub label: Option<String>,
    /// The title as printed, if any.
    pub title: Option<String>,
    /// Its chapters in order.
    pub chapters: Vec<WorkChapterInfo>,
}

/// How a paragraph is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParagraphStyle {
    /// Running text.
    Prose,
    /// An indented block (verse, a centred line, a salutation); its line breaks are kept.
    Block,
    /// A heading inside a chapter (`The First Commandment.`, `CONFIRMATION.`).
    Heading,
}

/// What a note is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    /// Nothing but citations.
    Citations,
    /// Comment with no citation found in it.
    Comment,
    /// Comment with citations inside.
    Mixed,
}

/// One note, anchored where its marker stood.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkNote {
    /// Unique in the work: volume and printed number (`1:61`).
    pub id: String,
    /// The marker as printed (`(61)`, `[703]`).
    pub marker: String,
    /// The paragraph of the section the marker stood in, 0-based.
    pub paragraph: u32,
    /// Scalar offset in that paragraph where the marker stood.
    pub scalar: u32,
    /// The note's text.
    pub text: String,
    /// Its kind.
    pub kind: NoteKind,
}

/// One paragraph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkParagraph {
    /// Layout.
    pub style: ParagraphStyle,
    /// The text, without note markers.
    pub text: String,
}

/// One section with its text and notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkSection {
    /// `3.2.7`, `dedication`, `1.argument`.
    pub locator: String,
    /// Position in reading order, from 1.
    pub ordinal: u32,
    /// The chapter-level unit it belongs to.
    pub chapter: String,
    /// The printed section number, if numbered.
    pub number: Option<u32>,
    /// The printed label (`VII.`); none for a chapter's first section, which the source does
    /// not number.
    pub label: Option<String>,
    /// The citation people write for it (`Inst. 3.2.7`), for a numbered section.
    pub citation: Option<String>,
    /// Paragraphs in order.
    pub paragraphs: Vec<WorkParagraph>,
    /// Notes in marker order.
    pub notes: Vec<WorkNote>,
}

/// A chapter with its sections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkChapter {
    /// The division it belongs to (`front`, `3`).
    pub division: String,
    /// The chapter.
    pub info: WorkChapterInfo,
    /// Its sections.
    pub sections: Vec<WorkSection>,
}

/// Whether a citation resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CitationStatus {
    /// It names a Bible passage that exists in the scheme.
    Resolved,
    /// It does not, or not without a guess; `class` says why.
    Unresolved,
}

impl CitationStatus {
    fn name(self) -> &'static str {
        match self {
            CitationStatus::Resolved => "resolved",
            CitationStatus::Unresolved => "unresolved",
        }
    }

    fn parse(text: &str) -> CitationStatus {
        if text == "resolved" {
            CitationStatus::Resolved
        } else {
            CitationStatus::Unresolved
        }
    }
}

/// One citation found in a note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkCitationLink {
    /// The citing section.
    pub section: String,
    /// The note it is in.
    pub note: String,
    /// As printed.
    pub text: String,
    /// Scalar range of `text` in the note.
    pub start: u32,
    /// End of that range, exclusive.
    pub end: u32,
    /// Resolved or not.
    pub status: CitationStatus,
    /// The passage, OSIS in [`CITATION_SCHEME`], when resolved.
    pub target: Option<String>,
    /// Why it did not resolve (`unknown_book`, `ambiguous_book`, …); `None` when resolved.
    pub class: Option<String>,
    /// A remark (the parser's error, `and following`, a chapter-only citation).
    pub detail: Option<String>,
}

/// A section that cites a passage: what "cited in Calvin" lists under a verse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitingSection {
    /// The citing section's locator.
    pub section: String,
    /// How people cite it (`Inst. 3.2.7`), or its chapter title for front matter.
    pub label: String,
    /// The title of its chapter, if any.
    pub chapter_title: Option<String>,
    /// The citation as printed.
    pub text: String,
    /// Its note.
    pub note: String,
    /// The whole cited passage, OSIS.
    pub target: String,
}

/// Errors of the work pack.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkPackError {
    /// The file or bytes are not a work pack this reader knows.
    #[error("not a work pack: {detail}")]
    NotAWorkPack {
        /// Why.
        detail: String,
    },
    /// No section or chapter has this locator.
    #[error("`{locator}` is not a place in this work")]
    NotFound {
        /// The locator as asked.
        locator: String,
    },
    /// A passage that does not parse.
    #[error("`{text}` is not a passage: {detail}")]
    Passage {
        /// As given.
        text: String,
        /// The parser's reason.
        detail: String,
    },
    /// SQLite or JSON failed.
    #[error("storage: {detail}")]
    Storage {
        /// What failed.
        detail: String,
    },
}

/// The result type of this module.
pub type WorkResult<T> = Result<T, WorkPackError>;

fn storage(error: impl std::fmt::Display) -> WorkPackError {
    WorkPackError::Storage {
        detail: error.to_string(),
    }
}

/// What a section stores besides its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SectionMeta {
    number: Option<u32>,
    label: Option<String>,
    citation: Option<String>,
    styles: Vec<ParagraphStyle>,
    notes: Vec<WorkNote>,
}

/// A Bible passage as decimal verse-id ranges cut at chapter ends, in [`CITATION_SCHEME`].
/// A chapter-only reference covers the chapter's verses. `None` when a chapter or verse is not
/// in the scheme.
#[must_use]
pub fn verse_spans(set: &openlogos_core::reference::ReferenceSet) -> Option<Vec<(u32, u32)>> {
    let scheme = SchemeId::new(CITATION_SCHEME);
    let mut spans = Vec::new();
    for range in &set.ranges {
        let (start, end) = (&range.start, &range.end);
        let first_of = |r: &Reference| -> Option<u32> {
            match r.verse {
                Some(v) => (v <= canon::last_verse(&scheme, r.book, r.chapter)?).then_some(v),
                None => canon::first_verse(&scheme, r.book, r.chapter),
            }
        };
        let last_of = |r: &Reference| -> Option<u32> {
            let last = canon::last_verse(&scheme, r.book, r.chapter)?;
            match r.verse {
                Some(v) => (v <= last).then_some(v),
                None => Some(last),
            }
        };
        let first = first_of(start)?;
        let last = last_of(end)?;
        let id = |book: BookId, chapter: u32, verse: u32| {
            VerseId::new(book.number(), chapter as u16, verse as u16).0
        };
        if start.book != end.book {
            // A range across books is not cut further: the citation's two ends are kept.
            spans.push((
                id(start.book, start.chapter, first),
                id(
                    start.book,
                    start.chapter,
                    canon::last_verse(&scheme, start.book, start.chapter)?,
                ),
            ));
            spans.push((
                id(
                    end.book,
                    end.chapter,
                    canon::first_verse(&scheme, end.book, end.chapter)?,
                ),
                id(end.book, end.chapter, last),
            ));
            continue;
        }
        for chapter in start.chapter..=end.chapter {
            let from = if chapter == start.chapter {
                first
            } else {
                canon::first_verse(&scheme, start.book, chapter)?
            };
            let to = if chapter == end.chapter {
                last
            } else {
                canon::last_verse(&scheme, start.book, chapter)?
            };
            if from > to {
                return None;
            }
            spans.push((id(start.book, chapter, from), id(start.book, chapter, to)));
        }
    }
    Some(spans)
}

/// One citation as the writer receives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationRow {
    /// The citation.
    pub link: WorkCitationLink,
    /// Its ranges as verse ids (see [`verse_spans`]); empty when unresolved.
    pub spans: Vec<(u32, u32)>,
}

/// Everything a work pack holds, as the importer assembles it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkContent {
    /// Identity, rights, counts.
    pub info: WorkInfo,
    /// Divisions and chapters.
    pub structure: Vec<WorkDivision>,
    /// Sections in reading order; ordinals must run 1, 2, 3 ….
    pub sections: Vec<WorkSection>,
    /// Citations in section order.
    pub citations: Vec<CitationRow>,
    /// What produced the citations (`b016-allen-notes@1 + core reference parser`).
    pub citation_method: String,
}

fn locator_of_span(first: u32, last: u32) -> String {
    if first == last {
        first.to_string()
    } else {
        format!("{first}-{last}")
    }
}

/// Writes `content` as a new pack file at `path` (which must not exist).
pub fn write_work_pack(path: &Path, content: &WorkContent) -> WorkResult<()> {
    use crate::pack::{EdgeRow, PackBuilder, PackManifest};
    use openlogos_retrieval::{ProvenanceKind, ReviewState};

    let info = &content.info;
    for (i, section) in content.sections.iter().enumerate() {
        if section.ordinal as usize != i + 1 {
            return Err(storage(format!(
                "section {} has ordinal {}, expected {}",
                section.locator,
                section.ordinal,
                i + 1
            )));
        }
    }
    let manifest = PackManifest {
        edition: info.resource_id.clone(),
        scheme: "none".to_owned(),
        source: info.source.clone(),
        license: info.license.clone(),
        attribution: info.attribution.clone(),
        work: Some(info.work.clone()),
        locator_scheme: Some(info.locator_scheme.clone()),
    };
    let mut builder = PackBuilder::create(path, &manifest, &[]).map_err(storage)?;
    for section in &content.sections {
        let body = section
            .paragraphs
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join(PARAGRAPH_SEPARATOR);
        builder
            .add_verse(VerseId(section.ordinal), &body)
            .map_err(storage)?;
        if section.number.is_some() {
            builder
                .add_section_map(&SectionMapRow {
                    scheme: info.locator_scheme.clone(),
                    locator: section.locator.clone(),
                    edition: info.resource_id.clone(),
                    segment: Some(section.locator.clone()),
                    kind: SectionMapKind::Equal,
                })
                .map_err(storage)?;
        }
    }
    for row in &content.citations {
        for (first, last) in &row.spans {
            builder
                .add_edge(&EdgeRow {
                    src_work: &info.work,
                    src_locator: &row.link.section,
                    rel: CITATION_RELATION,
                    dst_work: BIBLE_WORK,
                    dst_locator: &locator_of_span(*first, *last),
                    weight: 0.0,
                    source_work: &info.resource_id,
                    method: &content.citation_method,
                    provenance: ProvenanceKind::Extracted,
                    review_state: ReviewState::SourceAttested,
                    context: Some(&row.link.text),
                })
                .map_err(storage)?;
        }
    }
    builder.finish().map_err(storage)?;

    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(storage)?;
    conn.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; BEGIN;")
        .map_err(storage)?;
    conn.execute_batch(SCHEMA).map_err(storage)?;
    for (key, value) in [
        ("work_format", WORK_FORMAT.to_owned()),
        ("work_info", serde_json::to_string(info).map_err(storage)?),
        (
            "work_structure",
            serde_json::to_string(&content.structure).map_err(storage)?,
        ),
    ] {
        conn.execute(
            "INSERT INTO manifest(key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .map_err(storage)?;
    }
    {
        let mut insert = conn
            .prepare(
                "INSERT INTO work_section(ordinal, locator, chapter, meta) VALUES (?1, ?2, ?3, ?4)",
            )
            .map_err(storage)?;
        for section in &content.sections {
            let meta = SectionMeta {
                number: section.number,
                label: section.label.clone(),
                citation: section.citation.clone(),
                styles: section.paragraphs.iter().map(|p| p.style).collect(),
                notes: section.notes.clone(),
            };
            insert
                .execute(params![
                    section.ordinal,
                    section.locator,
                    section.chapter,
                    serde_json::to_string(&meta).map_err(storage)?
                ])
                .map_err(storage)?;
        }
        let ordinals: BTreeMap<&str, u32> = content
            .sections
            .iter()
            .map(|s| (s.locator.as_str(), s.ordinal))
            .collect();
        let mut insert_citation = conn
            .prepare(
                "INSERT INTO work_citation(id, ordinal, note, text, start, end, status, target, class, detail)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .map_err(storage)?;
        let mut insert_span = conn
            .prepare("INSERT OR IGNORE INTO cite_span(first, last, citation) VALUES (?1, ?2, ?3)")
            .map_err(storage)?;
        for (i, row) in content.citations.iter().enumerate() {
            let id = i as i64 + 1;
            let link = &row.link;
            let ordinal = ordinals
                .get(link.section.as_str())
                .ok_or_else(|| storage(format!("citation in unknown section {}", link.section)))?;
            insert_citation
                .execute(params![
                    id,
                    ordinal,
                    link.note,
                    link.text,
                    link.start,
                    link.end,
                    link.status.name(),
                    link.target,
                    link.class,
                    link.detail
                ])
                .map_err(storage)?;
            for (first, last) in &row.spans {
                insert_span
                    .execute(params![first, last, id])
                    .map_err(storage)?;
            }
        }
    }
    conn.execute_batch("COMMIT; VACUUM;").map_err(storage)?;
    conn.close().map_err(|(_, e)| storage(e))
}

/// A work pack opened for reading. Holds one read-only connection.
pub struct WorkPack {
    conn: Connection,
    info: WorkInfo,
    structure: Vec<WorkDivision>,
    /// Chapter locator → (division locator, index in it).
    chapters: BTreeMap<String, (usize, usize)>,
}

impl std::fmt::Debug for WorkPack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkPack")
            .field("resource_id", &self.info.resource_id)
            .finish_non_exhaustive()
    }
}

fn english() -> &'static AliasTable {
    static TABLE: std::sync::OnceLock<AliasTable> = std::sync::OnceLock::new();
    TABLE.get_or_init(AliasTable::english)
}

fn grammars() -> &'static GrammarTable {
    static TABLE: std::sync::OnceLock<GrammarTable> = std::sync::OnceLock::new();
    TABLE.get_or_init(GrammarTable::builtin)
}

impl WorkPack {
    /// Opens a pack file read-only.
    pub fn open(path: &Path) -> WorkResult<WorkPack> {
        let uri = format!(
            "file:{}?immutable=1",
            path.display()
                .to_string()
                .replace('%', "%25")
                .replace('?', "%3F")
                .replace('#', "%23")
        );
        let conn = Connection::open_with_flags(
            uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(storage)?;
        WorkPack::adopt(conn)
    }

    /// Opens a pack from its bytes (a browser's fetch); they are copied into SQLite's memory.
    pub fn from_bytes(bytes: &[u8]) -> WorkResult<WorkPack> {
        if !bytes.starts_with(b"SQLite format 3\0") {
            return Err(WorkPackError::NotAWorkPack {
                detail: "these bytes are not a SQLite pack".to_owned(),
            });
        }
        let mut conn = Connection::open_in_memory().map_err(storage)?;
        conn.deserialize_read_exact("main", Cursor::new(bytes), bytes.len(), true)
            .map_err(storage)?;
        WorkPack::adopt(conn)
    }

    fn adopt(conn: Connection) -> WorkResult<WorkPack> {
        register_tokenizer(&conn).map_err(storage)?;
        let value = |key: &str| -> WorkResult<Option<String>> {
            conn.query_row(
                "SELECT value FROM manifest WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| WorkPackError::NotAWorkPack {
                detail: e.to_string(),
            })
        };
        let not = |detail: String| WorkPackError::NotAWorkPack { detail };
        let version = value("schema_version")?.unwrap_or_default();
        if version != PACK_SCHEMA_VERSION {
            return Err(not(format!(
                "pack schema {version} is not the supported {PACK_SCHEMA_VERSION}"
            )));
        }
        let format = value("work_format")?.unwrap_or_default();
        if format != WORK_FORMAT {
            return Err(not(format!("work format `{format}` is not {WORK_FORMAT}")));
        }
        let info: WorkInfo = serde_json::from_str(&value("work_info")?.unwrap_or_default())
            .map_err(|e| not(format!("work_info: {e}")))?;
        let structure: Vec<WorkDivision> =
            serde_json::from_str(&value("work_structure")?.unwrap_or_default())
                .map_err(|e| not(format!("work_structure: {e}")))?;
        let mut chapters = BTreeMap::new();
        for (d, division) in structure.iter().enumerate() {
            for (c, chapter) in division.chapters.iter().enumerate() {
                chapters.insert(chapter.locator.clone(), (d, c));
            }
        }
        Ok(WorkPack {
            conn,
            info,
            structure,
            chapters,
        })
    }

    /// Identity, rights, bibliography and counts.
    #[must_use]
    pub fn info(&self) -> &WorkInfo {
        &self.info
    }

    /// Divisions, chapters and the locators of their sections, in reading order.
    #[must_use]
    pub fn structure(&self) -> &[WorkDivision] {
        &self.structure
    }

    /// A locator as people write it, in this work's canonical form: `Inst. 3.2.7`,
    /// `Institutes III.ii.7`, `institutes:3.2.7` and `3.2.7` are all `3.2.7`; a front-matter
    /// locator (`dedication`) is itself. A citation of another work is not found.
    pub fn canonical_locator(&self, input: &str) -> WorkResult<String> {
        let text = input.trim();
        let scheme = &self.info.locator_scheme;
        if let Some(rest) = text.strip_prefix(&format!("{scheme}:")) {
            return Ok(rest.to_owned());
        }
        match parse_work_citation(text, grammars()) {
            Ok(citation) if citation.scheme == *scheme => {
                let range = citation
                    .ranges
                    .first()
                    .ok_or_else(|| WorkPackError::NotFound {
                        locator: input.to_owned(),
                    })?;
                Ok(range.start.to_string())
            }
            Ok(_) => Err(WorkPackError::NotFound {
                locator: input.to_owned(),
            }),
            Err(_) => WorkLocator::parse(text)
                .map(|l| l.to_string())
                .map_err(|_| WorkPackError::NotFound {
                    locator: input.to_owned(),
                }),
        }
    }

    fn section_row(
        &self,
        sql_where: &str,
        key: &dyn rusqlite::ToSql,
    ) -> WorkResult<Option<WorkSection>> {
        let row: Option<(u32, String, String, String, String)> = self
            .conn
            .prepare_cached(&format!(
                "SELECT s.ordinal, s.locator, s.chapter, s.meta, t.body
                 FROM work_section s JOIN text t ON t.verse_id = s.ordinal WHERE {sql_where}"
            ))
            .map_err(storage)?
            .query_row(params![key], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .optional()
            .map_err(storage)?;
        row.map(|(ordinal, locator, chapter, meta, body)| {
            section_from(ordinal, locator, chapter, &meta, &body)
        })
        .transpose()
    }

    /// One section by locator (any form [`WorkPack::canonical_locator`] reads).
    pub fn section(&self, locator: &str) -> WorkResult<WorkSection> {
        let canonical = self.canonical_locator(locator)?;
        self.section_row("s.locator = ?1", &canonical)?
            .ok_or_else(|| WorkPackError::NotFound {
                locator: locator.to_owned(),
            })
    }

    /// The section at an ordinal (reading order, from 1).
    pub fn section_at(&self, ordinal: u32) -> WorkResult<WorkSection> {
        self.section_row("s.ordinal = ?1", &ordinal)?
            .ok_or_else(|| WorkPackError::NotFound {
                locator: format!("#{ordinal}"),
            })
    }

    /// A chapter-level unit with all its sections: `3.2`, `1.argument`, `dedication`. A
    /// section's locator opens the chapter that holds it.
    pub fn chapter(&self, locator: &str) -> WorkResult<WorkChapter> {
        let canonical = self.canonical_locator(locator)?;
        let key = if self.chapters.contains_key(&canonical) {
            canonical
        } else {
            let section = self.section(&canonical)?;
            section.chapter
        };
        let (d, c) = self.chapters[&key];
        let division = &self.structure[d];
        let info = division.chapters[c].clone();
        let mut sections = Vec::with_capacity(info.sections.len());
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT s.ordinal, s.locator, s.chapter, s.meta, t.body
                 FROM work_section s JOIN text t ON t.verse_id = s.ordinal
                 WHERE s.chapter = ?1 ORDER BY s.ordinal",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![key], |r| {
                Ok((
                    r.get::<_, u32>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(storage)?;
        for row in rows {
            let (ordinal, locator, chapter, meta, body) = row.map_err(storage)?;
            sections.push(section_from(ordinal, locator, chapter, &meta, &body)?);
        }
        Ok(WorkChapter {
            division: division.locator.clone(),
            info,
            sections,
        })
    }

    /// The citations found in a section's notes, resolved and unresolved, in order.
    pub fn citations(&self, locator: &str) -> WorkResult<Vec<WorkCitationLink>> {
        let section = self.section(locator)?;
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT note, text, start, end, status, target, class, detail FROM work_citation
                 WHERE ordinal = ?1 ORDER BY id",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![section.ordinal], |r| {
                Ok(WorkCitationLink {
                    section: section.locator.clone(),
                    note: r.get(0)?,
                    text: r.get(1)?,
                    start: r.get(2)?,
                    end: r.get(3)?,
                    status: CitationStatus::parse(&r.get::<_, String>(4)?),
                    target: r.get(5)?,
                    class: r.get(6)?,
                    detail: r.get(7)?,
                })
            })
            .map_err(storage)?;
        rows.collect::<Result<_, _>>().map_err(storage)
    }

    /// Every citation of the work, in section order: what the evidence counts.
    pub fn all_citations(&self) -> WorkResult<Vec<WorkCitationLink>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT s.locator, c.note, c.text, c.start, c.end, c.status, c.target, c.class, c.detail
                 FROM work_citation c JOIN work_section s ON s.ordinal = c.ordinal ORDER BY c.id",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(WorkCitationLink {
                    section: r.get(0)?,
                    note: r.get(1)?,
                    text: r.get(2)?,
                    start: r.get(3)?,
                    end: r.get(4)?,
                    status: CitationStatus::parse(&r.get::<_, String>(5)?),
                    target: r.get(6)?,
                    class: r.get(7)?,
                    detail: r.get(8)?,
                })
            })
            .map_err(storage)?;
        rows.collect::<Result<_, _>>().map_err(storage)
    }

    /// The sections whose notes cite a passage or any verse inside it (the reverse link index
    /// of B068): `Rom 8:28`, `Rom.8.28`, `Ps 23`. `scheme` is the passage's versification
    /// (`org` for the WLC); it is mapped into [`CITATION_SCHEME`] verse by verse first. One row
    /// per citation, in reading order.
    pub fn cited_by(&self, passage: &str, scheme: Option<&str>) -> WorkResult<Vec<CitingSection>> {
        let set = parse(passage, english()).map_err(|e| WorkPackError::Passage {
            text: passage.to_owned(),
            detail: e.to_string(),
        })?;
        let set = match scheme {
            Some(s) if s != CITATION_SCHEME => map_set(&set, s),
            _ => set,
        };
        let spans = verse_spans(&set).ok_or_else(|| WorkPackError::Passage {
            text: passage.to_owned(),
            detail: format!("not a passage of the `{CITATION_SCHEME}` scheme"),
        })?;
        let mut ids = std::collections::BTreeSet::new();
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT citation FROM cite_span WHERE first BETWEEN ?1 AND ?2 AND last >= ?3",
            )
            .map_err(storage)?;
        for (first, last) in spans {
            // Spans never cross a chapter, so a span touching [first, last] starts in its chapter.
            let chapter_base = first / 1000 * 1000;
            let rows = stmt
                .query_map(params![chapter_base, last, first], |r| r.get::<_, i64>(0))
                .map_err(storage)?;
            for row in rows {
                ids.insert(row.map_err(storage)?);
            }
        }
        let mut out = Vec::with_capacity(ids.len());
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT s.locator, s.chapter, s.meta, c.text, c.note, c.target
                 FROM work_citation c JOIN work_section s ON s.ordinal = c.ordinal WHERE c.id = ?1",
            )
            .map_err(storage)?;
        let mut rows: Vec<(u32, CitingSection)> = Vec::new();
        for id in ids {
            let (locator, chapter, meta, text, note, target): (
                String,
                String,
                String,
                String,
                String,
                Option<String>,
            ) = stmt
                .query_row(params![id], |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                })
                .map_err(storage)?;
            let meta: SectionMeta = serde_json::from_str(&meta).map_err(storage)?;
            let chapter_title = self
                .chapters
                .get(&chapter)
                .and_then(|(d, c)| self.structure[*d].chapters[*c].title.clone());
            let label = meta
                .citation
                .clone()
                .or_else(|| chapter_title.clone())
                .unwrap_or_else(|| locator.clone());
            rows.push((
                u32::try_from(id).unwrap_or(u32::MAX),
                CitingSection {
                    section: locator,
                    label,
                    chapter_title,
                    text,
                    note,
                    target: target.unwrap_or_default(),
                },
            ));
        }
        rows.sort_by_key(|(id, _)| *id);
        out.extend(rows.into_iter().map(|(_, row)| row));
        Ok(out)
    }

    /// How many sections hold a citation of the passage (see [`WorkPack::cited_by`]).
    pub fn cited_count(&self, passage: &str, scheme: Option<&str>) -> WorkResult<u32> {
        let rows = self.cited_by(passage, scheme)?;
        let sections: std::collections::BTreeSet<&str> =
            rows.iter().map(|r| r.section.as_str()).collect();
        Ok(sections.len() as u32)
    }
}

/// Each verse of `set` mapped from `scheme` into [`CITATION_SCHEME`]; a verse without a
/// counterpart is kept as written.
fn map_set(
    set: &openlogos_core::reference::ReferenceSet,
    scheme: &str,
) -> openlogos_core::reference::ReferenceSet {
    let from = SchemeId::new(scheme);
    let to = SchemeId::new(CITATION_SCHEME);
    let map_one = |r: &Reference| -> Reference {
        if r.verse.is_none() {
            return r.clone();
        }
        canon::map(r, &from, &to)
            .targets
            .first()
            .cloned()
            .unwrap_or_else(|| r.clone())
    };
    openlogos_core::reference::ReferenceSet {
        ranges: set
            .ranges
            .iter()
            .map(|range| openlogos_core::reference::ReferenceRange {
                start: map_one(&range.start),
                end: map_one(&range.end),
            })
            .collect(),
        flags: set.flags.clone(),
    }
}

fn section_from(
    ordinal: u32,
    locator: String,
    chapter: String,
    meta: &str,
    body: &str,
) -> WorkResult<WorkSection> {
    let meta: SectionMeta = serde_json::from_str(meta).map_err(storage)?;
    let texts: Vec<&str> = body.split(PARAGRAPH_SEPARATOR).collect();
    if texts.len() != meta.styles.len() {
        return Err(storage(format!(
            "section {locator}: {} paragraphs, {} styles",
            texts.len(),
            meta.styles.len()
        )));
    }
    Ok(WorkSection {
        locator,
        ordinal,
        chapter,
        number: meta.number,
        label: meta.label,
        citation: meta.citation,
        paragraphs: texts
            .into_iter()
            .zip(meta.styles)
            .map(|(text, style)| WorkParagraph {
                style,
                text: text.to_owned(),
            })
            .collect(),
        notes: meta.notes,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A hand-written work in the pack's shape; none of this is the Institutes' text.
    pub(crate) fn content() -> WorkContent {
        let paragraph = |text: &str| WorkParagraph {
            style: ParagraphStyle::Prose,
            text: text.to_owned(),
        };
        let section =
            |ordinal: u32, locator: &str, chapter: &str, number: Option<u32>, texts: &[&str]| {
                WorkSection {
                    locator: locator.to_owned(),
                    ordinal,
                    chapter: chapter.to_owned(),
                    number,
                    label: number.filter(|n| *n > 1).map(|n| format!("{n}.")),
                    citation: number.map(|_| format!("Institutes {locator}")),
                    paragraphs: texts.iter().map(|t| paragraph(t)).collect(),
                    notes: Vec::new(),
                }
            };
        let mut first = section(
            2,
            "3.2.1",
            "3.2",
            Some(1),
            &["Faith is a firm knowledge.", "A second paragraph."],
        );
        first.notes.push(WorkNote {
            id: "1:1".to_owned(),
            marker: "(1)".to_owned(),
            paragraph: 0,
            scalar: 26,
            text: "Rom. viii. 28‐30. Psalm li.".to_owned(),
            kind: NoteKind::Citations,
        });
        let sections = vec![
            section(
                1,
                "dedication",
                "dedication",
                None,
                &["To the king, a letter about faith."],
            ),
            first,
            section(
                3,
                "3.2.2",
                "3.2",
                Some(2),
                &["2. The second section, on light."],
            ),
        ];
        let link =
            |section: &str, text: &str, status, target: Option<&str>, class: Option<&str>| {
                WorkCitationLink {
                    section: section.to_owned(),
                    note: "1:1".to_owned(),
                    text: text.to_owned(),
                    start: 0,
                    end: text.chars().count() as u32,
                    status,
                    target: target.map(str::to_owned),
                    class: class.map(str::to_owned),
                    detail: None,
                }
            };
        let rom = parse("Rom 8:28-30", &AliasTable::english()).unwrap();
        let ps = parse("Ps 51", &AliasTable::english()).unwrap();
        let citations = vec![
            CitationRow {
                link: link(
                    "3.2.1",
                    "Rom. viii. 28‐30",
                    CitationStatus::Resolved,
                    Some("Rom.8.28-Rom.8.30"),
                    None,
                ),
                spans: verse_spans(&rom).unwrap(),
            },
            CitationRow {
                link: link(
                    "3.2.1",
                    "Psalm li",
                    CitationStatus::Resolved,
                    Some("Ps.51"),
                    None,
                ),
                spans: verse_spans(&ps).unwrap(),
            },
            CitationRow {
                link: link(
                    "3.2.2",
                    "Like viii. 5",
                    CitationStatus::Unresolved,
                    None,
                    Some("unknown_name"),
                ),
                spans: Vec::new(),
            },
        ];
        WorkContent {
            info: WorkInfo {
                resource_id: "test-work".to_owned(),
                work: "institutes".to_owned(),
                locator_scheme: "institutes".to_owned(),
                content_revision: "r1".to_owned(),
                title: "A Test Work".to_owned(),
                short_title: "Test".to_owned(),
                language: "en".to_owned(),
                source_id: "S00".to_owned(),
                license: "CC0-1.0".to_owned(),
                attribution: "Test".to_owned(),
                source: "hand-written".to_owned(),
                bibliography: WorkBibliography::default(),
                sections: 3,
                notes: 1,
                citations: 3,
                resolved: 2,
            },
            structure: vec![
                WorkDivision {
                    locator: "front".to_owned(),
                    kind: DivisionKind::Front,
                    label: None,
                    title: None,
                    chapters: vec![WorkChapterInfo {
                        locator: "dedication".to_owned(),
                        kind: ChapterKind::Front,
                        label: None,
                        title: Some("DEDICATION.".to_owned()),
                        sections: vec!["dedication".to_owned()],
                    }],
                },
                WorkDivision {
                    locator: "3".to_owned(),
                    kind: DivisionKind::Book,
                    label: Some("BOOK III.".to_owned()),
                    title: None,
                    chapters: vec![WorkChapterInfo {
                        locator: "3.2".to_owned(),
                        kind: ChapterKind::Chapter,
                        label: Some("Chapter II.".to_owned()),
                        title: Some("Faith.".to_owned()),
                        sections: vec!["3.2.1".to_owned(), "3.2.2".to_owned()],
                    }],
                },
            ],
            sections,
            citations,
            citation_method: "test@1".to_owned(),
        }
    }

    pub(crate) fn temp_pack(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("openlogos-work-pack-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test-work.sqlite");
        write_work_pack(&path, &content()).unwrap();
        path
    }

    #[test]
    fn a_written_pack_reads_back_section_for_section() {
        let path = temp_pack("roundtrip");
        let pack = WorkPack::open(&path).unwrap();
        let content = content();
        assert_eq!(pack.info(), &content.info);
        assert_eq!(pack.structure(), content.structure.as_slice());
        for section in &content.sections {
            assert_eq!(&pack.section(&section.locator).unwrap(), section);
        }
        assert_eq!(pack.section_at(2).unwrap().locator, "3.2.1");
        let chapter = pack.chapter("3.2").unwrap();
        assert_eq!(chapter.division, "3");
        assert_eq!(chapter.sections.len(), 2);
        assert_eq!(pack.chapter("3.2.2").unwrap().info.locator, "3.2");
        assert_eq!(pack.chapter("dedication").unwrap().sections[0].ordinal, 1);
    }

    #[test]
    fn a_locator_is_read_in_every_form_people_write() {
        let pack = WorkPack::open(&temp_pack("locators")).unwrap();
        for form in [
            "Inst. 3.2.1",
            "Institutes III.ii.1",
            "institutes:3.2.1",
            "3.2.1",
            " 3.2.1 ",
        ] {
            assert_eq!(pack.section(form).unwrap().locator, "3.2.1", "{form}");
        }
        assert!(matches!(
            pack.section("WCF 1.6"),
            Err(WorkPackError::NotFound { .. })
        ));
        assert!(matches!(
            pack.section("3.2.9"),
            Err(WorkPackError::NotFound { .. })
        ));
        assert!(matches!(
            pack.chapter("9.9"),
            Err(WorkPackError::NotFound { .. })
        ));
    }

    #[test]
    fn citations_keep_the_unresolved_and_the_reverse_index_finds_verses_inside_ranges() {
        let pack = WorkPack::open(&temp_pack("citations")).unwrap();
        let cites = pack.citations("3.2.1").unwrap();
        assert_eq!(cites.len(), 2);
        let unresolved = pack.citations("3.2.2").unwrap();
        assert_eq!(unresolved[0].status, CitationStatus::Unresolved);
        assert_eq!(unresolved[0].class.as_deref(), Some("unknown_name"));
        assert_eq!(pack.all_citations().unwrap().len(), 3);
        for verse in ["Rom 8:28", "Rom.8.29", "Romans 8:30"] {
            let rows = pack.cited_by(verse, None).unwrap();
            assert_eq!(rows.len(), 1, "{verse}");
            assert_eq!(rows[0].section, "3.2.1");
            assert_eq!(rows[0].label, "Institutes 3.2.1");
            assert_eq!(rows[0].chapter_title.as_deref(), Some("Faith."));
            assert_eq!(rows[0].target, "Rom.8.28-Rom.8.30");
        }
        assert!(pack.cited_by("Rom 8:31", None).unwrap().is_empty());
        assert!(pack.cited_by("Rom 8:27", None).unwrap().is_empty());
        // A chapter asked for finds what cites any verse of it; a whole-chapter citation is
        // found from any verse.
        assert_eq!(pack.cited_by("Rom 8", None).unwrap().len(), 1);
        assert_eq!(pack.cited_by("Ps 51:10", None).unwrap()[0].target, "Ps.51");
        // Hebrew numbering: Ps 51:12 in `org` is 51:10 in `eng`.
        assert_eq!(pack.cited_by("Ps 51:12", Some("org")).unwrap().len(), 1);
        assert_eq!(pack.cited_count("Rom 8:28-30", None).unwrap(), 1);
        assert!(matches!(
            pack.cited_by("Nowhere 1:1", None),
            Err(WorkPackError::Passage { .. })
        ));
    }

    #[test]
    fn bytes_answer_the_same_as_the_file_and_other_bytes_are_refused() {
        let path = temp_pack("bytes");
        let file = WorkPack::open(&path).unwrap();
        let memory = WorkPack::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(file.chapter("3.2").unwrap(), memory.chapter("3.2").unwrap());
        assert_eq!(
            file.cited_by("Rom 8:29", None).unwrap(),
            memory.cited_by("Rom 8:29", None).unwrap()
        );
        assert!(WorkPack::from_bytes(b"not a pack").is_err());
        // A Bible search pack is a pack but not a work pack.
        let bible = std::env::temp_dir().join(format!(
            "openlogos-work-pack-bible-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&bible);
        crate::pack::fixture::build(&bible).unwrap();
        assert!(matches!(
            WorkPack::open(&bible),
            Err(WorkPackError::NotAWorkPack { .. })
        ));
    }

    #[test]
    fn edges_and_section_maps_are_the_b068_shapes() {
        let path = temp_pack("edges");
        let conn = crate::pack::open_read_only(&path).unwrap();
        let edges: Vec<(String, String, String, String, String, String)> = conn
            .prepare("SELECT src_work, src_locator, rel, dst_work, dst_locator, context FROM edge ORDER BY rowid")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].0, "institutes");
        assert_eq!(edges[0].1, "3.2.1");
        assert_eq!(edges[0].2, CITATION_RELATION);
        assert_eq!(edges[0].3, BIBLE_WORK);
        assert_eq!(edges[0].4, "46008028-46008030");
        assert_eq!(edges[0].5, "Rom. viii. 28‐30");
        let maps: i64 = conn
            .query_row("SELECT count(*) FROM section_map", [], |r| r.get(0))
            .unwrap();
        assert_eq!(maps, 2, "numbered sections only");
        // The link index installs the pack's edges like any pack's.
        let mut index = crate::links::LinkIndex::in_memory().unwrap();
        assert_eq!(index.install_pack("test-work", &path).unwrap(), 2);
    }

    #[test]
    fn the_edition_search_answers_a_work_pack_with_section_locators() {
        use openlogos_workspaces::palette::TextProvider;
        let path = temp_pack("search");
        let mut search = crate::EditionSearch::new().unwrap();
        assert_eq!(search.attach_file(&path).unwrap(), "test-work");
        let hits = search.search("faith", 5).unwrap();
        let references: Vec<&str> = hits.iter().map(|h| h.reference.as_str()).collect();
        assert_eq!(references, ["institutes:dedication", "institutes:3.2.1"]);
        let second = search.search("second paragraph", 5).unwrap();
        assert_eq!(
            second[0].snippet,
            "Faith is a firm knowledge. A second paragraph."
        );
        let provider: &dyn TextProvider = &search;
        assert_eq!(provider.search("light", 5)[0].reference, "institutes:3.2.2");
        // The verse-shaped index leaves sections out.
        let verses = openlogos_retrieval::TextIndex::search_text(
            &search,
            &openlogos_retrieval::TextQuery {
                text: "faith".to_owned(),
                editions: Vec::new(),
                book: None,
                limit: 10,
            },
        )
        .unwrap();
        assert!(verses.is_empty());
        // Bytes attach the same way.
        let mut memory = crate::EditionSearch::new().unwrap();
        memory.attach_bytes(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(memory.search("faith", 5).unwrap(), hits);
    }
}
