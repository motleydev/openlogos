//! Continuous reading over `crates/text` reading (B078, specs/23): a [`ReadingHandle`]
//! holding the installed editions, one text pack each, and answering `locate`,
//! `chapter_neighbors` and `chapter_content`; `reference_for_segment` for the verse at the top
//! of the viewport. Records mirror the core's JSON field for field (the wasm surface returns
//! that JSON), so the two hosts read the same shapes.
//!
//! `ReadingHandle::placeholder()` is the placeholder library: BSB, SBLGNT and WLC generated
//! from the canon table with the conformance fixtures' verses overlaid, until B071's packs
//! are installed with `open_pack` (a pack directory on disk). `add_pack_manifest` and
//! `supply_chunk` are the browser's path, kept here so the native tests cover it too.

use std::sync::{Arc, Mutex};

use openlogos_text::reading::{
    self as core, ChapterBlock, ChunkRef, DirSource, Edition, FilePack, Location, ReadingLibrary,
};

/// Why a reading call failed. `kind` is the core's tag (`verse_out_of_range`,
/// `book_not_in_edition`, `chunk_not_loaded`, ...); `reason` is plain words for a toast. No
/// field is called `message` or `cause` (Kotlin's `Throwable`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum ReadingCallError {
    /// The core refused the call.
    #[error("{reason}")]
    Refused {
        /// The core's tag.
        kind: String,
        /// Plain words.
        reason: String,
    },
}

impl From<core::ReadingError> for ReadingCallError {
    fn from(error: core::ReadingError) -> Self {
        ReadingCallError::Refused {
            kind: serde_json::to_value(&error)
                .ok()
                .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_owned))
                .unwrap_or_default(),
            reason: error.to_string(),
        }
    }
}

fn name_of<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// One installed edition.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReadingEdition {
    /// The resource id tabs store (`bsb`).
    pub resource_id: String,
    /// The revision anchors are made against.
    pub content_revision: String,
    /// Short name (`BSB`).
    pub title: String,
    /// Full name.
    pub name: String,
    /// BCP 47 tag (`en`, `grc`, `hbo`).
    pub language: String,
    /// `ltr` or `rtl`.
    pub direction: String,
    /// Versification scheme (`eng`, `org`).
    pub scheme: String,
    /// `protestant` or `tanakh_bhs`.
    pub book_order: String,
    /// Whether its text is generated placeholder text.
    pub placeholder: bool,
    /// The attribution a reader must be shown, from the pack's manifest; `None` for a
    /// placeholder edition.
    pub attribution: Option<String>,
}

impl From<Edition> for ReadingEdition {
    fn from(e: Edition) -> Self {
        ReadingEdition {
            book_order: name_of(&e.book_order),
            resource_id: e.resource_id,
            content_revision: e.content_revision,
            title: e.title,
            name: e.name,
            language: e.language,
            direction: e.direction,
            scheme: e.scheme,
            placeholder: e.placeholder,
            attribution: e.attribution,
        }
    }
}

/// Where a typed passage is (`reading::Location`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReadingLocation {
    /// The edition.
    pub resource_id: String,
    /// The chapter to load first (`John.3`).
    pub chapter_key: String,
    /// The segment to scroll to the top.
    pub segment_id: String,
    /// The passage, OSIS.
    pub reference: String,
    /// Its first verse, OSIS.
    pub verse_start: String,
    /// Its last verse, OSIS.
    pub verse_end: String,
    /// The verses to mark; `None` for a chapter or book.
    pub highlight_range: Option<String>,
    /// `verse`, `chapter` or `book`.
    pub granularity: String,
}

impl From<Location> for ReadingLocation {
    fn from(l: Location) -> Self {
        ReadingLocation {
            granularity: name_of(&l.granularity),
            resource_id: l.resource_id,
            chapter_key: l.chapter_key,
            segment_id: l.segment_id,
            reference: l.reference,
            verse_start: l.verse_start,
            verse_end: l.verse_end,
            highlight_range: l.highlight_range,
        }
    }
}

