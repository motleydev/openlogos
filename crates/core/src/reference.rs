//! Scripture references: book identifiers, verse references, ranges and sets, with parsing
//! from the usual human syntaxes and formatting back to a canonical OSIS-like string.
//!
//! A reference here is an *address in a versification scheme*, not an address in an edition
//! (specs/03-data-model.md keeps the internal address edition-scoped); mapping between
//! schemes is [`crate::versification`]. Parsing never consults verse counts, so `Gen 1:99`
//! parses; validating against a scheme is the caller's job once it knows the edition.
//!
//! Chapter-only forms (`Ps 23`) and abbreviations that fit more than one book (`Ph`) are not
//! errors: they parse to the most likely reading and the result carries a [`ReviewFlag`] so a
//! UI can show them as review items (specs/04-ingestion-and-indexing.md).

use std::fmt;

use serde::{Deserialize, Serialize};

/// A book of the Protestant 66-book canon, in canonical order.
///
/// Variants are named after their OSIS identifiers (`1Sam` becomes `Sam1` because an
/// identifier cannot start with a digit). Deuterocanonical books will be appended in a later
/// MINOR version of [`crate::API_VERSION`]; code outside this crate should match with a
/// wildcard arm or use [`BookId::all`], [`BookId::number`] and [`BookId::from_osis`] rather
/// than relying on the variant count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
#[allow(missing_docs)]
pub enum BookId {
    Gen,
    Exod,
    Lev,
    Num,
    Deut,
    Josh,
    Judg,
    Ruth,
    Sam1,
    Sam2,
    Kgs1,
    Kgs2,
    Chr1,
    Chr2,
    Ezra,
    Neh,
    Esth,
    Job,
    Ps,
    Prov,
    Eccl,
    Song,
    Isa,
    Jer,
    Lam,
    Ezek,
    Dan,
    Hos,
    Joel,
    Amos,
    Obad,
    Jonah,
    Mic,
    Nah,
    Hab,
    Zeph,
    Hag,
    Zech,
    Mal,
    Matt,
    Mark,
    Luke,
    John,
    Acts,
    Rom,
    Cor1,
    Cor2,
    Gal,
    Eph,
    Phil,
    Col,
    Thess1,
    Thess2,
    Tim1,
    Tim2,
    Titus,
    Phlm,
    Heb,
    Jas,
    Pet1,
    Pet2,
    John1,
    John2,
    John3,
    Jude,
    Rev,
}

/// `(book, OSIS id, USFM/Paratext book number)` in canonical order.
///
/// The USFM number skips 40 between Malachi (39) and Matthew (41), and the deuterocanon
/// starts at 68, so the number is stable when books are added later.
const BOOKS: [(BookId, &str, u8); 66] = [
    (BookId::Gen, "Gen", 1),
    (BookId::Exod, "Exod", 2),
    (BookId::Lev, "Lev", 3),
    (BookId::Num, "Num", 4),
    (BookId::Deut, "Deut", 5),
    (BookId::Josh, "Josh", 6),
    (BookId::Judg, "Judg", 7),
    (BookId::Ruth, "Ruth", 8),
    (BookId::Sam1, "1Sam", 9),
    (BookId::Sam2, "2Sam", 10),
    (BookId::Kgs1, "1Kgs", 11),
    (BookId::Kgs2, "2Kgs", 12),
    (BookId::Chr1, "1Chr", 13),
    (BookId::Chr2, "2Chr", 14),
    (BookId::Ezra, "Ezra", 15),
    (BookId::Neh, "Neh", 16),
    (BookId::Esth, "Esth", 17),
    (BookId::Job, "Job", 18),
    (BookId::Ps, "Ps", 19),
    (BookId::Prov, "Prov", 20),
    (BookId::Eccl, "Eccl", 21),
    (BookId::Song, "Song", 22),
    (BookId::Isa, "Isa", 23),
    (BookId::Jer, "Jer", 24),
    (BookId::Lam, "Lam", 25),
    (BookId::Ezek, "Ezek", 26),
    (BookId::Dan, "Dan", 27),
    (BookId::Hos, "Hos", 28),
    (BookId::Joel, "Joel", 29),
    (BookId::Amos, "Amos", 30),
    (BookId::Obad, "Obad", 31),
    (BookId::Jonah, "Jonah", 32),
    (BookId::Mic, "Mic", 33),
    (BookId::Nah, "Nah", 34),
    (BookId::Hab, "Hab", 35),
    (BookId::Zeph, "Zeph", 36),
    (BookId::Hag, "Hag", 37),
    (BookId::Zech, "Zech", 38),
    (BookId::Mal, "Mal", 39),
    (BookId::Matt, "Matt", 41),
    (BookId::Mark, "Mark", 42),
    (BookId::Luke, "Luke", 43),
    (BookId::John, "John", 44),
    (BookId::Acts, "Acts", 45),
    (BookId::Rom, "Rom", 46),
    (BookId::Cor1, "1Cor", 47),
    (BookId::Cor2, "2Cor", 48),
    (BookId::Gal, "Gal", 49),
    (BookId::Eph, "Eph", 50),
    (BookId::Phil, "Phil", 51),
    (BookId::Col, "Col", 52),
    (BookId::Thess1, "1Thess", 53),
    (BookId::Thess2, "2Thess", 54),
    (BookId::Tim1, "1Tim", 55),
    (BookId::Tim2, "2Tim", 56),
    (BookId::Titus, "Titus", 57),
    (BookId::Phlm, "Phlm", 58),
    (BookId::Heb, "Heb", 59),
    (BookId::Jas, "Jas", 60),
    (BookId::Pet1, "1Pet", 61),
    (BookId::Pet2, "2Pet", 62),
    (BookId::John1, "1John", 63),
    (BookId::John2, "2John", 64),
    (BookId::John3, "3John", 65),
    (BookId::Jude, "Jude", 66),
    (BookId::Rev, "Rev", 67),
];

impl BookId {
    /// Every book in canonical order.
    #[must_use]
    pub fn all() -> Vec<BookId> {
        BOOKS.iter().map(|(book, _, _)| *book).collect()
    }

