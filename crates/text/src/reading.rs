//! Continuous reading (B078, specs/23-continuous-reading.md): an edition is one ordered
//! stream of chapters, Genesis 1:1 to Revelation 22:21 for a whole Bible, and the chapter is
//! the unit a host loads. A [`ReadingLibrary`] holds one [`TextPack`] per installed edition
//! and answers the four questions a reader needs:
//!
//! - [`ReadingLibrary::locate`]: where a typed passage is (its chapter, the verse to scroll
//!   to and the verses to mark), for ranges, whole chapters and whole books;
//! - [`ReadingLibrary::chapter_neighbors`]: the chapters before and after, across books;
//! - [`ReadingLibrary::chapter_content`]: one [`ChapterBlock`] with its heading and verses;
//! - [`reference_for_segment`]: the verse a segment id starts at, for the verse at the top of
//!   the viewport (which segment that is stays the host's measurement).
//!
//! A pack only has to say what books and chapters it has ([`TextPack::books`]) and give a
//! chapter's verses ([`TextPack::verses`]); everything else is computed here, so the
//! generated [`PlaceholderPack`] and a [`FilePack`] read from disk or fetched by a browser
//! behave the same. Structure comes from the pack, never from the shell: the canon table of
//! [`openlogos_core::canon`] is how the placeholder pack knows the shape of a Bible.

use std::collections::HashMap;
use std::sync::OnceLock;

use openlogos_core::canon;
use openlogos_core::reference::{AliasTable, BookId, Reference, parse};
use openlogos_core::versification::SchemeId;
use serde::{Deserialize, Serialize};

pub mod pack;
pub mod placeholder;

pub use pack::{
    ChapterChunk, ChunkRef, ChunkSource, DirSource, FilePack, MemorySource, PACK_FORMAT,
    PACK_FORMAT_VERSION, PackBook, PackChapter, PackManifest, PackWriter, WrittenPack,
};
pub use placeholder::{OverlayVerse, PlaceholderPack, placeholder_text};

/// One edition as a reader sees it: its names, language, direction, versification and book
/// order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edition {
    /// The resource id tabs store as `content_id` (`bsb`).
    pub resource_id: String,
    /// The revision anchors are made against (`bsb-2024`).
    pub content_revision: String,
    /// Short name for tab labels (`BSB`).
    pub title: String,
    /// Full name (`Berean Standard Bible`).
    pub name: String,
    /// BCP 47 language tag (`en`, `grc`, `hbo`).
    pub language: String,
    /// `ltr` or `rtl`.
    pub direction: String,
    /// The versification scheme its references are in (`eng`, `org`).
    pub scheme: String,
    /// The order its books are read in.
    pub book_order: canon::BookOrder,
    /// Whether the text is generated placeholder text (B078) rather than the edition.
    pub placeholder: bool,
}

/// One chapter of a book as a pack has it: the first and last verse numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterShape {
    /// Chapter number, 1-based.
    pub chapter: u32,
    /// First verse number (0 for a Psalm title the scheme numbers as verse 0).
    pub first_verse: u32,
    /// Last verse number.
    pub last_verse: u32,
}

/// One book of a pack: its title and chapters, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookShape {
    /// The book.
    pub book: BookId,
    /// The title a chapter heading shows (`John`, `Psalms`).
    pub title: String,
    /// Its chapters in order.
    pub chapters: Vec<ChapterShape>,
}

/// One token of a verse: its id and scalar range in the verse text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerseToken {
    /// The token id (`bsb:44003016001`).
    pub id: String,
    /// First scalar.
    pub start: u32,
    /// One past the last scalar.
    pub end: u32,
}