/// The chapters around a chapter.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReadingNeighbors {
    /// The chapter asked about.
    pub chapter_key: String,
    /// The chapter before, `None` at the start.
    pub prev: Option<String>,
    /// The chapter after, `None` at the end.
    pub next: Option<String>,
}

/// One token of a verse.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReadingToken {
    /// The token id (`bsb:44003016001`).
    pub id: String,
    /// First scalar.
    pub start: u32,
    /// One past the last scalar.
    pub end: u32,
}

/// One verse of a chapter.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReadingVerse {
    /// The segment id.
    pub segment_id: String,
    /// The first verse number it holds.
    pub number: u32,
    /// The last (a bridge holds several).
    pub last_number: u32,
    /// The printed label; empty for a Psalm title.
    pub label: String,
    /// The text, byte-exact.
    pub text: String,
    /// Whether a paragraph starts here.
    pub paragraph_start: bool,
    /// Whether it is set as poetry.
    pub poetry: bool,
    /// Whether it is a Psalm title.
    pub superscription: bool,
    /// Whether the text is a generated placeholder.
    pub placeholder: bool,
    /// Its tokens; none for a placeholder.
    pub tokens: Vec<ReadingToken>,
}

/// A chapter's heading.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReadingHeading {
    /// The book's title, on its first chapter only.
    pub book_title: Option<String>,
    /// `Chapter 3`, `Psalm 23`, or the book title for a one-chapter book.
    pub chapter_label: String,
    /// `Jn 3`, for a sticky header.
    pub short_label: String,
}

/// One chapter as a host renders it (`reading::ChapterBlock`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReadingChapter {
    /// The edition.
    pub resource_id: String,
    /// Its revision.
    pub content_revision: String,
    /// `John.3`.
    pub chapter_key: String,
    /// The book, OSIS.
    pub book: String,
    /// The chapter number.
    pub chapter: u32,
    /// The heading.
    pub heading: ReadingHeading,
    /// The verses in order.
    pub verses: Vec<ReadingVerse>,
    /// The chapter before.
    pub prev: Option<String>,
    /// The chapter after.
    pub next: Option<String>,
    /// Whether any verse is placeholder text.
    pub placeholder: bool,
}

impl From<ChapterBlock> for ReadingChapter {
    fn from(b: ChapterBlock) -> Self {
        ReadingChapter {
            resource_id: b.resource_id,
            content_revision: b.content_revision,
            chapter_key: b.chapter_key,
            book: b.book,
            chapter: b.chapter,
            heading: ReadingHeading {
                book_title: b.heading.book_title,
                chapter_label: b.heading.chapter_label,
                short_label: b.heading.short_label,
            },
            verses: b
                .verses
                .into_iter()
                .map(|v| ReadingVerse {
                    segment_id: v.segment_id,
                    number: v.number,
                    last_number: v.last_number,
                    label: v.label,
                    text: v.text,
                    paragraph_start: v.paragraph_start,
                    poetry: v.poetry,
                    superscription: v.superscription,
                    placeholder: v.placeholder,
                    tokens: v
                        .tokens
                        .into_iter()
                        .map(|t| ReadingToken {
                            id: t.id,
                            start: t.start,
                            end: t.end,
                        })
                        .collect(),
                })
                .collect(),
            prev: b.prev,
            next: b.next,
            placeholder: b.placeholder,
        }
    }
}

/// Where a file pack's chapter chunk is (`reading::ChunkRef`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReadingChunk {
    /// The edition.
    pub resource_id: String,
    /// The chapter.
    pub chapter_key: String,
    /// The file, relative to the pack.
    pub path: String,
    /// Byte offset in a bundle, else `None`.
    pub offset: Option<u64>,
    /// Length in bytes.
    pub length: u64,
    /// SHA-256, lower-case hex.
    pub sha256: String,
}

