//! Cross references for a passage in an installed edition (B057, specs/17 "Cross-reference
//! contract").
//!
//! A [`CrossReferenceSource`] holds published links in the source's own versification (the
//! OpenBible set is in `eng`) as [`CrossReferenceRow`]s: one row per published line, its
//! source verse, its target verse or range, and its votes exactly as published. Nothing here
//! scores, filters or invents links. [`ReadingLibrary::cross_references`] answers for a
//! passage in one edition:
//!
//! 1. The passage is located in the edition and each of its verses is mapped into the source's
//!    scheme by the canon table (WLC Mal 3:23 is `eng` Mal 4:5).
//! 2. The rows from those verses are looked up; with `include_reverse`, so are the rows that
//!    point *at* them, which become [`Direction::ReverseDerived`] links: a navigation aid,
//!    never a claim the source made.
//! 3. Each target is mapped into the edition's scheme, keeping a range a range, and checked
//!    against what the edition prints. A target the edition does not have, does not print, or
//!    that does not map is kept, with its [`Availability`] and [`MappingStatus`] saying so.
//! 4. Equal visible targets are merged into one [`CrossReference`]; every assertion behind it
//!    is kept in `assertions` with its own votes and source line.
//! 5. Published items come first, best first by their published votes (the best assertion's),
//!    then derived ones the same way, then canonical order of the target. Votes are not
//!    accuracy scores (S09).
//!
//! Preview text comes only from the edition asked about. A target it does not print shows no
//! text and lists the installed editions that do print it ([`CrossReference::available_in`]);
//! nothing silently shows a different translation.

use std::collections::{BTreeMap, HashMap};

use openlogos_core::canon;
use openlogos_core::reference::{BookId, Reference, ReferenceRange};
use openlogos_core::versification::{MappingStatus, SchemeId};
use serde::{Deserialize, Serialize};

use super::{ChapterVerse, ChunkRef, Installed, ReadingError, ReadingLibrary, english};

/// The longest passage, in verses, a single call looks up (a long chapter is 176).
pub const MAX_QUERY_VERSES: usize = 400;

/// What a cross-reference source is, as its pack's manifest states it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossReferenceInfo {
    /// The rights record (`S09`).
    pub source_id: String,
    /// The edition of the data (`openbible-cross-references:2026-10-26`).
    pub edition_id: String,
    /// The versification its references are in (`eng`).
    pub scheme: String,
    /// SPDX licence of the data.
    pub license: String,
    /// The attribution a reader must be shown.
    pub attribution: String,
    /// The relation type (`cross_reference`).
    pub relation: String,
    /// What kind of link the source says it is (`unspecified`: an imported traditional cross
    /// reference whose kind the source does not identify).
    pub kind: String,
    /// What the votes are, in the source's words.
    pub votes_meaning: String,
    /// The review state of every link (`source_attested`: as published, not reviewed here).
    pub review_state: String,
    /// What the data covers.
    pub coverage: String,
    /// Rows (published links).
    pub rows: u64,
}

/// One published link, in the source's scheme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossReferenceRow {
    /// Where it is in the source: the line of the published file.
    pub line: u32,
    /// The verse it is listed under.
    pub from: Reference,
    /// Its target: one verse, or the first verse of a range.
    pub to_start: Reference,
    /// The last verse of the target; equal to `to_start` for one verse.
    pub to_end: Reference,
    /// The votes as published; may be zero or negative.
    pub votes: i32,
}

/// Where cross references come from: a relation pack, or rows in memory for tests.
pub trait CrossReferenceSource: Send + Sync {
    /// What the source is.
    fn info(&self) -> &CrossReferenceInfo;

    /// The rows listed under a verse (in [`CrossReferenceInfo::scheme`]).
    fn outgoing(&self, verse: &Reference) -> Result<Vec<CrossReferenceRow>, String>;

    /// The rows whose target is or contains a verse.
    fn incoming(&self, verse: &Reference) -> Result<Vec<CrossReferenceRow>, String>;
}