/// One verse (segment) of a chapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterVerse {
    /// The segment id (`John.3.16`, `Ps.3.0`, a bridge `Num.26.3-4`).
    pub segment_id: String,
    /// The first verse number it holds.
    pub number: u32,
    /// The last verse number it holds: `number` unless it is a bridge.
    #[serde(default)]
    pub last_number: u32,
    /// The label printed before it (`16`, `3–4`); empty for a Psalm title.
    #[serde(default)]
    pub label: String,
    /// The text, byte-exact.
    pub text: String,
    /// Whether a paragraph starts here.
    #[serde(default)]
    pub paragraph_start: bool,
    /// Whether it is set as poetry (a line of its own, indented).
    #[serde(default)]
    pub poetry: bool,
    /// Whether it is a Psalm title (verse 0 in `eng`, verse 1 in `org`).
    #[serde(default)]
    pub superscription: bool,
    /// Whether the text is a generated placeholder, not the edition's.
    #[serde(default)]
    pub placeholder: bool,
    /// Its tokens, when the pack has them; placeholder verses have none.
    #[serde(default)]
    pub tokens: Vec<VerseToken>,
}

/// The heading a chapter block starts with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterHeading {
    /// The book's title, on the book's first chapter only (`John`).
    pub book_title: Option<String>,
    /// The chapter's own label (`Chapter 3`, `Psalm 23`; the book title for a one-chapter
    /// book).
    pub chapter_label: String,
    /// Book and chapter in short form for a sticky header (`Jn 3`).
    pub short_label: String,
}

/// One chapter as a host renders it: what [`ReadingLibrary::chapter_content`] returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterBlock {
    /// The edition.
    pub resource_id: String,
    /// Its revision.
    pub content_revision: String,
    /// `Book.C` (`John.3`).
    pub chapter_key: String,
    /// The book, OSIS (`John`).
    pub book: String,
    /// The chapter number.
    pub chapter: u32,
    /// The heading.
    pub heading: ChapterHeading,
    /// The verses in order.
    pub verses: Vec<ChapterVerse>,
    /// The chapter before, in the edition's stream.
    pub prev: Option<String>,
    /// The chapter after.
    pub next: Option<String>,
    /// Whether any verse is placeholder text.
    pub placeholder: bool,
}

impl ChapterBlock {
    /// The verse holding verse `number` (a bridge holds several).
    #[must_use]
    pub fn verse_holding(&self, number: u32) -> Option<&ChapterVerse> {
        self.verses
            .iter()
            .find(|v| (v.number..=v.last_number.max(v.number)).contains(&number))
    }
}

/// The chapters around one chapter of an edition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterNeighbors {
    /// The chapter asked about.
    pub chapter_key: String,
    /// The chapter before, `None` at the start of the edition.
    pub prev: Option<String>,
    /// The chapter after, `None` at its end.
    pub next: Option<String>,
}

/// How much of the text a located passage names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Granularity {
    /// Verses (`John 3:16`, `John 3:16–18`).
    Verse,
    /// Whole chapters (`John 3`, `Gen 1–3`).
    Chapter,
    /// A whole book (`John`).
    Book,
}

/// Where a typed passage is in an edition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// The edition.
    pub resource_id: String,
    /// The chapter to load first.
    pub chapter_key: String,
    /// The segment to scroll to the top.
    pub segment_id: String,
    /// The passage located, OSIS (`John.3.16-John.3.18`, `John.3`, `John.1`).
    pub reference: String,
    /// The first verse of the passage, OSIS.
    pub verse_start: String,
    /// The last verse of the passage, OSIS (the end of the chapter or of chapter 1 for
    /// chapter and book passages).
    pub verse_end: String,
    /// The verses to mark (`John.3.16-John.3.18`); `None` for a chapter or book, which
    /// is not flashed.
    pub highlight_range: Option<String>,
    /// How much was named.
    pub granularity: Granularity,
}

