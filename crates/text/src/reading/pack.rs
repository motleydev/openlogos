//! The text pack file format (B078, specs/23 §4): what B071's importer writes for the
//! installed BSB, SBLGNT and WLC, and what [`FilePack`] reads. A pack is a directory:
//!
//! ```text
//! manifest.json                 PackManifest: the edition, its books and chapters in reading
//!                               order, and per chapter where its chunk is and its SHA-256
//! chapters/John/3.json          one ChapterChunk per chapter (the default layout), or
//! chapters.bin                  every chunk concatenated, each found by offset and length
//! ```
//!
//! The chapter is the unit of loading: a reader opens a chapter by reading one chunk, from
//! disk natively or by a fetch in a browser (a whole file, or one byte range of the bundle),
//! so the format needs nothing but static files. Every chunk is checked against the
//! manifest's hash before it is used; the manifest's own SHA-256 is what a catalog records
//! (B068). Chunks are UTF-8 JSON with the text byte-exact (never normalized).

use std::collections::HashMap;
use std::path::PathBuf;

use openlogos_core::canon::BookOrder;
use openlogos_core::reference::BookId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{BookShape, ChapterShape, ChapterVerse, Edition, ReadingError, TextPack, chapter_key};

/// `format` of a manifest.
pub const PACK_FORMAT: &str = "openlogos-text-pack";

/// `format_version` this reader writes; it reads any `1.x`.
pub const PACK_FORMAT_VERSION: &str = "1.0";

/// `format` of a chapter chunk.
pub const CHUNK_FORMAT: &str = "openlogos-text-pack-chapter";

/// Where one chapter's chunk is and what it hashes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackChapter {
    /// Chapter number.
    pub chapter: u32,
    /// First verse number.
    pub first_verse: u32,
    /// Last verse number.
    pub last_verse: u32,
    /// The file holding the chunk, relative to the pack (`chapters/John/3.json`).
    pub path: String,
    /// Byte offset of the chunk in `path` when `path` is a bundle; absent for a file that
    /// is the chunk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
    /// Length of the chunk in bytes.
    pub length: u64,
    /// SHA-256 of the chunk's bytes, lower-case hex.
    pub sha256: String,
}

/// One book of a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackBook {
    /// The book, OSIS.
    pub book: BookId,
    /// The title its first chapter's heading shows, in the edition's language.
    pub title: String,
    /// Its chapters in order.
    pub chapters: Vec<PackChapter>,
}

/// `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackManifest {
    /// [`PACK_FORMAT`].
    pub format: String,
    /// `1.x`; a reader refuses another major.
    pub format_version: String,
    /// The resource id (`bsb`).
    pub resource_id: String,
    /// The revision anchors are made against.
    pub content_revision: String,
    /// Short name.
    pub title: String,
    /// Full name.
    pub name: String,
    /// BCP 47 language tag.
    pub language: String,
    /// `ltr` or `rtl`.
    pub direction: String,
    /// Versification scheme (`eng`, `org`).
    pub scheme: String,
    /// The order `books` is in.
    #[serde(default)]
    pub book_order: BookOrder,
    /// The text's licence, as the rights record states it.
    pub license: String,
    /// Where the text came from.
    pub source: String,
    /// Synthetic or fixture data, never an installed edition.
    #[serde(default)]
    pub fixture_only: bool,
    /// Whether every chapter has every verse of the scheme; `false` for an excerpt.
    pub complete: bool,
    /// Books in reading order.
    pub books: Vec<PackBook>,
}

/// One chapter's chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterChunk {
    /// [`CHUNK_FORMAT`].
    pub format: String,
    /// The resource id, as in the manifest.
    pub resource_id: String,
    /// `Book.C`.
    pub chapter_key: String,
    /// The verses in order.
    pub verses: Vec<ChapterVerse>,
}