    /// The OSIS book identifier, e.g. `1Sam`.
    #[must_use]
    pub fn as_osis(self) -> &'static str {
        BOOKS[self.canonical_index() as usize].1
    }

    /// The book with this OSIS identifier (case-sensitive, as OSIS defines them).
    #[must_use]
    pub fn from_osis(id: &str) -> Option<BookId> {
        BOOKS
            .iter()
            .find(|(_, osis, _)| *osis == id)
            .map(|(book, _, _)| *book)
    }

    /// Zero-based position in canonical order (Genesis is 0, Revelation is 65).
    #[must_use]
    pub fn canonical_index(self) -> u32 {
        self as u32
    }

    /// The USFM/Paratext book number (Genesis 1, Malachi 39, Matthew 41, Revelation 67).
    ///
    /// This is the `BB` field of a token ID (specs/19-search-and-retrieval.md); it does not
    /// change when deuterocanonical books are added, unlike [`BookId::canonical_index`].
    #[must_use]
    pub fn number(self) -> u8 {
        BOOKS[self.canonical_index() as usize].2
    }

    /// The book with this USFM/Paratext number, if it is in the canon this crate knows.
    #[must_use]
    pub fn from_number(number: u8) -> Option<BookId> {
        BOOKS
            .iter()
            .find(|(_, _, n)| *n == number)
            .map(|(book, _, _)| *book)
    }

    /// Books with a single chapter, where `Jude 3` means verse 3 rather than chapter 3.
    #[must_use]
    pub fn is_single_chapter(self) -> bool {
        matches!(
            self,
            BookId::Obad | BookId::Phlm | BookId::John2 | BookId::John3 | BookId::Jude
        )
    }
}

impl From<BookId> for String {
    fn from(book: BookId) -> String {
        book.as_osis().to_owned()
    }
}

impl TryFrom<String> for BookId {
    type Error = ReferenceError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        BookId::from_osis(&value).ok_or(ReferenceError::UnknownBook { name: value })
    }
}

impl fmt::Display for BookId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_osis())
    }
}

/// One verse, or a whole chapter when `verse` is `None`.
///
/// Verse 0 is the Psalm superscription in schemes that number it (Paratext `eng`), so it is
/// allowed; chapter 0 is not. `subverse` is the OSIS grain after `!` (`Gen.1.1!a`), used for
/// partial verse mappings and split verses.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Reference {
    /// The book.
    pub book: BookId,
    /// Chapter number, 1-based.
    pub chapter: u32,
    /// Verse number, or `None` for a chapter-only reference such as `Ps 23`.
    pub verse: Option<u32>,
    /// Sub-verse grain such as `a` or `b`; only meaningful with a verse.
    pub subverse: Option<String>,
}

impl Reference {
    /// A verse reference.
    #[must_use]
    pub fn verse(book: BookId, chapter: u32, verse: u32) -> Reference {
        Reference {
            book,
            chapter,
            verse: Some(verse),
            subverse: None,
        }
    }

    /// A chapter-only reference.
    #[must_use]
    pub fn chapter(book: BookId, chapter: u32) -> Reference {
        Reference {
            book,
            chapter,
            verse: None,
            subverse: None,
        }
    }

    /// The same reference with a sub-verse grain.
    #[must_use]
    pub fn with_subverse(mut self, subverse: &str) -> Reference {
        self.subverse = Some(subverse.to_owned());
        self
    }

    /// Canonical form: `John.3.16`, `Ps.23`, `Gen.1.1!a`.
    #[must_use]
    pub fn to_osis(&self) -> String {
        let mut out = format!("{}.{}", self.book.as_osis(), self.chapter);
        if let Some(verse) = self.verse {
            out.push('.');
            out.push_str(&verse.to_string());
            if let Some(sub) = &self.subverse {
                out.push('!');
                out.push_str(sub);
            }
        }
        out
    }

    /// Human form with the table's display name: `John 3:16`, `Ps 23`, `Gen 1:1a`, and for
    /// single-chapter books `Jude 3` or just `Jude`.
    #[must_use]
    pub fn to_human(&self, table: &AliasTable) -> String {
        let name = table.display_name(self.book);
        let label = self.verse_label();
        if self.omits_chapter() {
            return match label {
                Some(label) => format!("{name} {label}"),
                None => name.to_owned(),
            };
        }
        let mut out = format!("{name} {}", self.chapter);
        if let Some(label) = label {
            out.push(':');
            out.push_str(&label);
        }
        out
    }

    /// `verse` plus the sub-verse grain (`16`, `1a`), or `None` for a chapter-only reference.
    fn verse_label(&self) -> Option<String> {
        self.verse.map(|verse| {
            let mut label = verse.to_string();
            if let Some(sub) = &self.subverse {
                label.push_str(sub);
            }
            label
        })
    }

    /// Whether people write this reference without a chapter (`Jude 3`).
    fn omits_chapter(&self) -> bool {
        self.book.is_single_chapter() && self.chapter == 1
    }

    /// Human form without the book name: `3:16`, `23`, or `3` in a single-chapter book.
    fn to_human_in_book(&self, table: &AliasTable) -> String {
        strip_book(&self.to_human(table), table.display_name(self.book)).to_owned()
    }
}

/// Removes a leading book name from a human-form reference.
fn strip_book<'a>(human: &'a str, name: &str) -> &'a str {
    human.strip_prefix(name).map_or(human, str::trim_start)
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_osis())
    }
}

/// An inclusive range of references, within one book or across books.
///
/// A single verse is a range whose `start` and `end` are equal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ReferenceRange {
    /// First reference of the range.
    pub start: Reference,
    /// Last reference of the range, inclusive.
    pub end: Reference,
}

impl ReferenceRange {
    /// A range covering exactly one reference.
    #[must_use]
    pub fn single(reference: Reference) -> ReferenceRange {
        ReferenceRange {
            start: reference.clone(),
            end: reference,
        }
    }

    /// A range from `start` to `end`, rejecting one that runs backwards.
    pub fn new(start: Reference, end: Reference) -> Result<ReferenceRange, ReferenceError> {
        if end < start {
            return Err(ReferenceError::ReversedRange {
                start: start.to_osis(),
                end: end.to_osis(),
            });
        }
        Ok(ReferenceRange { start, end })
    }

    /// Whether the range is one reference.
    #[must_use]
    pub fn is_single(&self) -> bool {
        self.start == self.end
    }

    /// Canonical form: `John.3.16`, `Gen.1.1-Gen.2.3` (both ends fully qualified).
    #[must_use]
    pub fn to_osis(&self) -> String {
        if self.is_single() {
            self.start.to_osis()
        } else {
            format!("{}-{}", self.start.to_osis(), self.end.to_osis())
        }
    }