/// Why a reading call failed. The `Display` text is plain words for a toast; the serde tag
/// (`kind`) is for code.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReadingError {
    /// No pack with this resource id is installed.
    #[error("No edition {resource_id} is installed")]
    UnknownResource {
        /// The id asked for.
        resource_id: String,
    },
    /// The input is not a passage.
    #[error("“{input}” is not a passage")]
    InvalidReference {
        /// The input.
        input: String,
        /// The parser's reason.
        detail: String,
    },
    /// The edition does not have the book (John in the WLC).
    #[error("{book} is not in {edition}")]
    BookNotInEdition {
        /// The edition's short name.
        edition: String,
        /// The book's title.
        book: String,
    },
    /// The book has fewer chapters.
    #[error("{book} has no chapter {chapter}")]
    ChapterOutOfRange {
        /// The book's title.
        book: String,
        /// The chapter asked for.
        chapter: u32,
        /// How many it has.
        chapters: u32,
    },
    /// The chapter does not have the verse.
    #[error("{chapter_title} has no verse {verse}")]
    VerseOutOfRange {
        /// Book and chapter (`John 3`).
        chapter_title: String,
        /// The verse asked for.
        verse: u32,
        /// Its last verse.
        last_verse: u32,
    },
    /// A chapter key the edition does not have, or that is not `Book.C`.
    #[error("{chapter_key} is not a chapter of {resource_id}")]
    UnknownChapter {
        /// The edition.
        resource_id: String,
        /// The key asked for.
        chapter_key: String,
    },
    /// A file pack's chunk the host has not supplied yet (a browser fetches it first).
    #[error("{path} is not loaded")]
    ChunkNotLoaded {
        /// The edition.
        resource_id: String,
        /// The chapter.
        chapter_key: String,
        /// The chunk's path in the pack.
        path: String,
    },
    /// A chunk whose bytes do not hash to the manifest's value.
    #[error("{path} does not match its hash")]
    HashMismatch {
        /// The chunk's path.
        path: String,
        /// The manifest's SHA-256.
        expected: String,
        /// The bytes' SHA-256.
        actual: String,
    },
    /// A pack that does not read: a bad manifest, chunk or path, or an I/O failure.
    #[error("Invalid text pack: {detail}")]
    InvalidPack {
        /// What was wrong.
        detail: String,
    },
}

/// What a reading surface reads chapters through: the placeholder pack today, B071's file
/// packs later. A pack is immutable once built, except that a file pack in a browser is
/// handed the chunks it was asked for ([`TextPack::supply_chunk`]).
pub trait TextPack: Send + Sync {
    /// The edition.
    fn edition(&self) -> &Edition;

    /// Its books in reading order, each with its chapters.
    fn books(&self) -> &[BookShape];

    /// One chapter's verses in order. The library has checked the chapter is in
    /// [`TextPack::books`].
    fn verses(&self, book: BookId, chapter: u32) -> Result<Vec<ChapterVerse>, ReadingError>;

    /// Where a chapter's bytes are, for a pack read from files; `None` for a generated one.
    fn chunk(&self, _book: BookId, _chapter: u32) -> Option<ChunkRef> {
        None
    }

    /// Hands a file pack the bytes of one chapter's chunk, checked against its hash.
    fn supply_chunk(&mut self, chapter_key: &str, _bytes: &[u8]) -> Result<(), ReadingError> {
        Err(ReadingError::UnknownChapter {
            resource_id: self.edition().resource_id.clone(),
            chapter_key: chapter_key.to_owned(),
        })
    }
}

struct Installed {
    pack: Box<dyn TextPack>,
    /// Book → its index in `pack.books()`.
    books: HashMap<BookId, usize>,
}

impl Installed {
    fn new(pack: Box<dyn TextPack>) -> Installed {
        let books = pack
            .books()
            .iter()
            .enumerate()
            .map(|(i, b)| (b.book, i))
            .collect();
        Installed { pack, books }
    }

    fn book(&self, book: BookId) -> Option<(usize, &BookShape)> {
        let i = *self.books.get(&book)?;
        Some((i, &self.pack.books()[i]))
    }

    fn chapter(&self, book: BookId, chapter: u32) -> Option<(usize, usize, ChapterShape)> {
        let (b, shape) = self.book(book)?;
        let c = shape.chapters.iter().position(|c| c.chapter == chapter)?;
        Some((b, c, shape.chapters[c]))
    }

    fn key_at(&self, b: usize, c: usize) -> String {
        let book = &self.pack.books()[b];
        chapter_key(book.book, book.chapters[c].chapter)
    }