/// Rows held in memory: tests, and a host that already has them.
#[derive(Debug, Clone)]
pub struct MemoryCrossReferences {
    info: CrossReferenceInfo,
    rows: Vec<CrossReferenceRow>,
}

impl MemoryCrossReferences {
    /// A source over these rows.
    #[must_use]
    pub fn new(info: CrossReferenceInfo, rows: Vec<CrossReferenceRow>) -> MemoryCrossReferences {
        MemoryCrossReferences { info, rows }
    }
}

impl CrossReferenceSource for MemoryCrossReferences {
    fn info(&self) -> &CrossReferenceInfo {
        &self.info
    }

    fn outgoing(&self, verse: &Reference) -> Result<Vec<CrossReferenceRow>, String> {
        Ok(self
            .rows
            .iter()
            .filter(|r| &r.from == verse)
            .cloned()
            .collect())
    }

    fn incoming(&self, verse: &Reference) -> Result<Vec<CrossReferenceRow>, String> {
        Ok(self
            .rows
            .iter()
            .filter(|r| &r.to_start <= verse && verse <= &r.to_end)
            .cloned()
            .collect())
    }
}

/// Which way a link was published.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Listed under the passage asked about, pointing at the target: what the source says.
    AsPublished,
    /// Listed under the target, pointing at the passage: shown the other way round as a
    /// navigation aid. Not a claim that the source asserts the link both ways.
    ReverseDerived,
}

/// Whether the edition asked about prints a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    /// Every verse of the target is printed in the edition.
    Printed,
    /// Some verses are printed, some are not (a range over a verse the edition omits).
    PartlyPrinted,
    /// The edition has the book but prints none of the target (SBLGNT Rom 16:25–27, BSB
    /// Matt 17:21).
    NotPrinted,
    /// The edition does not have the book (Genesis in the SBLGNT, John in the WLC).
    BookNotInEdition,
    /// The target does not map into the edition's versification.
    Unmapped,
}

/// One assertion behind a visible link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossReferenceAssertion {
    /// The verse the source lists it under, in the source's scheme (OSIS).
    pub from: String,
    /// Its target as published (OSIS, `A-B` for a range).
    pub to: String,
    /// The votes as published.
    pub votes: i32,
    /// The line of the published file.
    pub line: u32,
    /// Whether it is shown as published or turned round.
    pub direction: Direction,
}

/// One visible link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossReference {
    /// The target in the edition's own numbering, OSIS (`Mal.3.23`, `John.1.1-John.1.3`);
    /// what `tab.open_edition` opens. `None` when it does not map.
    pub target: Option<String>,
    /// The target for display, in the edition's numbering when it maps (`John 1:1–3`), else
    /// as published.
    pub label: String,
    /// The target as published, in the source's scheme (OSIS).
    pub source_target: String,
    /// The target as published, for display (`Mal 4:5`).
    pub source_label: String,
    /// How the published target maps into the edition's scheme.
    pub mapping: MappingStatus,
    /// Whether the edition prints it.
    pub availability: Availability,
    /// The edition's text of the target's first verses, from the installed pack; `None` when it
    /// prints none of it or the chunk is not loaded yet.
    pub preview: Option<String>,
    /// Whether the preview stops before the end of the target.
    pub preview_truncated: bool,
    /// The chunk a browser must supply before the preview can be read; `None` otherwise.
    pub preview_chunk: Option<ChunkRef>,
    /// The relation type (`cross_reference`).
    pub relation: String,
    /// The kind of link (`unspecified`): never upgraded to a citation or quotation.
    pub kind: String,
    /// [`Direction::AsPublished`] if any assertion is; else derived.
    pub direction: Direction,
    /// The best published votes among the assertions shown this way round (the best of all
    /// when every assertion is derived).
    pub votes: i32,
    /// Every assertion behind it, best first.
    pub assertions: Vec<CrossReferenceAssertion>,
    /// Other installed editions that print the target's first verse; where to read it when
    /// this edition does not.
    pub available_in: Vec<String>,
    /// The attribution the source's licence asks for.
    pub attribution: String,
}