/// Where a chapter's bytes are: what a browser fetches before handing them back with
/// [`super::ReadingLibrary::supply_chunk`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkRef {
    /// The edition.
    pub resource_id: String,
    /// The chapter.
    pub chapter_key: String,
    /// The file, relative to the pack.
    pub path: String,
    /// Byte offset in a bundle (fetch `Range: bytes=offset-(offset+length-1)`), else `None`.
    pub offset: Option<u64>,
    /// Length in bytes.
    pub length: u64,
    /// SHA-256, lower-case hex.
    pub sha256: String,
}

/// Where a file pack reads chunk bytes from.
pub trait ChunkSource: Send + Sync {
    /// `length` bytes of `path` from `offset` (the whole file when `offset` is `None`).
    fn read(&self, path: &str, offset: Option<u64>, length: u64) -> Result<Vec<u8>, ReadingError>;
}

/// A pack directory on disk.
#[derive(Debug, Clone)]
pub struct DirSource {
    root: PathBuf,
}

impl DirSource {
    /// The pack at `root` (the directory holding `manifest.json`).
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> DirSource {
        DirSource { root: root.into() }
    }
}

fn io(path: &str, error: &std::io::Error) -> ReadingError {
    ReadingError::InvalidPack {
        detail: format!("{path}: {error}"),
    }
}

impl ChunkSource for DirSource {
    fn read(&self, path: &str, offset: Option<u64>, length: u64) -> Result<Vec<u8>, ReadingError> {
        use std::io::{Read, Seek, SeekFrom};
        check_path(path)?;
        let full = self.root.join(path);
        match offset {
            None => std::fs::read(&full).map_err(|e| io(path, &e)),
            Some(offset) => {
                let mut file = std::fs::File::open(&full).map_err(|e| io(path, &e))?;
                file.seek(SeekFrom::Start(offset))
                    .map_err(|e| io(path, &e))?;
                let mut bytes = vec![0; usize::try_from(length).unwrap_or(0)];
                file.read_exact(&mut bytes).map_err(|e| io(path, &e))?;
                Ok(bytes)
            }
        }
    }
}

/// Pack files held in memory (tests, the conformance fixture packs, a host that has the
/// whole pack in hand).
#[derive(Debug, Clone, Default)]
pub struct MemorySource {
    files: HashMap<String, Vec<u8>>,
}

impl MemorySource {
    /// Files by path.
    #[must_use]
    pub fn new(files: impl IntoIterator<Item = (String, Vec<u8>)>) -> MemorySource {
        MemorySource {
            files: files.into_iter().collect(),
        }
    }
}

impl ChunkSource for MemorySource {
    fn read(&self, path: &str, offset: Option<u64>, length: u64) -> Result<Vec<u8>, ReadingError> {
        let bytes = self
            .files
            .get(path)
            .ok_or_else(|| ReadingError::InvalidPack {
                detail: format!("{path}: no such file"),
            })?;
        let start = usize::try_from(offset.unwrap_or(0)).unwrap_or(usize::MAX);
        let end = match offset {
            None => bytes.len(),
            Some(_) => start.saturating_add(usize::try_from(length).unwrap_or(usize::MAX)),
        };
        bytes
            .get(start..end)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| ReadingError::InvalidPack {
                detail: format!("{path}: bytes {start}..{end} are past its end"),
            })
    }
}

/// A relative path inside the pack: no root, no `..`, no backslash, nothing but ASCII
/// letters, digits, `.`, `_`, `-` and `/`.
fn check_path(path: &str) -> Result<(), ReadingError> {
    let ok = !path.is_empty()
        && !path.starts_with('/')
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'));
    if ok {
        Ok(())
    } else {
        Err(ReadingError::InvalidPack {
            detail: format!("{path} is not a path inside the pack"),
        })
    }
}

/// SHA-256 of bytes, lower-case hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

fn invalid(detail: impl Into<String>) -> ReadingError {
    ReadingError::InvalidPack {
        detail: detail.into(),
    }
}