    fn neighbors(&self, b: usize, c: usize) -> (Option<String>, Option<String>) {
        let books = self.pack.books();
        let prev = if c > 0 {
            Some(self.key_at(b, c - 1))
        } else {
            (0..b)
                .rev()
                .find(|i| !books[*i].chapters.is_empty())
                .map(|i| self.key_at(i, books[i].chapters.len() - 1))
        };
        let next = if c + 1 < books[b].chapters.len() {
            Some(self.key_at(b, c + 1))
        } else {
            (b + 1..books.len())
                .find(|i| !books[*i].chapters.is_empty())
                .map(|i| self.key_at(i, 0))
        };
        (prev, next)
    }
}

/// The installed editions, each one pack.
#[derive(Default)]
pub struct ReadingLibrary {
    packs: Vec<Installed>,
}

impl std::fmt::Debug for ReadingLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.packs.iter().map(|p| &p.pack.edition().resource_id))
            .finish()
    }
}

/// `Book.C`, the key of one chapter.
#[must_use]
pub fn chapter_key(book: BookId, chapter: u32) -> String {
    format!("{}.{chapter}", book.as_osis())
}

/// The book and chapter of a key (`John.3`); `None` when it is not one.
#[must_use]
pub fn parse_chapter_key(key: &str) -> Option<(BookId, u32)> {
    let (book, chapter) = key.split_once('.')?;
    let chapter: u32 = chapter.parse().ok().filter(|c| *c > 0)?;
    Some((BookId::from_osis(book)?, chapter))
}

/// The verse a segment starts at, OSIS (`John.3.16`; a bridge `Num.26.3-4` → `Num.26.3`), for
/// the verse at the top of the viewport; `None` for an id that names no verse.
#[must_use]
pub fn reference_for_segment(segment_id: &str) -> Option<String> {
    canon::segment_reference(segment_id).map(|r| r.to_osis())
}

fn english() -> &'static AliasTable {
    static TABLE: OnceLock<AliasTable> = OnceLock::new();
    TABLE.get_or_init(AliasTable::english)
}

/// A book's English title (`Song of Songs`), what a pack without its own titles shows.
#[must_use]
pub fn book_title(book: BookId) -> &'static str {
    book.title()
}

fn chapter_title(book: &BookShape, chapter: u32) -> String {
    if book.book.is_single_chapter() {
        book.title.clone()
    } else if book.book == BookId::Ps {
        format!("Psalm {chapter}")
    } else {
        format!("{} {chapter}", book.title)
    }
}

fn chapter_label(book: &BookShape, chapter: u32) -> String {
    if book.book.is_single_chapter() {
        book.title.clone()
    } else if book.book == BookId::Ps {
        format!("Psalm {chapter}")
    } else {
        format!("Chapter {chapter}")
    }
}

fn short_label(book: BookId, chapter: u32) -> String {
    use openlogos_core::reference::{ReferenceRange, ReferenceSet};
    ReferenceSet {
        ranges: vec![ReferenceRange::single(Reference::chapter(book, chapter))],
        flags: Vec::new(),
    }
    .to_short()
}

impl ReadingLibrary {
    /// An empty library.
    #[must_use]
    pub fn new() -> ReadingLibrary {
        ReadingLibrary::default()
    }

    /// Installs a pack, replacing one with the same resource id.
    pub fn insert(&mut self, pack: Box<dyn TextPack>) {
        let id = pack.edition().resource_id.clone();
        self.packs.retain(|p| p.pack.edition().resource_id != id);
        self.packs.push(Installed::new(pack));
    }

    /// Every installed edition, in install order.
    #[must_use]
    pub fn editions(&self) -> Vec<Edition> {
        self.packs
            .iter()
            .map(|p| p.pack.edition().clone())
            .collect()
    }

    /// Resource id → scheme, what a shell registers to map linked tabs.
    #[must_use]
    pub fn schemes(&self) -> std::collections::BTreeMap<String, String> {
        self.packs
            .iter()
            .map(|p| {
                let e = p.pack.edition();
                (e.resource_id.clone(), e.scheme.clone())
            })
            .collect()
    }

    fn installed(&self, resource_id: &str) -> Result<&Installed, ReadingError> {
        self.packs
            .iter()
            .find(|p| p.pack.edition().resource_id == resource_id)
            .ok_or_else(|| ReadingError::UnknownResource {
                resource_id: resource_id.to_owned(),
            })
    }