/// The answer for one passage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossReferenceList {
    /// The edition.
    pub resource_id: String,
    /// The passage located in the edition, OSIS.
    pub reference: String,
    /// The passage's verses in the source's scheme, as looked up (OSIS).
    pub source_verses: Vec<String>,
    /// How the passage maps into the source's scheme: the worst of its verses.
    pub mapping: MappingStatus,
    /// The page asked for (`offset`, `limit`), in order.
    pub items: Vec<CrossReference>,
    /// Items in all.
    pub total: usize,
    /// Published (`as_published`) items in all.
    pub published: usize,
    /// The source.
    pub source: CrossReferenceInfo,
}

/// What to ask for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossReferenceOptions {
    /// Also show links listed under other verses that point at the passage, labelled
    /// [`Direction::ReverseDerived`].
    #[serde(default)]
    pub include_reverse: bool,
    /// At most this many items; `None` for all. A host asks for a page (25 fills a panel)
    /// and the next with `offset`: preview text is read only for the items returned.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Items to skip before the first returned, in the list's order.
    #[serde(default)]
    pub offset: usize,
    /// Verses of preview text per item (0 for none).
    #[serde(default = "default_preview_verses")]
    pub preview_verses: usize,
}

fn default_preview_verses() -> usize {
    3
}

impl Default for CrossReferenceOptions {
    fn default() -> Self {
        CrossReferenceOptions {
            include_reverse: false,
            limit: None,
            offset: 0,
            preview_verses: default_preview_verses(),
        }
    }
}

/// Why a cross-reference call failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CrossReferenceError {
    /// No cross-reference data is installed.
    #[error("Needs cross-reference data")]
    NotInstalled,
    /// The passage could not be located in the edition.
    #[error("{error}")]
    Reading {
        /// The reading error.
        error: ReadingError,
    },
    /// The passage is longer than [`MAX_QUERY_VERSES`].
    #[error("Cross references work on up to {max} verses; this passage has {verses}")]
    TooLong {
        /// Verses in the passage.
        verses: usize,
        /// The limit.
        max: usize,
    },
    /// The source failed to answer.
    #[error("The cross-reference data could not be read: {detail}")]
    Source {
        /// What failed.
        detail: String,
    },
}

impl From<ReadingError> for CrossReferenceError {
    fn from(error: ReadingError) -> Self {
        CrossReferenceError::Reading { error }
    }
}

/// A target in the edition: its ends, mapping and what is printed.
struct Placed {
    range: Option<ReferenceRange>,
    mapping: MappingStatus,
    availability: Availability,
}

fn worst(a: MappingStatus, b: MappingStatus) -> MappingStatus {
    a.max(b)
}

/// Maps one verse between schemes: the targets in order with the status.
fn map_verse(verse: &Reference, from: &str, to: &str) -> (MappingStatus, Vec<Reference>) {
    if from == to {
        return (MappingStatus::Exact, vec![verse.clone()]);
    }
    let result = canon::map(verse, &SchemeId::new(from), &SchemeId::new(to));
    (result.status, result.targets)
}