/// A pack read through its manifest. Chunks come from the [`ChunkSource`] when it has one,
/// else only from [`TextPack::supply_chunk`] (a browser fetches them).
pub struct FilePack {
    manifest: PackManifest,
    edition: Edition,
    books: Vec<BookShape>,
    source: Option<Box<dyn ChunkSource>>,
    supplied: HashMap<String, Vec<ChapterVerse>>,
}

impl std::fmt::Debug for FilePack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilePack")
            .field("resource_id", &self.manifest.resource_id)
            .field("supplied", &self.supplied.len())
            .finish_non_exhaustive()
    }
}

impl FilePack {
    /// A pack from its manifest's JSON, reading chunks through `source` (`None` in a
    /// browser). Checks the format, version, paths, hashes' shape and the chapter order.
    pub fn from_manifest(
        json: &str,
        source: Option<Box<dyn ChunkSource>>,
    ) -> Result<FilePack, ReadingError> {
        let manifest: PackManifest =
            serde_json::from_str(json).map_err(|e| invalid(format!("manifest: {e}")))?;
        if manifest.format != PACK_FORMAT {
            return Err(invalid(format!(
                "manifest format is {}, not {PACK_FORMAT}",
                manifest.format
            )));
        }
        if manifest.format_version.split('.').next() != Some("1") {
            return Err(invalid(format!(
                "format version {} is not 1.x",
                manifest.format_version
            )));
        }
        if manifest.resource_id.is_empty() || manifest.scheme.is_empty() {
            return Err(invalid("a manifest names its resource and scheme"));
        }
        let mut seen = Vec::new();
        for book in &manifest.books {
            if seen.contains(&book.book) {
                return Err(invalid(format!("{} is listed twice", book.book)));
            }
            seen.push(book.book);
            let mut last = 0;
            for c in &book.chapters {
                check_path(&c.path)?;
                if c.chapter <= last || c.first_verse > c.last_verse {
                    return Err(invalid(format!(
                        "{}.{} is out of order",
                        book.book, c.chapter
                    )));
                }
                if c.sha256.len() != 64 || !c.sha256.chars().all(|h| h.is_ascii_hexdigit()) {
                    return Err(invalid(format!(
                        "{}.{} has no SHA-256",
                        book.book, c.chapter
                    )));
                }
                last = c.chapter;
            }
        }
        let edition = Edition {
            resource_id: manifest.resource_id.clone(),
            content_revision: manifest.content_revision.clone(),
            title: manifest.title.clone(),
            name: manifest.name.clone(),
            language: manifest.language.clone(),
            direction: manifest.direction.clone(),
            scheme: manifest.scheme.clone(),
            book_order: manifest.book_order,
            placeholder: false,
        };
        let books = manifest
            .books
            .iter()
            .map(|b| BookShape {
                book: b.book,
                title: b.title.clone(),
                chapters: b
                    .chapters
                    .iter()
                    .map(|c| ChapterShape {
                        chapter: c.chapter,
                        first_verse: c.first_verse,
                        last_verse: c.last_verse,
                    })
                    .collect(),
            })
            .collect();
        Ok(FilePack {
            manifest,
            edition,
            books,
            source,
            supplied: HashMap::new(),
        })
    }

    /// The manifest as read.
    #[must_use]
    pub fn manifest(&self) -> &PackManifest {
        &self.manifest
    }

    fn entry(&self, book: BookId, chapter: u32) -> Option<&PackChapter> {
        self.manifest
            .books
            .iter()
            .find(|b| b.book == book)?
            .chapters
            .iter()
            .find(|c| c.chapter == chapter)
    }