    /// The pack of an edition, to supply chunks to.
    pub fn pack_mut(
        &mut self,
        resource_id: &str,
    ) -> Result<&mut (dyn TextPack + 'static), ReadingError> {
        self.packs
            .iter_mut()
            .find(|p| p.pack.edition().resource_id == resource_id)
            .map(|p| p.pack.as_mut())
            .ok_or_else(|| ReadingError::UnknownResource {
                resource_id: resource_id.to_owned(),
            })
    }

    /// The books of an edition in reading order.
    pub fn books(&self, resource_id: &str) -> Result<&[BookShape], ReadingError> {
        Ok(self.installed(resource_id)?.pack.books())
    }

    /// Where a typed passage is: `John 3:16–18`, `Jn 3.16`, `John.3.16`, a chapter (`Ps 23`,
    /// `Gen 1–3`: its first chapter) or a book (`John`: its first verse). The passage is read
    /// in the edition's own versification. Of several ranges (`Rom 8:28, 31`) the first is
    /// located. Fails for an unknown edition, input that is not a passage, a book the edition
    /// does not have, and a chapter or verse past the end.
    pub fn locate(&self, resource_id: &str, input: &str) -> Result<Location, ReadingError> {
        let installed = self.installed(resource_id)?;
        let edition = installed.pack.edition();
        let invalid = |detail: String| ReadingError::InvalidReference {
            input: input.trim().to_owned(),
            detail,
        };
        let (start, end, granularity) = match parse(input, english()) {
            Ok(set) => {
                let range = set
                    .ranges
                    .first()
                    .ok_or_else(|| invalid("no passage".to_owned()))?;
                let granularity = if range.start.verse.is_some() {
                    Granularity::Verse
                } else {
                    Granularity::Chapter
                };
                (range.start.clone(), range.end.clone(), granularity)
            }
            // A bare book name (`John`) is not a citation, but it is a place to read from.
            Err(error) => match english().lookup(input.trim()).first() {
                Some(book) => {
                    let first = Reference::chapter(*book, 1);
                    (first.clone(), first, Granularity::Book)
                }
                None => return Err(invalid(error.to_string())),
            },
        };
        let check = |r: &Reference| -> Result<ChapterShape, ReadingError> {
            let (_, book) =
                installed
                    .book(r.book)
                    .ok_or_else(|| ReadingError::BookNotInEdition {
                        edition: edition.title.clone(),
                        book: book_title(r.book).to_owned(),
                    })?;
            let (_, _, shape) = installed.chapter(r.book, r.chapter).ok_or_else(|| {
                ReadingError::ChapterOutOfRange {
                    book: book.title.clone(),
                    chapter: r.chapter,
                    chapters: book.chapters.last().map_or(0, |c| c.chapter),
                }
            })?;
            if let Some(verse) = r.verse
                && !(shape.first_verse..=shape.last_verse).contains(&verse)
            {
                return Err(ReadingError::VerseOutOfRange {
                    chapter_title: chapter_title(book, r.chapter),
                    verse,
                    last_verse: shape.last_verse,
                });
            }
            Ok(shape)
        };
        let first = check(&start)?;
        let last = check(&end)?;
        let verse = |r: &Reference, v: u32| Reference::verse(r.book, r.chapter, v).to_osis();
        let verse_start = match start.verse {
            Some(v) => verse(&start, v),
            None => verse(&start, first.first_verse),
        };
        let verse_end = match (granularity, end.verse) {
            (Granularity::Verse, Some(v)) => verse(&end, v),
            _ => verse(&end, last.last_verse),
        };
        let reference = match granularity {
            Granularity::Verse if verse_start == verse_end => verse_start.clone(),
            Granularity::Verse => format!("{verse_start}-{verse_end}"),
            Granularity::Chapter if start == end => start.to_osis(),
            Granularity::Chapter => format!("{}-{}", start.to_osis(), end.to_osis()),
            Granularity::Book => start.book.as_osis().to_owned(),
        };
        Ok(Location {
            resource_id: resource_id.to_owned(),
            chapter_key: chapter_key(start.book, start.chapter),
            segment_id: verse_start.clone(),
            highlight_range: (granularity == Granularity::Verse).then(|| reference.clone()),
            reference,
            verse_start,
            verse_end,
            granularity,
        })
    }