/// The verses of an edition's scheme from `start` to `end`, by the edition's books and
/// chapters, with whether each is printed. Bounded by `cap` verses.
fn walk(
    installed: &Installed,
    start: &Reference,
    end: &Reference,
    cap: usize,
) -> (Vec<(Reference, bool)>, bool) {
    let mut out = Vec::new();
    let (Some(first), Some(last)) = (start.verse, end.verse) else {
        return (out, false);
    };
    let books = installed.pack.books();
    let mut book_index = match installed.books.get(&start.book) {
        Some(i) => *i,
        None => return (out, false),
    };
    let mut chapter = start.chapter;
    let mut verse = first;
    loop {
        let shape = &books[book_index];
        let Some(c) = shape.chapters.iter().position(|c| c.chapter == chapter) else {
            return (out, false);
        };
        let ch = shape.chapters[c];
        let absent = installed.pack.absent_verses(shape.book, chapter);
        let at_end = shape.book == end.book && chapter == end.chapter;
        let stop = if at_end {
            last
        } else {
            ch.last_verse.max(verse)
        };
        for v in verse..=stop {
            let printed = (ch.first_verse..=ch.last_verse).contains(&v) && !absent.contains(&v);
            out.push((Reference::verse(shape.book, chapter, v), printed));
            if out.len() >= cap {
                return (out, true);
            }
        }
        if at_end {
            return (out, false);
        }
        if c + 1 < shape.chapters.len() {
            chapter = shape.chapters[c + 1].chapter;
            verse = shape.chapters[c + 1].first_verse;
        } else {
            book_index += 1;
            let Some(next) = books.get(book_index) else {
                return (out, false);
            };
            if next.book > end.book || next.chapters.is_empty() {
                return (out, false);
            }
            chapter = next.chapters[0].chapter;
            verse = next.chapters[0].first_verse;
        }
    }
}

/// Every verse of a scheme the canon table knows from `start` to `end` inclusive, crossing
/// chapters and books (`2Chr.36.22-Ezra.1.3`); `None` when an end is not in the scheme or the
/// range runs backwards.
#[must_use]
pub fn scheme_verses(scheme: &str, start: &Reference, end: &Reference) -> Option<Vec<Reference>> {
    let scheme = SchemeId::new(scheme);
    if !canon::contains(&scheme, start) || !canon::contains(&scheme, end) || end < start {
        return None;
    }
    let mut out = Vec::new();
    let mut book = start.book;
    let mut chapter = start.chapter;
    let mut verse = start.verse?;
    loop {
        let at_end = book == end.book && chapter == end.chapter;
        let stop = if at_end {
            end.verse?
        } else {
            canon::last_verse(&scheme, book, chapter)?
        };
        out.extend((verse..=stop).map(|v| Reference::verse(book, chapter, v)));
        if at_end {
            return Some(out);
        }
        if chapter < canon::chapter_count(&scheme, book)? {
            chapter += 1;
        } else {
            book = BookId::all()
                .into_iter()
                .find(|b| b.canonical_index() == book.canonical_index() + 1)?;
            chapter = 1;
        }
        verse = canon::first_verse(&scheme, book, chapter)?;
    }
}

/// The longest target walked for its availability; a published range is far shorter.
const WALK_CAP: usize = 2_000;

fn place(installed: &Installed, source_scheme: &str, start: &Reference, end: &Reference) -> Placed {
    let scheme = installed.pack.edition().scheme.as_str();
    let (mut s_status, mut s_targets) = map_verse(start, source_scheme, scheme);
    let (mut e_status, mut e_targets) = map_verse(end, source_scheme, scheme);
    // A range whose end (or start) has no counterpart, as `eng` Acts 19:41 has none in `org`,
    // keeps the part that maps and says it is partial; a single verse stays unmapped.
    if start != end && (s_targets.is_empty() || e_targets.is_empty()) {
        let inner = scheme_verses(source_scheme, start, end).unwrap_or_default();
        let mapped: Vec<(MappingStatus, Vec<Reference>)> = inner
            .iter()
            .map(|v| map_verse(v, source_scheme, scheme))
            .filter(|(_, t)| !t.is_empty())
            .collect();
        if let (Some(first), Some(last)) = (mapped.first(), mapped.last()) {
            if s_targets.is_empty() {
                (s_status, s_targets) = (MappingStatus::Partial, first.1.clone());
            }
            if e_targets.is_empty() {
                (e_status, e_targets) = (MappingStatus::Partial, last.1.clone());
            }
        }
    }
    let (Some(first), Some(last)) = (s_targets.first(), e_targets.last()) else {
        return Placed {
            range: None,
            mapping: MappingStatus::Unmapped,
            availability: Availability::Unmapped,
        };
    };
    let mapping = worst(s_status, e_status);
    let range = ReferenceRange {
        start: first.clone(),
        end: last.clone(),
    };
    let availability = if installed.book(first.book).is_none() {
        Availability::BookNotInEdition
    } else {
        let (verses, _) = walk(installed, first, last, WALK_CAP);
        let printed = verses.iter().filter(|(_, p)| *p).count();
        if verses.is_empty() || printed == 0 {
            Availability::NotPrinted
        } else if printed == verses.len() {
            Availability::Printed
        } else {
            Availability::PartlyPrinted
        }
    };
    Placed {
        range: Some(range),
        mapping,
        availability,
    }
}