    /// Human form: `John 3:16`, `Gen 1:1–2:3`, `Gen 1:1–5`, `Gen 1–3`, `Gen 50:26–Exod 1:1`.
    #[must_use]
    pub fn to_human(&self, table: &AliasTable) -> String {
        let mut out = self.start.to_human(table);
        if self.is_single() {
            return out;
        }
        out.push('–');
        if self.end.book != self.start.book {
            out.push_str(&self.end.to_human(table));
        } else if self.spans_one_chapter() {
            // `Gen 1:1–5`, `Jude 3–4`.
            out.push_str(&self.end.verse_label().unwrap_or_default());
        } else {
            out.push_str(&self.end.to_human_in_book(table));
        }
        out
    }

    /// Whether both ends are verses of the same chapter of the same book.
    fn spans_one_chapter(&self) -> bool {
        self.start.book == self.end.book
            && self.start.chapter == self.end.chapter
            && self.start.verse.is_some()
            && self.end.verse.is_some()
    }

    /// Human form without the book name: `3:16`, `1:1–2:3`, `23`.
    fn to_human_in_book(&self, table: &AliasTable) -> String {
        strip_book(&self.to_human(table), table.display_name(self.start.book)).to_owned()
    }
}

/// Why a parsed range needs a human look before it is treated as precise.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReviewFlag {
    /// The range names a chapter without verses (`Ps 23`); its extent depends on the scheme.
    ChapterOnly {
        /// Index into [`ReferenceSet::ranges`].
        range: u32,
    },
    /// The book abbreviation matches several books; the first candidate was used.
    AmbiguousBook {
        /// Index into [`ReferenceSet::ranges`].
        range: u32,
        /// The abbreviation as written.
        alias: String,
        /// Every book the abbreviation can mean, most likely first.
        candidates: Vec<BookId>,
    },
}

/// An ordered list of ranges, as one citation such as `Rom 8:28, 31; 12:1` produces.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReferenceSet {
    /// The ranges in the order written.
    pub ranges: Vec<ReferenceRange>,
    /// Review items the parser raised; empty for a precise, unambiguous citation.
    pub flags: Vec<ReviewFlag>,
}

impl ReferenceSet {
    /// Canonical form, ranges separated by `;`: `Rom.8.28;Rom.8.31;Rom.12.1`.
    #[must_use]
    pub fn to_osis(&self) -> String {
        self.ranges
            .iter()
            .map(ReferenceRange::to_osis)
            .collect::<Vec<_>>()
            .join(";")
    }

    /// Human form, eliding repeated books and chapters: `Rom 8:28, 31; 12:1`.
    #[must_use]
    pub fn to_human(&self, table: &AliasTable) -> String {
        let mut out = String::new();
        let mut previous: Option<&ReferenceRange> = None;
        for range in &self.ranges {
            match previous {
                None => out.push_str(&range.to_human(table)),
                Some(prev) if prev.end.book != range.start.book => {
                    out.push_str("; ");
                    out.push_str(&range.to_human(table));
                }
                Some(prev)
                    if prev.end.chapter == range.start.chapter
                        && prev.end.verse.is_some()
                        && (range.is_single() || range.spans_one_chapter())
                        && range.start.verse.is_some() =>
                {
                    // Same book and chapter: only verses, `31` or `31–33`.
                    out.push_str(", ");
                    out.push_str(&range.start.verse_label().unwrap_or_default());
                    if !range.is_single() {
                        out.push('–');
                        out.push_str(&range.end.verse_label().unwrap_or_default());
                    }
                }
                Some(_) => {
                    // Same book, new chapter: `12:1` after `Rom 8:28`.
                    out.push_str("; ");
                    out.push_str(&range.to_human_in_book(table));
                }
            }
            previous = Some(range);
        }
        out
    }

    /// Short form for tab labels and palette rows: [`ReferenceSet::to_human`] with the
    /// short English names of [`AliasTable::english_short`] (`Jn 3:16`, `Gen 1:1–2`).
    #[must_use]
    pub fn to_short(&self) -> String {
        static SHORT: std::sync::OnceLock<AliasTable> = std::sync::OnceLock::new();
        self.to_human(SHORT.get_or_init(AliasTable::english_short))
    }
}

/// Errors from parsing or building references.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReferenceError {
    /// The input had no reference in it.
    #[error("empty reference")]
    Empty,
    /// A book name or abbreviation that the alias table does not know.
    #[error("unknown book `{name}`")]
    UnknownBook {
        /// The name as written.
        name: String,
    },
    /// A number without a book before it and no book carried over from an earlier item.
    #[error("reference `{text}` has no book")]
    MissingBook {
        /// The item as written.
        text: String,
    },
    /// A book name with no chapter after it (`Gen` on its own, or `Gen 0:1`).
    #[error("`{text}` needs a chapter number of 1 or more")]
    MissingChapter {
        /// The item as written.
        text: String,
    },
    /// A number too large to be a chapter or verse.
    #[error("`{text}` is not a usable chapter or verse number")]
    InvalidNumber {
        /// The digits as written.
        text: String,
    },
    /// A character that no reference syntax uses at this position.
    #[error("unexpected `{character}` at scalar offset {offset}")]
    UnexpectedCharacter {
        /// Offset of the character in Unicode scalar values.
        offset: u32,
        /// The character itself.
        character: char,
    },
    /// A range whose end precedes its start.
    #[error("range runs backwards: {start} to {end}")]
    ReversedRange {
        /// Canonical form of the start.
        start: String,
        /// Canonical form of the end.
        end: String,
    },
}

/// Names and abbreviations for one book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookAliases {
    /// The book.
    pub book: BookId,
    /// The name used when formatting for people, e.g. `1 Corinthians` or `Ps`.
    pub display_name: String,
    /// Accepted spellings; compared after [`AliasTable::normalize`].
    pub aliases: Vec<String>,
}

/// Book names and abbreviations for one language, used by [`parse`] and the human formatters.
///
/// The OSIS identifier of every book is always accepted even if the table omits it. An
/// abbreviation listed under more than one book is ambiguous; the first book in table order
/// wins and the parse carries [`ReviewFlag::AmbiguousBook`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AliasTable {
    /// One entry per book, in the order ambiguity is resolved.
    pub books: Vec<BookAliases>,
}