    /// Checks a chunk's bytes against its entry and parses it.
    fn parse_chunk(
        &self,
        key: &str,
        entry: &PackChapter,
        bytes: &[u8],
    ) -> Result<Vec<ChapterVerse>, ReadingError> {
        let actual = sha256_hex(bytes);
        if actual != entry.sha256.to_ascii_lowercase() {
            return Err(ReadingError::HashMismatch {
                path: entry.path.clone(),
                expected: entry.sha256.clone(),
                actual,
            });
        }
        let chunk: ChapterChunk =
            serde_json::from_slice(bytes).map_err(|e| invalid(format!("{key}: {e}")))?;
        if chunk.format != CHUNK_FORMAT
            || chunk.chapter_key != key
            || chunk.resource_id != self.manifest.resource_id
        {
            return Err(invalid(format!(
                "{} holds {} of {}, not {key}",
                entry.path, chunk.chapter_key, chunk.resource_id
            )));
        }
        Ok(chunk
            .verses
            .into_iter()
            .map(|mut v| {
                v.last_number = v.last_number.max(v.number);
                if v.label.is_empty() && !v.superscription {
                    v.label = if v.last_number > v.number {
                        format!("{}–{}", v.number, v.last_number)
                    } else {
                        v.number.to_string()
                    };
                }
                v
            })
            .collect())
    }
}

impl TextPack for FilePack {
    fn edition(&self) -> &Edition {
        &self.edition
    }

    fn books(&self) -> &[BookShape] {
        &self.books
    }

    fn verses(&self, book: BookId, chapter: u32) -> Result<Vec<ChapterVerse>, ReadingError> {
        let key = chapter_key(book, chapter);
        if let Some(verses) = self.supplied.get(&key) {
            return Ok(verses.clone());
        }
        let entry = self
            .entry(book, chapter)
            .ok_or_else(|| ReadingError::UnknownChapter {
                resource_id: self.edition.resource_id.clone(),
                chapter_key: key.clone(),
            })?;
        let Some(source) = &self.source else {
            return Err(ReadingError::ChunkNotLoaded {
                resource_id: self.edition.resource_id.clone(),
                chapter_key: key,
                path: entry.path.clone(),
            });
        };
        let bytes = source.read(&entry.path, entry.offset, entry.length)?;
        self.parse_chunk(&key, entry, &bytes)
    }

    fn chunk(&self, book: BookId, chapter: u32) -> Option<ChunkRef> {
        let entry = self.entry(book, chapter)?;
        Some(ChunkRef {
            resource_id: self.edition.resource_id.clone(),
            chapter_key: chapter_key(book, chapter),
            path: entry.path.clone(),
            offset: entry.offset,
            length: entry.length,
            sha256: entry.sha256.clone(),
        })
    }

    fn supply_chunk(&mut self, chapter_key: &str, bytes: &[u8]) -> Result<(), ReadingError> {
        let unknown = || ReadingError::UnknownChapter {
            resource_id: self.edition.resource_id.clone(),
            chapter_key: chapter_key.to_owned(),
        };
        let (book, chapter) = super::parse_chapter_key(chapter_key).ok_or_else(unknown)?;
        let entry = self.entry(book, chapter).ok_or_else(unknown)?.clone();
        let verses = self.parse_chunk(chapter_key, &entry, bytes)?;
        self.supplied.insert(chapter_key.to_owned(), verses);
        Ok(())
    }
}

/// Writes a pack: the manifest and the chunk files, chapter files by default or one
/// `chapters.bin` bundle. What B071's importer calls once per edition; the fixture packs
/// are written with it.
#[derive(Debug, Clone)]
pub struct PackWriter {
    manifest: PackManifest,
    chapters: Vec<(BookId, String, u32, Vec<ChapterVerse>)>,
    bundle: bool,
}

/// What [`PackWriter::finish`] produces: the manifest and the chunk files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenPack {
    /// `manifest.json`, pretty-printed.
    pub manifest_json: String,
    /// Every other file, by path relative to the pack.
    pub files: Vec<(String, Vec<u8>)>,
}