type ChapterCache = HashMap<(BookId, u32), Result<Vec<ChapterVerse>, ReadingError>>;

/// The preview of a placed target: the first `n` printed verses, joined by a space.
fn preview(
    installed: &Installed,
    cache: &mut ChapterCache,
    range: &ReferenceRange,
    n: usize,
) -> (Option<String>, bool, Option<ChunkRef>) {
    if n == 0 {
        return (None, false, None);
    }
    let (verses, capped) = walk(installed, &range.start, &range.end, WALK_CAP);
    let printed: Vec<&Reference> = verses.iter().filter(|(_, p)| *p).map(|(r, _)| r).collect();
    let mut parts: Vec<String> = Vec::new();
    let mut last_segment: Option<String> = None;
    for r in printed.iter().take(n) {
        let key = (r.book, r.chapter);
        let chapter = cache
            .entry(key)
            .or_insert_with(|| installed.pack.verses(r.book, r.chapter));
        let verses = match chapter {
            Ok(v) => v,
            Err(ReadingError::ChunkNotLoaded { .. }) => {
                return (None, false, installed.pack.chunk(r.book, r.chapter));
            }
            Err(_) => return (None, false, None),
        };
        let number = r.verse.unwrap_or(0);
        if let Some(v) = verses
            .iter()
            .find(|v| (v.number..=v.last_number.max(v.number)).contains(&number))
        {
            // A bridge holds several verses: its text once.
            if last_segment.as_deref() != Some(v.segment_id.as_str()) {
                parts.push(v.text.trim().to_owned());
                last_segment = Some(v.segment_id.clone());
            }
        }
    }
    if parts.is_empty() {
        return (None, false, None);
    }
    (Some(parts.join(" ")), capped || printed.len() > n, None)
}

fn osis_range(start: &Reference, end: &Reference) -> String {
    ReferenceRange {
        start: start.clone(),
        end: end.clone(),
    }
    .to_osis()
}

fn human_range(start: &Reference, end: &Reference) -> String {
    ReferenceRange {
        start: start.clone(),
        end: end.clone(),
    }
    .to_human(english())
}

/// One visible target as it is being merged.
struct Pending {
    source_start: Reference,
    source_end: Reference,
    assertions: Vec<CrossReferenceAssertion>,
}

impl ReadingLibrary {
    /// Installs a cross-reference source, replacing any installed before.
    pub fn set_cross_references(&mut self, source: Box<dyn CrossReferenceSource>) {
        self.cross_references = Some(source);
    }

    /// Removes the installed cross-reference source.
    pub fn clear_cross_references(&mut self) {
        self.cross_references = None;
    }

    /// Where a target published in `source_scheme` lands in an edition: its OSIS range in the
    /// edition's numbering (`None` when it does not map), the mapping status and what the
    /// edition prints of it. What [`ReadingLibrary::cross_references`] labels each item with;
    /// the importer counts it over every published row.
    pub fn place_cross_reference(
        &self,
        resource_id: &str,
        source_scheme: &str,
        start: &Reference,
        end: &Reference,
    ) -> Result<(Option<String>, MappingStatus, Availability), ReadingError> {
        let placed = place(self.installed(resource_id)?, source_scheme, start, end);
        Ok((
            placed.range.as_ref().map(ReferenceRange::to_osis),
            placed.mapping,
            placed.availability,
        ))
    }