/// `(book, display name, aliases)`: the English table. Full names and SBL/common
/// abbreviations; deliberately ambiguous ones (`Ph`, `Jud`, `Ez`) appear under each book.
const ENGLISH: [(BookId, &str, &[&str]); 66] = [
    (BookId::Gen, "Gen", &["Genesis", "Ge", "Gn"]),
    (BookId::Exod, "Exod", &["Exodus", "Exo", "Ex"]),
    (BookId::Lev, "Lev", &["Leviticus", "Le", "Lv"]),
    (BookId::Num, "Num", &["Numbers", "Nu", "Nm", "Nb"]),
    (BookId::Deut, "Deut", &["Deuteronomy", "Deu", "Dt", "De"]),
    (BookId::Josh, "Josh", &["Joshua", "Jos", "Jsh"]),
    (BookId::Judg, "Judg", &["Judges", "Jdg", "Jg", "Jgs", "Jud"]),
    (BookId::Ruth, "Ruth", &["Ru", "Rth"]),
    (BookId::Sam1, "1 Sam", &["1 Samuel", "1Sa", "1Sm", "1S"]),
    (BookId::Sam2, "2 Sam", &["2 Samuel", "2Sa", "2Sm", "2S"]),
    (
        BookId::Kgs1,
        "1 Kgs",
        &["1 Kings", "1Kg", "1Ki", "1Kin", "1K"],
    ),
    (
        BookId::Kgs2,
        "2 Kgs",
        &["2 Kings", "2Kg", "2Ki", "2Kin", "2K"],
    ),
    (BookId::Chr1, "1 Chr", &["1 Chronicles", "1Ch", "1Chron"]),
    (BookId::Chr2, "2 Chr", &["2 Chronicles", "2Ch", "2Chron"]),
    (BookId::Ezra, "Ezra", &["Ezr", "Ez"]),
    (BookId::Neh, "Neh", &["Nehemiah", "Ne"]),
    (BookId::Esth, "Esth", &["Esther", "Est", "Es"]),
    (BookId::Job, "Job", &["Jb"]),
    (
        BookId::Ps,
        "Ps",
        &["Psalms", "Psalm", "Pss", "Psa", "Psm", "Pslm"],
    ),
    (BookId::Prov, "Prov", &["Proverbs", "Pro", "Pr", "Prv"]),
    (
        BookId::Eccl,
        "Eccl",
        &["Ecclesiastes", "Ecc", "Ec", "Qoheleth", "Qoh"],
    ),
    (
        BookId::Song,
        "Song",
        &[
            "Song of Songs",
            "Song of Solomon",
            "So",
            "SoS",
            "Canticles",
            "Cant",
            "Sg",
        ],
    ),
    (BookId::Isa, "Isa", &["Isaiah", "Is"]),
    (BookId::Jer, "Jer", &["Jeremiah", "Je", "Jr"]),
    (BookId::Lam, "Lam", &["Lamentations", "La"]),
    (BookId::Ezek, "Ezek", &["Ezekiel", "Eze", "Ezk", "Ez"]),
    (BookId::Dan, "Dan", &["Daniel", "Da", "Dn"]),
    (BookId::Hos, "Hos", &["Hosea", "Ho"]),
    (BookId::Joel, "Joel", &["Joe", "Jl"]),
    (BookId::Amos, "Amos", &["Am"]),
    (BookId::Obad, "Obad", &["Obadiah", "Ob"]),
    (BookId::Jonah, "Jonah", &["Jon", "Jnh"]),
    (BookId::Mic, "Mic", &["Micah", "Mc"]),
    (BookId::Nah, "Nah", &["Nahum", "Na"]),
    (BookId::Hab, "Hab", &["Habakkuk", "Hb"]),
    (BookId::Zeph, "Zeph", &["Zephaniah", "Zep", "Zp"]),
    (BookId::Hag, "Hag", &["Haggai", "Hg"]),
    (BookId::Zech, "Zech", &["Zechariah", "Zec", "Zc"]),
    (BookId::Mal, "Mal", &["Malachi", "Ml"]),
    (BookId::Matt, "Matt", &["Matthew", "Mat", "Mt"]),
    (BookId::Mark, "Mark", &["Mrk", "Mk", "Mr"]),
    (BookId::Luke, "Luke", &["Luk", "Lk"]),
    (BookId::John, "John", &["Jn", "Jhn", "Joh"]),
    (BookId::Acts, "Acts", &["Act", "Ac"]),
    (BookId::Rom, "Rom", &["Romans", "Ro", "Rm"]),
    (BookId::Cor1, "1 Cor", &["1 Corinthians", "1Co", "1C"]),
    (BookId::Cor2, "2 Cor", &["2 Corinthians", "2Co", "2C"]),
    (BookId::Gal, "Gal", &["Galatians", "Ga"]),
    (BookId::Eph, "Eph", &["Ephesians", "Ep"]),
    (BookId::Phil, "Phil", &["Philippians", "Php", "Pp", "Ph"]),
    (BookId::Col, "Col", &["Colossians", "Co"]),
    (
        BookId::Thess1,
        "1 Thess",
        &["1 Thessalonians", "1Thes", "1Th"],
    ),
    (
        BookId::Thess2,
        "2 Thess",
        &["2 Thessalonians", "2Thes", "2Th"],
    ),
    (BookId::Tim1, "1 Tim", &["1 Timothy", "1Ti"]),
    (BookId::Tim2, "2 Tim", &["2 Timothy", "2Ti"]),
    (BookId::Titus, "Titus", &["Tit", "Ti"]),
    (
        BookId::Phlm,
        "Phlm",
        &["Philemon", "Phm", "Philem", "Pm", "Ph"],
    ),
    (BookId::Heb, "Heb", &["Hebrews"]),
    (BookId::Jas, "Jas", &["James", "Jm"]),
    (BookId::Pet1, "1 Pet", &["1 Peter", "1Pe", "1Pt", "1P"]),
    (BookId::Pet2, "2 Pet", &["2 Peter", "2Pe", "2Pt", "2P"]),
    (BookId::John1, "1 John", &["1Jn", "1Jhn", "1Jo", "1J"]),
    (BookId::John2, "2 John", &["2Jn", "2Jhn", "2Jo", "2J"]),
    (BookId::John3, "3 John", &["3Jn", "3Jhn", "3Jo", "3J"]),
    (BookId::Jude, "Jude", &["Jd", "Jud"]),
    (
        BookId::Rev,
        "Rev",
        &["Revelation", "Re", "Apocalypse", "Apoc"],
    ),
];

/// Where the short English names differ from the display names: the Gospels and John's
/// letters in the two-letter forms readers use in tight space. Every short name is also an
/// alias, so a short label parses back to its book.
const ENGLISH_SHORT: [(BookId, &str); 7] = [
    (BookId::Matt, "Mt"),
    (BookId::Mark, "Mk"),
    (BookId::Luke, "Lk"),
    (BookId::John, "Jn"),
    (BookId::John1, "1 Jn"),
    (BookId::John2, "2 Jn"),
    (BookId::John3, "3 Jn"),
];