impl From<ChunkRef> for ReadingChunk {
    fn from(c: ChunkRef) -> Self {
        ReadingChunk {
            resource_id: c.resource_id,
            chapter_key: c.chapter_key,
            path: c.path,
            offset: c.offset,
            length: c.length,
            sha256: c.sha256,
        }
    }
}

/// The installed editions. One per process is enough; calls cost microseconds for the
/// placeholder pack and one file read for a pack on disk.
#[derive(uniffi::Object)]
pub struct ReadingHandle {
    library: Mutex<ReadingLibrary>,
}

impl ReadingHandle {
    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, ReadingLibrary> {
        self.library
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Whether a segment is placeholder text (B074: never highlighted); `false` when the
    /// library does not know it.
    pub(crate) fn is_placeholder(&self, resource_id: &str, segment_id: &str) -> bool {
        self.lock().segment_is_placeholder(resource_id, segment_id) == Some(true)
    }

    fn install(&self, pack: FilePack) -> ReadingEdition {
        let edition = core::TextPack::edition(&pack).clone();
        self.lock().insert(Box::new(pack));
        edition.into()
    }
}

#[uniffi::export]
impl ReadingHandle {
    /// No editions.
    #[uniffi::constructor]
    pub fn new() -> Arc<ReadingHandle> {
        Arc::new(ReadingHandle {
            library: Mutex::new(ReadingLibrary::new()),
        })
    }

    /// The placeholder library: BSB (66 books, `eng`), SBLGNT (New Testament, `org`) and
    /// WLC (Old Testament, `org`) from the canon table, the conformance fixtures' verses
    /// overlaid.
    #[uniffi::constructor]
    pub fn placeholder() -> Arc<ReadingHandle> {
        Arc::new(ReadingHandle {
            library: Mutex::new(openlogos_conformance::reading_library()),
        })
    }

    /// Installs the pack in `directory` (its `manifest.json` and chunks), replacing an
    /// edition with the same resource id.
    pub fn open_pack(&self, directory: String) -> Result<ReadingEdition, ReadingCallError> {
        let manifest =
            std::fs::read_to_string(std::path::Path::new(&directory).join("manifest.json"))
                .map_err(|e| core::ReadingError::InvalidPack {
                    detail: format!("{directory}/manifest.json: {e}"),
                })?;
        let pack = FilePack::from_manifest(&manifest, Some(Box::new(DirSource::new(directory))))?;
        Ok(self.install(pack))
    }

    /// Installs a pack from its manifest alone: its chunks arrive by `supply_chunk` (what a
    /// browser does after fetching `chunk_for`).
    pub fn add_pack_manifest(
        &self,
        manifest_json: String,
    ) -> Result<ReadingEdition, ReadingCallError> {
        Ok(self.install(FilePack::from_manifest(&manifest_json, None)?))
    }

    /// Every installed edition, in install order.
    pub fn editions(&self) -> Vec<ReadingEdition> {
        self.lock().editions().into_iter().map(Into::into).collect()
    }

    /// Resource id → scheme, for `ShellHandle.set_resource_schemes`.
    pub fn schemes(&self) -> std::collections::HashMap<String, String> {
        self.lock().schemes().into_iter().collect()
    }

    /// Where a typed passage is in an edition: ranges (`John 3:16–18`), chapters (`Ps 23`),
    /// books (`John`); fails with `verse_out_of_range`, `chapter_out_of_range`,
    /// `book_not_in_edition`, `invalid_reference` or `unknown_resource`.
    pub fn locate(
        &self,
        resource_id: String,
        input: String,
    ) -> Result<ReadingLocation, ReadingCallError> {
        Ok(self.lock().locate(&resource_id, &input)?.into())
    }

    /// The chapters before and after one, across books.
    pub fn chapter_neighbors(
        &self,
        resource_id: String,
        chapter_key: String,
    ) -> Result<ReadingNeighbors, ReadingCallError> {
        let n = self.lock().chapter_neighbors(&resource_id, &chapter_key)?;
        Ok(ReadingNeighbors {
            chapter_key: n.chapter_key,
            prev: n.prev,
            next: n.next,
        })
    }