/// The bundle's path when [`PackWriter::bundled`] is used.
pub const BUNDLE_PATH: &str = "chapters.bin";

impl PackWriter {
    /// A writer for an edition, with its licence and source.
    #[must_use]
    pub fn new(edition: &Edition, license: &str, source: &str, fixture_only: bool) -> PackWriter {
        PackWriter {
            manifest: PackManifest {
                format: PACK_FORMAT.to_owned(),
                format_version: PACK_FORMAT_VERSION.to_owned(),
                resource_id: edition.resource_id.clone(),
                content_revision: edition.content_revision.clone(),
                title: edition.title.clone(),
                name: edition.name.clone(),
                language: edition.language.clone(),
                direction: edition.direction.clone(),
                scheme: edition.scheme.clone(),
                book_order: edition.book_order,
                license: license.to_owned(),
                source: source.to_owned(),
                fixture_only,
                complete: false,
                books: Vec::new(),
            },
            chapters: Vec::new(),
            bundle: false,
        }
    }

    /// Writes every chunk into one [`BUNDLE_PATH`] instead of a file per chapter.
    #[must_use]
    pub fn bundled(mut self) -> PackWriter {
        self.bundle = true;
        self
    }

    /// Marks the pack as having every verse of its scheme.
    #[must_use]
    pub fn complete(mut self, complete: bool) -> PackWriter {
        self.manifest.complete = complete;
        self
    }

    /// Adds a chapter (in reading order) with the title its book's heading shows.
    pub fn chapter(&mut self, book: BookId, title: &str, chapter: u32, verses: Vec<ChapterVerse>) {
        self.chapters
            .push((book, title.to_owned(), chapter, verses));
    }