impl AliasTable {
    /// The English table with short display names: the SBL abbreviations of
    /// [`AliasTable::english`], except the Gospels and John's letters (`Mt`, `Mk`, `Lk`,
    /// `Jn`, `1 Jn`). Parsing is the same as with [`AliasTable::english`].
    #[must_use]
    pub fn english_short() -> AliasTable {
        let mut table = AliasTable::english();
        for entry in &mut table.books {
            if let Some((_, short)) = ENGLISH_SHORT.iter().find(|(book, _)| *book == entry.book) {
                entry.display_name = (*short).to_owned();
            }
        }
        table
    }

    /// English names and abbreviations.
    #[must_use]
    pub fn english() -> AliasTable {
        AliasTable {
            books: ENGLISH
                .iter()
                .map(|(book, display, aliases)| BookAliases {
                    book: *book,
                    display_name: (*display).to_owned(),
                    aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
                })
                .collect(),
        }
    }

    /// The display name for a book, falling back to its OSIS identifier.
    #[must_use]
    pub fn display_name(&self, book: BookId) -> &str {
        self.books
            .iter()
            .find(|entry| entry.book == book)
            .map_or_else(|| book.as_osis(), |entry| entry.display_name.as_str())
    }

    /// The key form of a name: lower case, ordinals `I`/`II`/`III` and `First`/`Second`/
    /// `Third` folded to digits, spaces and periods removed.
    #[must_use]
    pub fn normalize(name: &str) -> String {
        let lower = name.trim().to_lowercase();
        let folded = [
            ("iii ", "3"),
            ("ii ", "2"),
            ("i ", "1"),
            ("iii.", "3"),
            ("ii.", "2"),
            ("i.", "1"),
            ("first ", "1"),
            ("second ", "2"),
            ("third ", "3"),
        ]
        .iter()
        .find_map(|(prefix, digit)| {
            lower
                .strip_prefix(prefix)
                .map(|rest| format!("{digit}{rest}"))
        })
        .unwrap_or(lower);
        folded
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '.')
            .collect()
    }

    /// Every book a written name can mean, most likely first; empty if unknown.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Vec<BookId> {
        let key = Self::normalize(name);
        if key.is_empty() {
            return Vec::new();
        }
        let mut found: Vec<BookId> = self
            .books
            .iter()
            .filter(|entry| {
                Self::normalize(&entry.display_name) == key
                    || entry.aliases.iter().any(|a| Self::normalize(a) == key)
            })
            .map(|entry| entry.book)
            .collect();
        if found.is_empty()
            && let Some(book) = BookId::all()
                .into_iter()
                .find(|b| b.as_osis().to_lowercase() == key)
        {
            found.push(book);
        }
        found
    }
}

/// What separated the current item from the previous one, which decides what a bare number
/// means: after `;` it is a chapter, after `,` a verse when the previous item had one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Separator {
    Start,
    Semicolon,
    Comma,
}

/// The numbers of one item: `first` is a chapter or, without `second`, possibly a verse.
struct Numbers {
    first: u32,
    second: Option<u32>,
    subverse: Option<String>,
}

/// Cursor over the input's scalar values.
struct Scanner {
    chars: Vec<char>,
    pos: usize,
}

impl Scanner {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.pos + ahead).copied()
    }

    fn at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn eat_dash(&mut self) -> bool {
        self.eat('-') || self.eat('–') || self.eat('—')
    }

    /// A book name: an optional ordinal digit, then letters, spaces and periods up to the
    /// next digit or punctuation. Empty when the input continues with a number. The flag is
    /// set for the dotted OSIS form (`Jude.1`), where the first number is always a chapter.
    ///
    /// A digit is only taken as an ordinal (`1 Cor`, `2Co`) when the table knows the result;
    /// otherwise it is left for [`Scanner::numbers`], so `2b` stays verse 2, grain `b`.
    fn book_name(&mut self, table: &AliasTable) -> (String, bool) {
        let start = self.pos;
        let mut name = self.book_name_text(true);
        if name.starts_with(|c: char| c.is_ascii_digit()) && table.lookup(&name).is_empty() {
            self.pos = start;
            name = self.book_name_text(false);
        }
        let osis_form = name.ends_with('.') && self.peek().is_some_and(|c| c.is_ascii_digit());
        (name, osis_form)
    }

    fn book_name_text(&mut self, allow_ordinal: bool) -> String {
        let start = self.pos;
        // `1 Cor`: a single digit 1–3 followed (after optional spaces) by a letter.
        if allow_ordinal && matches!(self.peek(), Some('1'..='3')) {
            let mut ahead = 1;
            while self.peek_at(ahead).is_some_and(|c| c == ' ') {
                ahead += 1;
            }
            if self.peek_at(ahead).is_some_and(char::is_alphabetic) {
                self.pos += ahead;
            }
        }
        while self
            .peek()
            .is_some_and(|c| c.is_alphabetic() || c == ' ' || c == '.')
        {
            self.pos += 1;
        }
        self.chars[start..self.pos].iter().collect()
    }

    fn number(&mut self) -> Result<Option<u32>, ReferenceError> {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        if start == self.pos {
            return Ok(None);
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        text.parse::<u32>()
            .ok()
            .filter(|n| *n <= 999)
            .map(Some)
            .ok_or(ReferenceError::InvalidNumber { text })
    }

    /// A sub-verse grain: `a`, `b` or `!a` directly after a verse number.
    fn subverse(&mut self) -> Option<String> {
        let start = self.pos;
        self.eat('!');
        let letters_start = self.pos;
        while self.pos - letters_start < 2 && self.peek().is_some_and(|c| c.is_ascii_lowercase()) {
            self.pos += 1;
        }
        // One or two letters, not the start of a longer word.
        if self.pos > letters_start && !self.peek().is_some_and(char::is_alphabetic) {
            Some(self.chars[letters_start..self.pos].iter().collect())
        } else {
            self.pos = start;
            None
        }
    }

    /// `chapter`, `chapter:verse`, `chapter.verse`, with an optional sub-verse grain.
    fn numbers(&mut self) -> Result<Option<Numbers>, ReferenceError> {
        let Some(first) = self.number()? else {
            return Ok(None);
        };
        let second = if matches!(self.peek(), Some(':') | Some('.'))
            && self.peek_at(1).is_some_and(|c| c.is_ascii_digit())
        {
            self.pos += 1;
            self.number()?
        } else {
            None
        };
        let subverse = self.subverse();
        Ok(Some(Numbers {
            first,
            second,
            subverse,
        }))
    }

    fn unexpected(&self) -> ReferenceError {
        ReferenceError::UnexpectedCharacter {
            offset: self.pos as u32,
            character: self.peek().unwrap_or('\0'),
        }
    }
}