    /// One chapter with its heading and verses.
    pub fn chapter_content(
        &self,
        resource_id: String,
        chapter_key: String,
    ) -> Result<ReadingChapter, ReadingCallError> {
        Ok(self
            .lock()
            .chapter_content(&resource_id, &chapter_key)?
            .into())
    }

    /// Where a file pack's chapter chunk is; `None` for a generated edition.
    pub fn chunk_for(
        &self,
        resource_id: String,
        chapter_key: String,
    ) -> Result<Option<ReadingChunk>, ReadingCallError> {
        Ok(self
            .lock()
            .chunk_for(&resource_id, &chapter_key)?
            .map(Into::into))
    }

    /// Hands a pack one chapter's chunk, checked against its SHA-256.
    pub fn supply_chunk(
        &self,
        resource_id: String,
        chapter_key: String,
        bytes: Vec<u8>,
    ) -> Result<(), ReadingCallError> {
        Ok(self
            .lock()
            .supply_chunk(&resource_id, &chapter_key, &bytes)?)
    }
}

/// The verse a segment starts at, OSIS (`John.3.16`; a bridge `Num.26.3-4` → `Num.26.3`), for
/// the verse at the top of the viewport; `None` for an id that names no verse.
#[uniffi::export]
#[must_use]
pub fn reference_for_segment(segment_id: String) -> Option<String> {
    core::reference_for_segment(&segment_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_placeholder_handle_reads_and_locates() {
        let handle = ReadingHandle::placeholder();
        assert_eq!(handle.editions().len(), 3);
        assert_eq!(handle.schemes()["bsb"], "eng");
        let l = handle.locate("bsb".into(), "Jn 3:16-18".into()).unwrap();
        assert_eq!(l.granularity, "verse");
        let c = handle
            .chapter_content("bsb".into(), l.chapter_key.clone())
            .unwrap();
        assert_eq!(c.verses.len(), 36);
        assert!(!c.verses[15].placeholder);
        let n = handle
            .chapter_neighbors("bsb".into(), "Mal.4".into())
            .unwrap();
        assert_eq!(n.next.as_deref(), Some("Matt.1"));
        let error = handle.locate("wlc".into(), "John 3:16".into()).unwrap_err();
        assert_eq!(
            error,
            ReadingCallError::Refused {
                kind: "book_not_in_edition".into(),
                reason: "John is not in WLC".into()
            }
        );
        assert_eq!(
            reference_for_segment("Num.26.3-4".into()).as_deref(),
            Some("Num.26.3")
        );
    }

    #[test]
    fn a_pack_opens_from_disk_and_from_supplied_chunks() {
        let root = std::env::temp_dir().join(format!("openlogos-b078-{}", std::process::id()));
        let (_, pack) = openlogos_conformance::fixture_packs().remove(0);
        for (path, bytes) in &pack.files {
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, bytes).unwrap();
        }
        std::fs::write(root.join("manifest.json"), &pack.manifest_json).unwrap();
        let disk = ReadingHandle::new();
        let edition = disk.open_pack(root.display().to_string()).unwrap();
        assert_eq!(edition.resource_id, "bsb");
        assert_eq!(
            disk.chapter_content("bsb".into(), "John.3".into())
                .unwrap()
                .verses
                .len(),
            2
        );
        std::fs::remove_dir_all(&root).ok();

        let browser = ReadingHandle::new();
        browser
            .add_pack_manifest(pack.manifest_json.clone())
            .unwrap();
        let chunk = browser
            .chunk_for("bsb".into(), "John.3".into())
            .unwrap()
            .unwrap();
        let bytes = pack
            .files
            .iter()
            .find(|(p, _)| *p == chunk.path)
            .unwrap()
            .1
            .clone();
        browser
            .supply_chunk("bsb".into(), "John.3".into(), bytes)
            .unwrap();
        assert_eq!(
            browser
                .chapter_content("bsb".into(), "John.3".into())
                .unwrap()
                .verses[0]
                .segment_id,
            "John.3.16"
        );
    }
}