    /// The manifest JSON and every file, by path. Fails for a chapter without verses.
    pub fn finish(self) -> Result<WrittenPack, ReadingError> {
        let mut manifest = self.manifest;
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        let mut bundle: Vec<u8> = Vec::new();
        for (book, title, chapter, verses) in self.chapters {
            let (Some(first), Some(last)) = (verses.first(), verses.last()) else {
                return Err(invalid(format!("{book}.{chapter} has no verses")));
            };
            let (first_verse, last_verse) = (first.number, last.last_number.max(last.number));
            let key = chapter_key(book, chapter);
            let chunk = ChapterChunk {
                format: CHUNK_FORMAT.to_owned(),
                resource_id: manifest.resource_id.clone(),
                chapter_key: key,
                verses,
            };
            let mut bytes = serde_json::to_vec(&chunk).map_err(|e| invalid(e.to_string()))?;
            bytes.push(b'\n');
            let entry = PackChapter {
                chapter,
                first_verse,
                last_verse,
                path: if self.bundle {
                    BUNDLE_PATH.to_owned()
                } else {
                    format!("chapters/{}/{chapter}.json", book.as_osis())
                },
                offset: self.bundle.then_some(bundle.len() as u64),
                length: bytes.len() as u64,
                sha256: sha256_hex(&bytes),
            };
            if self.bundle {
                bundle.extend_from_slice(&bytes);
            } else {
                files.push((entry.path.clone(), bytes));
            }
            match manifest.books.iter_mut().find(|b| b.book == book) {
                Some(b) => b.chapters.push(entry),
                None => manifest.books.push(PackBook {
                    book,
                    title,
                    chapters: vec![entry],
                }),
            }
        }
        if self.bundle {
            files.push((BUNDLE_PATH.to_owned(), bundle));
        }
        let mut json =
            serde_json::to_string_pretty(&manifest).map_err(|e| invalid(e.to_string()))?;
        json.push('\n');
        Ok(WrittenPack {
            manifest_json: json,
            files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reading::{ReadingLibrary, VerseToken};

    fn edition() -> Edition {
        Edition {
            resource_id: "test".to_owned(),
            content_revision: "t1".to_owned(),
            title: "TEST".to_owned(),
            name: "Test".to_owned(),
            language: "en".to_owned(),
            direction: "ltr".to_owned(),
            scheme: "eng".to_owned(),
            book_order: BookOrder::Protestant,
            placeholder: false,
        }
    }

    fn verse(id: &str, n: u32, text: &str) -> ChapterVerse {
        ChapterVerse {
            segment_id: id.to_owned(),
            number: n,
            last_number: n,
            label: n.to_string(),
            text: text.to_owned(),
            paragraph_start: n == 1,
            poetry: false,
            superscription: false,
            placeholder: false,
            tokens: vec![VerseToken {
                id: format!("test:{n}"),
                start: 0,
                end: 1,
            }],
        }
    }

    fn written(bundle: bool) -> (String, Vec<(String, Vec<u8>)>) {
        let pack = writer(bundle).finish().unwrap();
        (pack.manifest_json, pack.files)
    }

    fn writer(bundle: bool) -> PackWriter {
        let mut w = PackWriter::new(&edition(), "CC0", "synthetic", true);
        if bundle {
            w = w.bundled();
        }
        w.chapter(BookId::Jude, "Jude", 1, vec![verse("Jude.1.1", 1, "a")]);
        w.chapter(
            BookId::Rev,
            "Revelation",
            1,
            vec![verse("Rev.1.1", 1, "b"), verse("Rev.1.2", 2, "c")],
        );
        w
    }

    #[test]
    fn a_written_pack_reads_back_from_files_and_from_a_bundle() {
        for bundle in [false, true] {
            let (manifest, files) = written(bundle);
            let pack = FilePack::from_manifest(
                &manifest,
                Some(Box::new(MemorySource::new(files.clone()))),
            )
            .unwrap();
            let mut library = ReadingLibrary::new();
            library.insert(Box::new(pack));
            let rev = library.chapter_content("test", "Rev.1").unwrap();
            assert_eq!(rev.verses.len(), 2);
            assert_eq!(rev.verses[1].text, "c");
            assert_eq!(rev.prev.as_deref(), Some("Jude.1"));
            assert_eq!(rev.heading.book_title.as_deref(), Some("Revelation"));
            assert!(!rev.placeholder);
            assert_eq!(files.len(), if bundle { 1 } else { 2 });
        }
    }

    #[test]
    fn a_browser_pack_waits_for_its_chunks_and_checks_them() {
        let (manifest, files) = written(false);
        let mut library = ReadingLibrary::new();
        library.insert(Box::new(FilePack::from_manifest(&manifest, None).unwrap()));
        let chunk = library.chunk_for("test", "Rev.1").unwrap().unwrap();
        assert_eq!(chunk.path, "chapters/Rev/1.json");
        assert!(matches!(
            library.chapter_content("test", "Rev.1"),
            Err(ReadingError::ChunkNotLoaded { .. })
        ));
        let bytes = &files.iter().find(|(p, _)| *p == chunk.path).unwrap().1;
        let mut tampered = bytes.clone();
        tampered[10] ^= 1;
        assert!(matches!(
            library.supply_chunk("test", "Rev.1", &tampered),
            Err(ReadingError::HashMismatch { .. })
        ));
        library.supply_chunk("test", "Rev.1", bytes).unwrap();
        assert_eq!(
            library.chapter_content("test", "Rev.1").unwrap().verses[0].text,
            "b"
        );
    }

    #[test]
    fn manifests_and_paths_are_checked() {
        let (manifest, _) = written(false);
        let bad = manifest.replace("chapters/Rev/1.json", "../Rev/1.json");
        assert!(FilePack::from_manifest(&bad, None).is_err());
        let bad = manifest.replace("\"1.0\"", "\"2.0\"");
        assert!(FilePack::from_manifest(&bad, None).is_err());
        assert!(check_path("chapters/John/3.json").is_ok());
        assert!(check_path("/etc/passwd").is_err());
        assert!(check_path("a\\b").is_err());
    }
}