/// Parses a citation in any of the usual syntaxes into a [`ReferenceSet`].
///
/// Accepted: `John 3:16`, `Jn 3.16`, `John.3.16`, `Gen 1:1-2:3`, `Ps 9:1–10:18`, `Gen 1-3`,
/// `Rom 8:28, 31; 12:1`, `Jn 3:16,18`, `Ps 23`, `Jude 3`, `1 Cor 13:4`, `1Co 13.4`,
/// `Gen 1:1a`, `Gen.1.1!a`, `Gen 50:26-Exod 1:1`. Hyphen, en dash and em dash all mark a
/// range. Chapter numbers must be at least 1; verses may be 0 (Psalm superscriptions).
pub fn parse(input: &str, table: &AliasTable) -> Result<ReferenceSet, ReferenceError> {
    let mut scanner = Scanner {
        chars: input.chars().collect(),
        pos: 0,
    };
    let mut set = ReferenceSet::default();
    let mut separator = Separator::Start;
    let mut context: Option<Reference> = None;

    loop {
        scanner.skip_whitespace();
        if scanner.at_end() {
            break;
        }
        let item_start = scanner.pos;
        let range_index = set.ranges.len() as u32;
        let missing_chapter = |scanner: &Scanner| ReferenceError::MissingChapter {
            text: rest_of_item(scanner, item_start),
        };

        // Book, written or carried over from the previous item.
        let (name, osis_form) = scanner.book_name(table);
        let named_book = !name.trim().is_empty();
        let book = if named_book {
            resolve_book(&name, table, range_index, &mut set.flags)?
        } else {
            match &context {
                Some(prev) => prev.book,
                None => {
                    return Err(ReferenceError::MissingBook {
                        text: rest_of_item(&scanner, item_start),
                    });
                }
            }
        };
        scanner.skip_whitespace();

        // Start of the range.
        let start = match scanner.numbers()? {
            Some(Numbers {
                first: chapter,
                second: Some(verse),
                subverse,
            }) => Reference {
                book,
                chapter,
                verse: Some(verse),
                subverse,
            },
            // `Jude 3`, or `3` carried over inside Jude: a verse of the only chapter.
            Some(Numbers {
                first: verse,
                second: None,
                subverse,
            }) if book.is_single_chapter() && !osis_form => Reference {
                book,
                chapter: 1,
                verse: Some(verse),
                subverse,
            },
            // `Rom 8:28, 31`: a bare number after a comma continues the chapter.
            Some(Numbers {
                first: verse,
                second: None,
                subverse,
            }) if !named_book
                && separator == Separator::Comma
                && context.as_ref().is_some_and(|prev| prev.verse.is_some()) =>
            {
                Reference {
                    book,
                    chapter: context.as_ref().map_or(1, |prev| prev.chapter),
                    verse: Some(verse),
                    subverse,
                }
            }
            Some(Numbers {
                first: chapter,
                second: None,
                ..
            }) => Reference::chapter(book, chapter),
            // `Jude` on its own is the whole single chapter.
            None if named_book && book.is_single_chapter() => Reference::chapter(book, 1),
            None => return Err(missing_chapter(&scanner)),
        };
        if start.chapter == 0 {
            return Err(missing_chapter(&scanner));
        }

        // Optional end of the range.
        scanner.skip_whitespace();
        let end = if scanner.eat_dash() {
            scanner.skip_whitespace();
            let (end_name, end_osis_form) = scanner.book_name(table);
            let end_named = !end_name.trim().is_empty();
            let end_book = if end_named {
                resolve_book(&end_name, table, range_index, &mut set.flags)?
            } else {
                start.book
            };
            scanner.skip_whitespace();
            let end = match scanner.numbers()? {
                Some(Numbers {
                    first: chapter,
                    second: Some(verse),
                    subverse,
                }) => Reference {
                    book: end_book,
                    chapter,
                    verse: Some(verse),
                    subverse,
                },
                Some(Numbers {
                    first: verse,
                    second: None,
                    subverse,
                }) if end_book.is_single_chapter() && !end_osis_form => Reference {
                    book: end_book,
                    chapter: 1,
                    verse: Some(verse),
                    subverse,
                },
                // `Gen 1:1-5`: a bare end number is a verse when the start has one.
                Some(Numbers {
                    first: verse,
                    second: None,
                    subverse,
                }) if !end_named && start.verse.is_some() => Reference {
                    book: end_book,
                    chapter: start.chapter,
                    verse: Some(verse),
                    subverse,
                },
                Some(Numbers {
                    first: chapter,
                    second: None,
                    ..
                }) => Reference::chapter(end_book, chapter),
                None => return Err(missing_chapter(&scanner)),
            };
            if end.chapter == 0 {
                return Err(missing_chapter(&scanner));
            }
            end
        } else {
            start.clone()
        };

        if start.verse.is_none() {
            set.flags
                .push(ReviewFlag::ChapterOnly { range: range_index });
        }
        context = Some(end.clone());
        set.ranges.push(ReferenceRange::new(start, end)?);

        // Separator or end of input.
        scanner.skip_whitespace();
        if scanner.at_end() {
            break;
        }
        separator = if scanner.eat(';') {
            Separator::Semicolon
        } else if scanner.eat(',') {
            Separator::Comma
        } else {
            return Err(scanner.unexpected());
        };
    }

    if set.ranges.is_empty() {
        return Err(ReferenceError::Empty);
    }
    Ok(set)
}

/// Looks a written book name up, recording an [`ReviewFlag::AmbiguousBook`] when needed.
fn resolve_book(
    name: &str,
    table: &AliasTable,
    range_index: u32,
    flags: &mut Vec<ReviewFlag>,
) -> Result<BookId, ReferenceError> {
    let written = name.trim().trim_end_matches('.').to_owned();
    let candidates = table.lookup(name);
    let Some(book) = candidates.first().copied() else {
        return Err(ReferenceError::UnknownBook { name: written });
    };
    if candidates.len() > 1 {
        flags.push(ReviewFlag::AmbiguousBook {
            range: range_index,
            alias: written,
            candidates,
        });
    }
    Ok(book)
}