    fn chapter_of(
        &self,
        resource_id: &str,
        key: &str,
    ) -> Result<(&Installed, usize, usize, ChapterShape), ReadingError> {
        let installed = self.installed(resource_id)?;
        let unknown = || ReadingError::UnknownChapter {
            resource_id: resource_id.to_owned(),
            chapter_key: key.to_owned(),
        };
        let (book, chapter) = parse_chapter_key(key).ok_or_else(unknown)?;
        let (b, c, shape) = installed.chapter(book, chapter).ok_or_else(unknown)?;
        Ok((installed, b, c, shape))
    }

    /// The chapters before and after one chapter in the edition's stream, crossing books
    /// (Malachi 4 → Matthew 1 in a whole English Bible); `None` at either end.
    pub fn chapter_neighbors(
        &self,
        resource_id: &str,
        chapter_key: &str,
    ) -> Result<ChapterNeighbors, ReadingError> {
        let (installed, b, c, _) = self.chapter_of(resource_id, chapter_key)?;
        let (prev, next) = installed.neighbors(b, c);
        Ok(ChapterNeighbors {
            chapter_key: chapter_key.to_owned(),
            prev,
            next,
        })
    }

    /// One chapter with its heading, verses and neighbours.
    pub fn chapter_content(
        &self,
        resource_id: &str,
        chapter_key: &str,
    ) -> Result<ChapterBlock, ReadingError> {
        let (installed, b, c, shape) = self.chapter_of(resource_id, chapter_key)?;
        let book = &installed.pack.books()[b];
        let verses = installed.pack.verses(book.book, shape.chapter)?;
        let (prev, next) = installed.neighbors(b, c);
        let edition = installed.pack.edition();
        Ok(ChapterBlock {
            resource_id: resource_id.to_owned(),
            content_revision: edition.content_revision.clone(),
            chapter_key: chapter_key.to_owned(),
            book: book.book.as_osis().to_owned(),
            chapter: shape.chapter,
            heading: ChapterHeading {
                book_title: (c == 0).then(|| book.title.clone()),
                chapter_label: chapter_label(book, shape.chapter),
                short_label: short_label(book.book, shape.chapter),
            },
            placeholder: verses.iter().any(|v| v.placeholder),
            verses,
            prev,
            next,
        })
    }

    /// Where a file pack's chapter chunk is, so a browser can fetch it before
    /// [`ReadingLibrary::chapter_content`]; `None` for a generated pack.
    pub fn chunk_for(
        &self,
        resource_id: &str,
        chapter_key: &str,
    ) -> Result<Option<ChunkRef>, ReadingError> {
        let (installed, b, _, shape) = self.chapter_of(resource_id, chapter_key)?;
        Ok(installed
            .pack
            .chunk(installed.pack.books()[b].book, shape.chapter))
    }

    /// Hands a file pack one chapter's chunk bytes (a browser, after fetching
    /// [`ReadingLibrary::chunk_for`]); checked against the manifest's hash.
    pub fn supply_chunk(
        &mut self,
        resource_id: &str,
        chapter_key: &str,
        bytes: &[u8],
    ) -> Result<(), ReadingError> {
        self.chapter_of(resource_id, chapter_key)?;
        self.pack_mut(resource_id)?.supply_chunk(chapter_key, bytes)
    }
}

/// The structure of a scheme's books from the canon table, as a placeholder pack has it.
#[must_use]
pub fn canon_books(scheme: &SchemeId, books: &[BookId], order: canon::BookOrder) -> Vec<BookShape> {
    order
        .arrange(books)
        .into_iter()
        .filter_map(|book| {
            let count = canon::chapter_count(scheme, book)?;
            let chapters = (1..=count)
                .filter_map(|chapter| {
                    Some(ChapterShape {
                        chapter,
                        first_verse: canon::first_verse(scheme, book, chapter)?,
                        last_verse: canon::last_verse(scheme, book, chapter)?,
                    })
                })
                .collect();
            Some(BookShape {
                book,
                title: book_title(book).to_owned(),
                chapters,
            })
        })
        .collect()
}