    /// What the installed cross-reference source is, if one is installed.
    #[must_use]
    pub fn cross_reference_info(&self) -> Option<&CrossReferenceInfo> {
        self.cross_references.as_ref().map(|s| s.info())
    }

    /// The cross references of a passage (`John 3:16`, `Ps 23`, `Rom 8:28–30`) in one edition,
    /// best first by the published votes. See the module documentation for every rule.
    pub fn cross_references(
        &self,
        resource_id: &str,
        input: &str,
        options: &CrossReferenceOptions,
    ) -> Result<CrossReferenceList, CrossReferenceError> {
        let source = self
            .cross_references
            .as_deref()
            .ok_or(CrossReferenceError::NotInstalled)?;
        let info = source.info();
        let installed = self.installed(resource_id)?;
        let edition_scheme = installed.pack.edition().scheme.clone();
        let location = self.locate(resource_id, input)?;
        let start = canon::segment_reference(&location.verse_start).ok_or_else(|| {
            CrossReferenceError::Reading {
                error: ReadingError::InvalidReference {
                    input: input.to_owned(),
                    detail: "no verse".to_owned(),
                },
            }
        })?;
        let end = canon::segment_reference(&location.verse_end).unwrap_or_else(|| start.clone());
        let (passage, capped) = walk(installed, &start, &end, MAX_QUERY_VERSES + 1);
        if capped || passage.len() > MAX_QUERY_VERSES {
            return Err(CrossReferenceError::TooLong {
                verses: passage.len(),
                max: MAX_QUERY_VERSES,
            });
        }

        // The passage in the source's scheme.
        let mut mapping = MappingStatus::Exact;
        let mut source_verses: Vec<Reference> = Vec::new();
        // Every verse of the passage, printed or not: a verse the edition omits (BSB Matt 17:21)
        // still has the links the source lists under it.
        for (verse, _) in &passage {
            let (status, targets) = map_verse(verse, &edition_scheme, &info.scheme);
            mapping = worst(mapping, status);
            for t in targets {
                if !source_verses.contains(&t) {
                    source_verses.push(t);
                }
            }
        }
        if source_verses.is_empty() {
            mapping = MappingStatus::Unmapped;
        }

        // Every assertion, keyed by the target as published.
        let mut pending: BTreeMap<(Reference, Reference), Pending> = BTreeMap::new();
        let mut add = |start: Reference, end: Reference, assertion: CrossReferenceAssertion| {
            pending
                .entry((start.clone(), end.clone()))
                .or_insert_with(|| Pending {
                    source_start: start,
                    source_end: end,
                    assertions: Vec::new(),
                })
                .assertions
                .push(assertion);
        };
        let fail = |detail: String| CrossReferenceError::Source { detail };
        for verse in &source_verses {
            for row in source.outgoing(verse).map_err(fail)? {
                add(
                    row.to_start.clone(),
                    row.to_end.clone(),
                    CrossReferenceAssertion {
                        from: row.from.to_osis(),
                        to: osis_range(&row.to_start, &row.to_end),
                        votes: row.votes,
                        line: row.line,
                        direction: Direction::AsPublished,
                    },
                );
            }
            if options.include_reverse {
                for row in source.incoming(verse).map_err(fail)? {
                    // A link from inside the passage to inside it is already listed as published.
                    if source_verses.contains(&row.from) {
                        continue;
                    }
                    add(
                        row.from.clone(),
                        row.from.clone(),
                        CrossReferenceAssertion {
                            from: row.from.to_osis(),
                            to: osis_range(&row.to_start, &row.to_end),
                            votes: row.votes,
                            line: row.line,
                            direction: Direction::ReverseDerived,
                        },
                    );
                }
            }
        }

        // Placed in the edition and merged by visible target.
        let mut merged: Vec<(Placed, Vec<Pending>)> = Vec::new();
        let mut by_visible: HashMap<String, usize> = HashMap::new();
        for (_, p) in pending {
            let placed = place(installed, &info.scheme, &p.source_start, &p.source_end);
            let key = match &placed.range {
                Some(r) => format!("e:{}", r.to_osis()),
                None => format!("s:{}", osis_range(&p.source_start, &p.source_end)),
            };
            match by_visible.get(&key) {
                Some(i) => merged[*i].1.push(p),
                None => {
                    by_visible.insert(key, merged.len());
                    merged.push((placed, vec![p]));
                }
            }
        }

        let others: Vec<&Installed> = self
            .packs
            .iter()
            .filter(|p| p.pack.edition().resource_id != resource_id)
            .collect();
        let mut items: Vec<(Reference, Option<ReferenceRange>, CrossReference)> =
            Vec::with_capacity(merged.len());
        for (placed, group) in merged {
            let first = &group[0];
            let mut assertions: Vec<CrossReferenceAssertion> =
                group.iter().flat_map(|p| p.assertions.clone()).collect();
            assertions.sort_by(|a, b| {
                a.direction
                    .cmp(&b.direction)
                    .then(b.votes.cmp(&a.votes))
                    .then(a.line.cmp(&b.line))
            });
            let direction = assertions[0].direction;
            let votes = assertions[0].votes;
            let source_target = osis_range(&first.source_start, &first.source_end);
            let source_label = human_range(&first.source_start, &first.source_end);
            let available_in = others
                .iter()
                .filter(|o| {
                    let p = place(o, &info.scheme, &first.source_start, &first.source_start);
                    matches!(p.availability, Availability::Printed)
                })
                .map(|o| o.pack.edition().resource_id.clone())
                .collect();
            let order = placed
                .range
                .as_ref()
                .map_or_else(|| first.source_start.clone(), |r| r.start.clone());
            let printed = matches!(
                placed.availability,
                Availability::Printed | Availability::PartlyPrinted
            );
            items.push((
                order,
                placed.range.clone().filter(|_| printed),
                CrossReference {
                    target: placed.range.as_ref().map(ReferenceRange::to_osis),
                    label: placed
                        .range
                        .as_ref()
                        .map_or_else(|| source_label.clone(), |r| r.to_human(english())),
                    source_target,
                    source_label,
                    mapping: placed.mapping,
                    availability: placed.availability,
                    preview: None,
                    preview_truncated: false,
                    preview_chunk: None,
                    relation: info.relation.clone(),
                    kind: info.kind.clone(),
                    direction,
                    votes,
                    assertions,
                    available_in,
                    attribution: info.attribution.clone(),
                },
            ));
        }
        items.sort_by(|(oa, _, a), (ob, _, b)| {
            a.direction
                .cmp(&b.direction)
                .then(b.votes.cmp(&a.votes))
                .then(oa.cmp(ob))
        });
        let total = items.len();
        let published = items
            .iter()
            .filter(|(_, _, i)| i.direction == Direction::AsPublished)
            .count();
        // Preview text only for the page returned: reading a chapter is what costs.
        let mut cache = ChapterCache::new();
        let items = items
            .into_iter()
            .skip(options.offset)
            .take(options.limit.unwrap_or(usize::MAX))
            .map(|(_, range, mut item)| {
                if let Some(range) = range {
                    (item.preview, item.preview_truncated, item.preview_chunk) =
                        preview(installed, &mut cache, &range, options.preview_verses);
                }
                item
            })
            .collect();
        Ok(CrossReferenceList {
            resource_id: resource_id.to_owned(),
            reference: location.reference,
            source_verses: source_verses.iter().map(Reference::to_osis).collect(),
            mapping,
            items,
            total,
            published,
            source: info.clone(),
        })
    }
}

#[cfg(test)]
mod tests;