/// The text of the item being parsed, for error messages.
fn rest_of_item(scanner: &Scanner, item_start: usize) -> String {
    scanner.chars[item_start..]
        .iter()
        .take_while(|c| **c != ';' && **c != ',')
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod short_tests {
    use super::*;

    #[test]
    fn short_forms_read_well_and_parse_back() {
        let english = AliasTable::english();
        for (input, short) in [
            ("John 3", "Jn 3"),
            ("John 3:16", "Jn 3:16"),
            ("Gen 1:1-2", "Gen 1:1–2"),
            ("1 John 4:8", "1 Jn 4:8"),
            ("Matt 5:3-7:29", "Mt 5:3–7:29"),
            ("Rom 8:28, 31; 12:1", "Rom 8:28, 31; 12:1"),
            ("Jude 3", "Jude 3"),
        ] {
            let set = parse(input, &english).unwrap();
            assert_eq!(set.to_short(), short, "{input}");
            assert_eq!(parse(short, &english).unwrap(), set, "{short} parses back");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn english() -> AliasTable {
        AliasTable::english()
    }

    fn r(book: BookId, chapter: u32, verse: u32) -> Reference {
        Reference::verse(book, chapter, verse)
    }

    fn range(start: Reference, end: Reference) -> ReferenceRange {
        ReferenceRange::new(start, end).unwrap()
    }

    #[test]
    fn osis_ids_round_trip_for_every_book() {
        let all = BookId::all();
        assert_eq!(all.len(), 66);
        for (index, book) in all.iter().enumerate() {
            assert_eq!(book.canonical_index() as usize, index);
            assert_eq!(BookId::from_osis(book.as_osis()), Some(*book));
            assert_eq!(BookId::from_number(book.number()), Some(*book));
        }
        assert_eq!(BookId::Gen.number(), 1);
        assert_eq!(BookId::Mal.number(), 39);
        assert_eq!(BookId::Matt.number(), 41);
        assert_eq!(BookId::Rev.number(), 67);
        assert_eq!(BookId::from_number(40), None);
        assert_eq!(BookId::from_osis("gen"), None);
    }

    #[test]
    fn books_serialize_as_osis_strings() {
        assert_eq!(serde_json::to_string(&BookId::Sam1).unwrap(), "\"1Sam\"");
        assert_eq!(
            serde_json::from_str::<BookId>("\"1Sam\"").unwrap(),
            BookId::Sam1
        );
        assert!(serde_json::from_str::<BookId>("\"Nope\"").is_err());
    }

    #[test]
    fn parses_the_usual_syntaxes() {
        let t = english();
        let cases: &[(&str, &str)] = &[
            ("John 3:16", "John.3.16"),
            ("Jn 3.16", "John.3.16"),
            ("John.3.16", "John.3.16"),
            ("Gen 1:1-2:3", "Gen.1.1-Gen.2.3"),
            ("Ps 9:1–10:18", "Ps.9.1-Ps.10.18"),
            ("Ps 9:1—10:18", "Ps.9.1-Ps.10.18"),
            ("Rom 8:28, 31; 12:1", "Rom.8.28;Rom.8.31;Rom.12.1"),
            ("Ps 23", "Ps.23"),
            ("Jn 3:16,18", "John.3.16;John.3.18"),
            ("Gen 1:1-5", "Gen.1.1-Gen.1.5"),
            ("Gen 1-3", "Gen.1-Gen.3"),
            ("Gen 1; 3", "Gen.1;Gen.3"),
            ("Gen 50:26-Exod 1:1", "Gen.50.26-Exod.1.1"),
            ("1 Cor 13:4", "1Cor.13.4"),
            ("1Co 13.4", "1Cor.13.4"),
            ("I Cor 13:4", "1Cor.13.4"),
            ("First Corinthians 13:4", "1Cor.13.4"),
            ("Song of Songs 2:1", "Song.2.1"),
            ("Jude 3", "Jude.1.3"),
            ("Jude 3-4", "Jude.1.3-Jude.1.4"),
            ("Phlm 6", "Phlm.1.6"),
            ("Phil 4:13", "Phil.4.13"),
            ("Gen 1:1a", "Gen.1.1!a"),
            ("Gen.1.1!a", "Gen.1.1!a"),
            ("Gen 1:1a-2b", "Gen.1.1!a-Gen.1.2!b"),
            ("Ps 3:0", "Ps.3.0"),
            ("Gen. 1:1", "Gen.1.1"),
            ("Jn 3:16; Rom 8:1", "John.3.16;Rom.8.1"),
            ("Jn 3:16, Rom 8:1", "John.3.16;Rom.8.1"),
            ("Ps 23; 3 John 1:4", "Ps.23;3John.1.4"),
            ("Jude", "Jude.1"),
            ("Jude 3, 5", "Jude.1.3;Jude.1.5"),
            ("Jude 3; 5", "Jude.1.3;Jude.1.5"),
            ("Jude.1", "Jude.1"),
            ("Jude.1.3", "Jude.1.3"),
            ("Obad 1:1", "Obad.1.1"),
        ];
        for (input, expected) in cases {
            let set = parse(input, &t).unwrap_or_else(|e| panic!("{input}: {e}"));
            assert_eq!(set.to_osis(), *expected, "input {input:?}");
        }
    }

    #[test]
    fn chapter_only_and_ambiguous_forms_are_flagged_not_rejected() {
        let t = english();
        let set = parse("Ps 23", &t).unwrap();
        assert_eq!(set.flags, vec![ReviewFlag::ChapterOnly { range: 0 }]);

        let set = parse("Ph 1:1", &t).unwrap();
        assert_eq!(set.ranges[0].start.book, BookId::Phil);
        assert_eq!(
            set.flags,
            vec![ReviewFlag::AmbiguousBook {
                range: 0,
                alias: "Ph".to_owned(),
                candidates: vec![BookId::Phil, BookId::Phlm],
            }]
        );

        let set = parse("Jud 1:1", &t).unwrap();
        assert_eq!(set.ranges[0].start.book, BookId::Judg);
        assert!(matches!(set.flags[0], ReviewFlag::AmbiguousBook { .. }));

        let set = parse("Jn 3:16; Ps 23", &t).unwrap();
        assert_eq!(set.flags, vec![ReviewFlag::ChapterOnly { range: 1 }]);

        assert!(parse("Phil 1:1", &t).unwrap().flags.is_empty());
        assert!(parse("Philemon 6", &t).unwrap().flags.is_empty());
    }

    #[test]
    fn rejects_what_it_cannot_read() {
        let t = english();
        assert_eq!(parse("", &t), Err(ReferenceError::Empty));
        assert_eq!(parse("   ", &t), Err(ReferenceError::Empty));
        assert!(matches!(
            parse("Hezekiah 3:16", &t),
            Err(ReferenceError::UnknownBook { name }) if name == "Hezekiah"
        ));
        assert!(matches!(
            parse("3:16", &t),
            Err(ReferenceError::MissingBook { .. })
        ));
        assert!(matches!(
            parse("Gen", &t),
            Err(ReferenceError::MissingChapter { .. })
        ));
        assert!(matches!(
            parse("Gen 0:1", &t),
            Err(ReferenceError::MissingChapter { .. })
        ));
        assert!(matches!(
            parse("Gen 2:1-1:1", &t),
            Err(ReferenceError::ReversedRange { .. })
        ));
        assert!(matches!(
            parse("Gen 1:1 Exod", &t),
            Err(ReferenceError::UnexpectedCharacter { .. })
        ));
        assert!(matches!(
            parse("Gen 1:1-", &t),
            Err(ReferenceError::MissingChapter { .. })
        ));
        assert!(matches!(
            parse("Gen 1:99999", &t),
            Err(ReferenceError::InvalidNumber { .. })
        ));
    }

    #[test]
    fn human_formatting_elides_repeated_books_and_chapters() {
        let t = english();
        let cases: &[(&str, &str)] = &[
            ("John 3:16", "John 3:16"),
            ("Rom 8:28, 31; 12:1", "Rom 8:28, 31; 12:1"),
            ("Gen 1:1-2:3", "Gen 1:1–2:3"),
            ("Gen 1:1-5", "Gen 1:1–5"),
            ("Gen 1-3", "Gen 1–3"),
            ("Ps 23", "Ps 23"),
            ("Gen 50:26-Exod 1:1", "Gen 50:26–Exod 1:1"),
            ("1 Cor 13:4-7", "1 Cor 13:4–7"),
            ("Jn 3:16,18; Rom 8:1", "John 3:16, 18; Rom 8:1"),
            ("Jn 3:16; Jn 3:18-19", "John 3:16, 18–19"),
            ("Gen 1:1a", "Gen 1:1a"),
            ("Ps 23; 24:1", "Ps 23; 24:1"),
        ];
        for (input, expected) in cases {
            let set = parse(input, &t).unwrap();
            assert_eq!(set.to_human(&t), *expected, "input {input:?}");
        }
    }

    #[test]
    fn format_then_parse_is_identity_over_a_grid() {
        let t = english();
        for book in BookId::all() {
            for chapter in [1u32, 2, 9, 10, 23, 119, 150] {
                let chapter_only = ReferenceRange::single(Reference::chapter(book, chapter));
                let set = ReferenceSet {
                    ranges: vec![chapter_only],
                    flags: vec![],
                };
                let parsed = parse(&set.to_osis(), &t).unwrap();
                assert_eq!(parsed.ranges, set.ranges, "{}", set.to_osis());
                // `Jude 9` reads as a verse; only `Jude` itself is a chapter in human form.
                if !book.is_single_chapter() || chapter == 1 {
                    let human = set.to_human(&t);
                    let parsed = parse(&human, &t).unwrap();
                    assert_eq!(parsed.ranges, set.ranges, "{human}");
                    assert_eq!(parsed.flags, vec![ReviewFlag::ChapterOnly { range: 0 }]);
                }
                for verse in [0u32, 1, 2, 16, 31, 176] {
                    let single = ReferenceRange::single(r(book, chapter, verse));
                    let span = range(r(book, chapter, verse), r(book, chapter + 1, verse + 3));
                    let sub = ReferenceRange::single(r(book, chapter, verse).with_subverse("b"));
                    let set = ReferenceSet {
                        ranges: vec![single, span, sub],
                        flags: vec![],
                    };
                    let osis = set.to_osis();
                    let parsed = parse(&osis, &t).unwrap();
                    assert_eq!(parsed.ranges, set.ranges, "{osis}");
                    assert!(parsed.flags.is_empty(), "{osis}");
                    assert_eq!(parsed.to_osis(), osis);
                    let human = set.to_human(&t);
                    let reparsed = parse(&human, &t).unwrap();
                    assert_eq!(reparsed.ranges, set.ranges, "{human}");
                    assert_eq!(reparsed.to_human(&t), human);
                }
            }
        }
    }

    #[test]
    fn parse_then_format_is_stable() {
        let t = english();
        for input in [
            "John 3:16",
            "Rom 8:28, 31; 12:1",
            "Gen 1:1-2:3",
            "Ps 23",
            "Jn 3:16,18",
            "Gen 50:26-Exod 1:1",
            "Ph 1:1",
        ] {
            let once = parse(input, &t).unwrap();
            let twice = parse(&once.to_osis(), &t).unwrap();
            assert_eq!(once.ranges, twice.ranges);
            assert_eq!(once.to_osis(), twice.to_osis());
            let human = parse(&once.to_human(&t), &t).unwrap();
            assert_eq!(human.ranges, once.ranges);
        }
    }

    #[test]
    fn cross_book_ranges_order_by_canon() {
        assert!(r(BookId::Gen, 50, 26) < r(BookId::Exod, 1, 1));
        assert!(r(BookId::Mal, 4, 6) < r(BookId::Matt, 1, 1));
        assert!(Reference::chapter(BookId::Ps, 23) < r(BookId::Ps, 23, 1));
        assert!(ReferenceRange::new(r(BookId::Exod, 1, 1), r(BookId::Gen, 1, 1)).is_err());
    }

    #[test]
    fn every_english_alias_resolves_to_its_book() {
        let t = english();
        for entry in &t.books {
            assert!(
                t.lookup(&entry.display_name).contains(&entry.book),
                "{}",
                entry.display_name
            );
            for alias in &entry.aliases {
                assert!(t.lookup(alias).contains(&entry.book), "{alias}");
            }
            assert_eq!(t.lookup(entry.book.as_osis()), vec![entry.book]);
        }
        assert!(t.lookup("").is_empty());
        assert!(t.lookup("Hezekiah").is_empty());
    }

    #[test]
    fn errors_and_sets_serialize() {
        let t = english();
        let set = parse("Ph 1:1; Ps 23", &t).unwrap();
        let json = serde_json::to_string(&set).unwrap();
        let back: ReferenceSet = serde_json::from_str(&json).unwrap();
        assert_eq!(back, set);
        let err = parse("Hezekiah 1", &t).unwrap_err();
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("unknown_book"));
    }
}
