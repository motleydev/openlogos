//! Alignment layers (B067): token-keyed standoff alignments between an original-language
//! edition (SBLGNT, WLC) and a translation (BSB), one SQLite file per pair, read beside the
//! text packs and the morphology layers.
//!
//! Every record is a group of source words and target words, many-to-many, possibly
//! discontinuous, with the curator's own metadata kept as given (Clear's `origin: manual`,
//! `status: created`). A word in no record is *unaligned*, which the answers say; a word the
//! importer could not join to a pack token stays in its record with no token id and its
//! join class, so nothing is flattened or dropped silently.
//!
//! - **Source words** are morphemes of the edition's morphology layer (B026), keyed by that
//!   layer's position; the layer gives each its token, morpheme id, lemma and gloss. A layer
//!   of another revision invalidates the keys, so answers that need it refuse.
//! - **Target words** are the curator's words, each inside one translation token with its
//!   scalar range there (`ground` and `because` are two words of the BSB token
//!   `ground—because`).
//!
//! The manifest records both sides' tokenization hashes (specs/03: SHA-256 of the
//! `<token id>\t<surface>\n` lines of the edition revision in id order) and the text packs'
//! revisions; [`AlignmentLayers::alignment_set`] exports records in the
//! `core::identity::AlignmentSet` form.
//!
//! Translated words aligned to a lemma are evidence of how one translation rendered it, not a
//! definition of the lemma (specs/03); the rendering table says its scope and denominator.

use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;
use std::path::{Path, PathBuf};

use openlogos_core::anchor::TokenId;
use openlogos_core::identity::{AlignmentRecord, AlignmentSet, AlignmentStatus};
use openlogos_core::reference::{BookId, Reference};
use openlogos_retrieval::{Result, RetrievalError, VerseId};
use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};

use crate::morphology::{Morpheme, MorphologyLayers};

/// `alignment_format` in the manifest of every alignment layer; readers refuse others.
pub const ALIGNMENT_FORMAT: &str = "openlogos-alignment-1";

// --- Identity

/// A layer's manifest: what it aligns, under what terms, against which token layers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignmentInfo {
    /// `clear-sblgnt-bsb`.
    pub alignment_id: String,
    /// What a reader is shown (`Clear SBLGNT–BSB alignment (manual)`).
    pub title: String,
    /// The original-language edition (`sblgnt`).
    pub source_edition: String,
    /// The translation (`bsb`).
    pub target_edition: String,
    /// The layer's revision; a rebuild from other bytes is another.
    pub revision: String,
    /// SPDX licence, with the deed.
    pub license: String,
    /// The attribution the licence asks for; a host shows it with every answer.
    pub attribution: String,
    /// Where it came from: repository, commit, file and the manifest hash of checked bytes.
    pub source: String,
    /// `curated:<name>@<version>` for every record (specs/03).
    pub origin: String,
    /// The source text pack's `content_revision`.
    pub source_text_content_revision: String,
    /// SHA-256 of the source text pack's `manifest.json`.
    pub source_text_manifest_sha256: String,
    /// Tokenization hash of the source text pack.
    pub source_tokenization_hash: String,
    /// The morphology layer whose positions key the source words (`macula-greek`).
    pub source_morphology_layer: String,
    /// Its revision.
    pub source_morphology_revision: String,
    /// The target text pack's `content_revision`.
    pub target_text_content_revision: String,
    /// SHA-256 of the target text pack's `manifest.json`.
    pub target_text_manifest_sha256: String,
    /// Tokenization hash of the target text pack.
    pub target_tokenization_hash: String,
}

/// The tokenization hash of specs/03: lower-case hex SHA-256 of the UTF-8 lines
/// `<token id>\t<surface>\n` for every token, in id order. `tokens` must already be in id
/// order (the caller's verse and word order, which is the id order).
#[must_use]
pub fn tokenization_hash<'a>(tokens: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for (id, surface) in tokens {
        hasher.update(id.as_bytes());
        hasher.update(b"\t");
        hasher.update(surface.as_bytes());
        hasher.update(b"\n");
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// --- Writing

/// One source word (a morpheme of the morphology layer) as the importer hands it over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceWordRow {
    /// Its position in the morphology layer.
    pub pos: u32,
    /// Its verse.
    pub verse: VerseId,
    /// The text pack word number of its token; 0 when it has none (a qere read, not written).
    pub word: u32,
    /// Index among the morphemes of that token.
    pub morpheme: u8,
    /// How many morphemes the token has.
    pub morphemes: u8,
    /// The curator's id for it (MACULA `xml:id`).
    pub external_id: String,
    /// The curator's letters for it.
    pub text: String,
    /// How it was joined (`exact`, `accents_omitted`, …).
    pub join: String,
}

/// One target word (the curator's word) as the importer hands it over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetWordRow {
    /// Its verse in the translation's pack; the curator's verse when not joined.
    pub verse: VerseId,
    /// The text pack word number of the token holding it; 0 when not joined.
    pub word: u32,
    /// Scalar range inside that token.
    pub span: Option<(u32, u32)>,
    /// The curator's id for it (`40001001002`).
    pub external_id: String,
    /// Its letters as the curator has them.
    pub text: String,
    /// How it was joined (`exact`, `punctuation`, `shares_token`, `text_differs`, …).
    pub join: String,
}

/// One alignment record: indexes into the rows added, plus the curator's metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRow {
    /// The curator's record id (`40001001.001`).
    pub external_id: String,
    /// Handles from [`AlignmentWriter::add_source`].
    pub sources: Vec<u32>,
    /// Handles from [`AlignmentWriter::add_target`].
    pub targets: Vec<u32>,
    /// The curator's metadata as given (`origin` = `manual`, `status` = `created`).
    pub meta: BTreeMap<String, String>,
}

/// Which side of an alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// The original-language edition.
    Source,
    /// The translation.
    Target,
}

impl Side {
    fn code(self) -> i64 {
        match self {
            Side::Source => 0,
            Side::Target => 1,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
const SCHEMA: &str = "
CREATE TABLE manifest(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE string(id INTEGER PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE source_word(id INTEGER PRIMARY KEY, pos INTEGER NOT NULL, verse_id INTEGER NOT NULL,
    word INTEGER NOT NULL, morpheme INTEGER NOT NULL, morphemes INTEGER NOT NULL,
    external_id TEXT NOT NULL, text TEXT NOT NULL, join_class INTEGER NOT NULL);
CREATE TABLE target_word(id INTEGER PRIMARY KEY, verse_id INTEGER NOT NULL, word INTEGER NOT NULL,
    span_start INTEGER, span_end INTEGER, external_id TEXT NOT NULL, text INTEGER NOT NULL,
    join_class INTEGER NOT NULL);
CREATE TABLE record(id INTEGER PRIMARY KEY, external_id TEXT NOT NULL, meta INTEGER NOT NULL,
    sources BLOB NOT NULL, targets BLOB NOT NULL);
CREATE TABLE absence(side INTEGER NOT NULL, verse_id INTEGER NOT NULL, word INTEGER NOT NULL,
    pos INTEGER, reason TEXT NOT NULL);
";

/// Writes one alignment layer. Source words may arrive in any order (the writer sorts them by
/// layer position); target words must arrive in the curator's reading order, which they
/// keep, so adjacent ids are adjacent words.
#[cfg(not(target_arch = "wasm32"))]
pub struct AlignmentWriter {
    path: PathBuf,
    info: AlignmentInfo,
    sources: Vec<SourceWordRow>,
    targets: Vec<TargetWordRow>,
    records: Vec<RecordRow>,
    absent: Vec<(Side, VerseId, u32, Option<u32>, String)>,
}

#[cfg(not(target_arch = "wasm32"))]
fn u32_blob(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[cfg(not(target_arch = "wasm32"))]
impl AlignmentWriter {
    /// Starts a layer at `path`, which must not exist.
    pub fn create(path: &Path, info: &AlignmentInfo) -> Result<AlignmentWriter> {
        if path.exists() {
            return Err(RetrievalError::Storage(format!(
                "{} exists; an alignment layer is written once",
                path.display()
            )));
        }
        Ok(AlignmentWriter {
            path: path.to_path_buf(),
            info: info.clone(),
            sources: Vec::new(),
            targets: Vec::new(),
            records: Vec::new(),
            absent: Vec::new(),
        })
    }

    /// Adds a source word; returns its handle for records.
    pub fn add_source(&mut self, row: SourceWordRow) -> u32 {
        self.sources.push(row);
        u32::try_from(self.sources.len() - 1).unwrap_or(u32::MAX)
    }

    /// Adds a target word; returns its handle for records.
    pub fn add_target(&mut self, row: TargetWordRow) -> u32 {
        self.targets.push(row);
        u32::try_from(self.targets.len() - 1).unwrap_or(u32::MAX)
    }

    /// Adds a record over handles already returned.
    pub fn add_record(&mut self, row: RecordRow) -> Result<()> {
        if row.sources.is_empty() && row.targets.is_empty() {
            return Err(RetrievalError::InvalidQuery(format!(
                "record {} has no words",
                row.external_id
            )));
        }
        if row
            .sources
            .iter()
            .any(|s| *s as usize >= self.sources.len())
            || row
                .targets
                .iter()
                .any(|t| *t as usize >= self.targets.len())
        {
            return Err(RetrievalError::InvalidQuery(format!(
                "record {} names a word not added",
                row.external_id
            )));
        }
        self.records.push(row);
        Ok(())
    }

    /// Records a token (source: a layer position, when it has one) with no word in the
    /// alignment, and why.
    pub fn absent(
        &mut self,
        side: Side,
        verse: VerseId,
        word: u32,
        pos: Option<u32>,
        reason: &str,
    ) {
        self.absent
            .push((side, verse, word, pos, reason.to_owned()));
    }

    /// Writes the file.
    pub fn finish(self) -> Result<()> {
        let AlignmentWriter {
            path,
            info,
            sources,
            targets,
            records,
            absent,
        } = self;
        let mut source_order: Vec<u32> = (0..sources.len() as u32).collect();
        source_order.sort_by_key(|i| (sources[*i as usize].pos, *i));
        // Target words keep the order they were added in: the curator's reading order, which
        // a word with no token keeps too.
        let target_order: Vec<u32> = (0..targets.len() as u32).collect();
        let mut source_new = vec![0u32; sources.len()];
        for (new, old) in source_order.iter().enumerate() {
            source_new[*old as usize] = new as u32;
        }
        let mut target_new = vec![0u32; targets.len()];
        for (new, old) in target_order.iter().enumerate() {
            target_new[*old as usize] = new as u32;
        }
        let mut conn = Connection::open(&path).map_err(storage)?;
        let tx = conn.transaction().map_err(storage)?;
        tx.execute_batch(SCHEMA).map_err(storage)?;
        let mut strings: HashMap<String, i64> = HashMap::new();
        let mut id_of = |text: &str| -> i64 {
            let next = strings.len() as i64 + 1;
            *strings.entry(text.to_owned()).or_insert(next)
        };
        {
            let mut insert = tx
                .prepare(
                    "INSERT INTO source_word(id, pos, verse_id, word, morpheme, morphemes,
                     external_id, text, join_class) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                )
                .map_err(storage)?;
            for (new, old) in source_order.iter().enumerate() {
                let s = &sources[*old as usize];
                insert
                    .execute(params![
                        new as i64,
                        i64::from(s.pos),
                        i64::from(s.verse.0),
                        i64::from(s.word),
                        i64::from(s.morpheme),
                        i64::from(s.morphemes),
                        s.external_id,
                        s.text,
                        id_of(&s.join)
                    ])
                    .map_err(storage)?;
            }
            let mut insert = tx
                .prepare(
                    "INSERT INTO target_word(id, verse_id, word, span_start, span_end,
                     external_id, text, join_class) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )
                .map_err(storage)?;
            for (new, old) in target_order.iter().enumerate() {
                let t = &targets[*old as usize];
                insert
                    .execute(params![
                        new as i64,
                        i64::from(t.verse.0),
                        i64::from(t.word),
                        t.span.map(|s| i64::from(s.0)),
                        t.span.map(|s| i64::from(s.1)),
                        t.external_id,
                        id_of(&t.text),
                        id_of(&t.join)
                    ])
                    .map_err(storage)?;
            }
            let mut insert = tx
                .prepare(
                    "INSERT INTO record(id, external_id, meta, sources, targets)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .map_err(storage)?;
            for (id, r) in records.iter().enumerate() {
                let mut s: Vec<u32> = r.sources.iter().map(|h| source_new[*h as usize]).collect();
                s.sort_unstable();
                s.dedup();
                let mut t: Vec<u32> = r.targets.iter().map(|h| target_new[*h as usize]).collect();
                t.sort_unstable();
                t.dedup();
                let meta = serde_json::to_string(&r.meta)
                    .map_err(|e| RetrievalError::Storage(e.to_string()))?;
                insert
                    .execute(params![
                        id as i64,
                        r.external_id,
                        id_of(&meta),
                        u32_blob(&s),
                        u32_blob(&t)
                    ])
                    .map_err(storage)?;
            }
            let mut insert = tx
                .prepare(
                    "INSERT INTO absence(side, verse_id, word, pos, reason) VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .map_err(storage)?;
            for (side, verse, word, pos, reason) in &absent {
                insert
                    .execute(params![
                        side.code(),
                        i64::from(verse.0),
                        i64::from(*word),
                        pos.map(i64::from),
                        reason
                    ])
                    .map_err(storage)?;
            }
            let mut insert = tx
                .prepare("INSERT INTO string(id, value) VALUES (?1, ?2)")
                .map_err(storage)?;
            for (value, id) in &strings {
                insert.execute(params![id, value]).map_err(storage)?;
            }
            let mut insert = tx
                .prepare("INSERT INTO manifest(key, value) VALUES (?1, ?2)")
                .map_err(storage)?;
            for (key, value) in manifest_pairs(&info) {
                insert.execute(params![key, value]).map_err(storage)?;
            }
        }
        tx.execute_batch("CREATE INDEX absence_place ON absence(side, verse_id, word);")
            .map_err(storage)?;
        tx.commit().map_err(storage)?;
        conn.execute_batch("VACUUM").map_err(storage)?;
        conn.close().map_err(|(_, e)| storage(e))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn manifest_pairs(info: &AlignmentInfo) -> Vec<(&'static str, String)> {
    vec![
        ("alignment_format", ALIGNMENT_FORMAT.to_owned()),
        ("alignment_id", info.alignment_id.clone()),
        ("title", info.title.clone()),
        ("source_edition", info.source_edition.clone()),
        ("target_edition", info.target_edition.clone()),
        ("revision", info.revision.clone()),
        ("license", info.license.clone()),
        ("attribution", info.attribution.clone()),
        ("source", info.source.clone()),
        ("origin", info.origin.clone()),
        (
            "source_text_content_revision",
            info.source_text_content_revision.clone(),
        ),
        (
            "source_text_manifest_sha256",
            info.source_text_manifest_sha256.clone(),
        ),
        (
            "source_tokenization_hash",
            info.source_tokenization_hash.clone(),
        ),
        (
            "source_morphology_layer",
            info.source_morphology_layer.clone(),
        ),
        (
            "source_morphology_revision",
            info.source_morphology_revision.clone(),
        ),
        (
            "target_text_content_revision",
            info.target_text_content_revision.clone(),
        ),
        (
            "target_text_manifest_sha256",
            info.target_text_manifest_sha256.clone(),
        ),
        (
            "target_tokenization_hash",
            info.target_tokenization_hash.clone(),
        ),
    ]
}

// --- Answers

/// Whether a token has aligned words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenStatus {
    /// At least one of its words is in a record.
    Aligned,
    /// It has words in the alignment, none in a record: the curators left it unaligned.
    Unaligned,
    /// The alignment has no word for it: see the reason.
    NotJoined,
    /// No alignment layer covers its edition.
    NoLayer,
}

/// One word of the alignment inside a token, with whether it is aligned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WordInToken {
    /// The curator's id.
    pub external_id: String,
    /// The curator's letters.
    pub text: String,
    /// Scalar range inside the token (target words; source morphemes take theirs from the
    /// morphology layer).
    pub start: Option<u32>,
    /// End of it, exclusive.
    pub end: Option<u32>,
    /// The morpheme id, for a source word (`wlc:01001001001b`).
    pub morpheme_id: Option<String>,
    /// How it was joined.
    pub join: String,
    /// `aligned` or `unaligned`.
    pub status: TokenStatus,
    /// Ids of the records it is in, in record order.
    pub records: Vec<String>,
}

/// A token asked about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenAlignment {
    /// The token (a morpheme suffix is kept when one was asked for).
    pub token_id: String,
    /// Its edition.
    pub edition: String,
    /// Whether it is aligned.
    pub status: TokenStatus,
    /// Why not, when [`TokenStatus::NotJoined`]: `not_in_clear_target`,
    /// `verse_not_in_clear_target`, `not_in_alignment_source`, `unknown_token`.
    pub reason: Option<String>,
    /// The alignment's words inside it.
    pub words: Vec<WordInToken>,
}

/// A source word of a record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignedSourceWord {
    /// The text pack token; `None` when the word has none.
    pub token_id: Option<String>,
    /// The morpheme id (`wlc:01001001001b`), or the token id for a one-morpheme token.
    pub morpheme_id: Option<String>,
    /// The curator's id (MACULA `xml:id`).
    pub external_id: String,
    /// The curator's letters.
    pub text: String,
    /// How it was joined.
    pub join: String,
    /// The morphology layer's analysis of this morpheme (lemma, gloss, parsing); `None` when
    /// no matching layer is installed.
    pub morphology: Option<Morpheme>,
}

/// A target word of a record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignedTargetWord {
    /// The text pack token holding it; `None` when it was not joined.
    pub token_id: Option<String>,
    /// Scalar range inside the token.
    pub start: Option<u32>,
    /// End of it, exclusive.
    pub end: Option<u32>,
    /// The curator's id.
    pub external_id: String,
    /// The curator's letters.
    pub text: String,
    /// How it was joined.
    pub join: String,
}

/// One alignment record with its words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignmentGroup {
    /// The layer.
    pub alignment_id: String,
    /// The curator's record id (`40001001.001`).
    pub record_id: String,
    /// `curated:<name>@<version>`.
    pub origin: String,
    /// The curator's metadata as given (`origin`: `manual`, `status`: `created`).
    pub meta: BTreeMap<String, String>,
    /// Source words, in reading order.
    pub sources: Vec<AlignedSourceWord>,
    /// Target words, in reading order.
    pub targets: Vec<AlignedTargetWord>,
    /// Whether the target words are not one run of the translation's words.
    pub discontinuous: bool,
}

/// What the alignment layers say about a selection of tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignmentAnswer {
    /// Per token asked about, in the order asked.
    pub tokens: Vec<TokenAlignment>,
    /// The records any of them is in, once each, in reading order.
    pub groups: Vec<AlignmentGroup>,
    /// What to show with the answer: each layer's attribution (and the morphology layers'
    /// when their analyses are included).
    pub attribution: Vec<String>,
}

/// One rendering of a lemma in the translation, with its count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderingRow {
    /// The record's target words in reading order, `…` between runs; empty for the
    /// unaligned row.
    pub rendering: String,
    /// `aligned`, or `unaligned` for occurrences in no record.
    pub status: TokenStatus,
    /// Occurrences rendered so.
    pub count: u64,
    /// Of those, occurrences whose record also holds other source words (the rendering is
    /// of the group: `ὁ λόγος` → `the Word`).
    pub with_other_source_words: u64,
    /// The first few places (`John.1.1`), in reading order.
    pub examples: Vec<String>,
}

/// How one translation renders one lemma.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderingTable {
    /// The lemma as the source spells it.
    pub lemma: String,
    /// Strong's number of its first occurrence.
    pub strong: String,
    /// The original-language edition (`sblgnt`).
    pub source_edition: String,
    /// The translation (`bsb`).
    pub target_edition: String,
    /// The alignment layer.
    pub alignment_id: String,
    /// The scope counted, in words (`SBLGNT, all books` or `SBLGNT, John`).
    pub scope: String,
    /// The denominator: every occurrence (morpheme) of the lemma in the scope.
    pub occurrences: u64,
    /// Occurrences in some record.
    pub aligned: u64,
    /// Occurrences in the alignment's source text but in no record.
    pub unaligned: u64,
    /// Occurrences the alignment does not cover (not in its source text, such as an article
    /// only a prefix's vowel implies).
    pub not_covered: u64,
    /// Rows, most frequent first; the unaligned row last.
    pub rows: Vec<RenderingRow>,
    /// The layers' attribution, to show with the table.
    pub attribution: Vec<String>,
}

/// Rendering-per-translation counts for a lemma query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Renderings {
    /// What was asked.
    pub query: String,
    /// `exact` or `key` (as lemma search matches).
    pub matched_by: String,
    /// One table per matched lemma and alignment layer, largest first.
    pub tables: Vec<RenderingTable>,
    /// Editions asked for that have no morphology or alignment layer, or whose morphology
    /// layer is not the one the alignment was joined to (with the reason).
    pub unavailable: Vec<String>,
    /// The fixed caution: renderings are evidence of translation choices, not definitions.
    pub note: String,
}

/// The note every rendering answer carries (specs/03, word study).
pub const RENDERING_NOTE: &str = "How one translation rendered each occurrence, from \
     token-keyed alignments; renderings are evidence of translation choices, not definitions \
     of the word.";

// --- Reading

/// Per rendering: occurrences, those with other source words, examples, first occurrence.
type Tally = (u64, u64, Vec<String>, usize);

/// Offsets-and-values lists: the members of row `i` are `values[offsets[i]..offsets[i+1]]`.
#[derive(Debug, Default)]
struct Csr {
    offsets: Vec<u32>,
    values: Vec<u32>,
}

impl Csr {
    fn from_lists(lists: &[Vec<u32>]) -> Csr {
        let mut csr = Csr {
            offsets: Vec::with_capacity(lists.len() + 1),
            values: Vec::new(),
        };
        csr.offsets.push(0);
        for list in lists {
            csr.values.extend_from_slice(list);
            csr.offsets.push(csr.values.len() as u32);
        }
        csr
    }

    fn get(&self, row: usize) -> &[u32] {
        match (self.offsets.get(row), self.offsets.get(row + 1)) {
            (Some(a), Some(b)) => &self.values[*a as usize..*b as usize],
            _ => &[],
        }
    }
}

struct Layer {
    schema: String,
    info: AlignmentInfo,
    strings: Vec<String>,
    // Source words, by id (sorted by layer position).
    source_pos: Vec<u32>,
    source_verse: Vec<u32>,
    source_word: Vec<u16>,
    source_morpheme: Vec<u8>,
    source_morphemes: Vec<u8>,
    source_join: Vec<u32>,
    /// `(verse << 16 | word, id)`, sorted: the source words of a token.
    source_by_token: Vec<(u64, u32)>,
    source_records: Csr,
    // Target words, by id (the curator's reading order).
    target_verse: Vec<u32>,
    target_word: Vec<u16>,
    target_span: Vec<Option<(u32, u32)>>,
    target_text: Vec<u32>,
    target_join: Vec<u32>,
    target_records: Csr,
    /// `(verse << 16 | word, id)`, sorted (stably: reading order inside a token).
    target_by_token: Vec<(u64, u32)>,
    // Records.
    record_meta: Vec<u32>,
    record_sources: Csr,
    record_targets: Csr,
    // Tokens with no word, by side: `(verse << 16 | word)` → reason.
    absent_target: HashMap<u64, String>,
    absent_source: HashMap<u64, String>,
    absent_pos: HashMap<u32, String>,
}

/// The installed alignment layers.
pub struct AlignmentLayers {
    conn: Connection,
    layers: Vec<Layer>,
}

impl std::fmt::Debug for AlignmentLayers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlignmentLayers")
            .field(
                "layers",
                &self
                    .layers
                    .iter()
                    .map(|l| &l.info.alignment_id)
                    .collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

fn storage(error: rusqlite::Error) -> RetrievalError {
    RetrievalError::Storage(error.to_string())
}

fn file_uri(path: &Path) -> String {
    let escaped = path
        .display()
        .to_string()
        .replace('%', "%25")
        .replace('?', "%3F")
        .replace('#', "%23");
    format!("file:{escaped}?immutable=1")
}

fn u32s(blob: &[u8]) -> Vec<u32> {
    blob.as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

fn token_key(verse: u32, word: u32) -> u64 {
    (u64::from(verse) << 16) | u64::from(word)
}

/// `edition:BBCCCVVVWWW`.
fn token_id(edition: &str, verse: VerseId, word: u32) -> String {
    format!(
        "{edition}:{:02}{:03}{:03}{word:03}",
        verse.book(),
        verse.chapter(),
        verse.verse()
    )
}

fn morpheme_id(edition: &str, verse: VerseId, word: u32, morpheme: u8, morphemes: u8) -> String {
    let mut id = token_id(edition, verse, word);
    if morphemes >= 2 {
        id.push(char::from(b'a' + morpheme.min(25)));
    }
    id
}

/// `John.1.1`.
#[must_use]
pub fn osis(verse: VerseId) -> String {
    BookId::from_number(verse.book()).map_or_else(
        || verse.0.to_string(),
        |book| {
            Reference::verse(book, u32::from(verse.chapter()), u32::from(verse.verse())).to_osis()
        },
    )
}

impl AlignmentLayers {
    /// No layers yet.
    pub fn new() -> Result<AlignmentLayers> {
        let conn = Connection::open_in_memory_with_flags(
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(storage)?;
        Ok(AlignmentLayers {
            conn,
            layers: Vec::new(),
        })
    }

    /// Attaches every `*.sqlite` of a directory, in name order.
    pub fn open_directory(directory: &Path) -> Result<AlignmentLayers> {
        let mut layers = AlignmentLayers::new()?;
        let read = std::fs::read_dir(directory)
            .map_err(|e| RetrievalError::Storage(format!("{}: {e}", directory.display())))?;
        let mut paths: Vec<PathBuf> = read
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "sqlite"))
            .collect();
        paths.sort();
        if paths.is_empty() {
            return Err(RetrievalError::Storage(format!(
                "{} holds no alignment layers",
                directory.display()
            )));
        }
        for path in paths {
            layers.attach_file(&path)?;
        }
        Ok(layers)
    }

    fn next_schema(&self) -> String {
        format!("a{}", self.layers.len())
    }

    /// Attaches one layer from a file, read-only. Returns its id.
    pub fn attach_file(&mut self, path: &Path) -> Result<String> {
        let schema = self.next_schema();
        self.conn
            .execute(
                &format!("ATTACH DATABASE ?1 AS \"{schema}\""),
                params![file_uri(path)],
            )
            .map_err(storage)?;
        self.adopt(schema)
    }

    /// Attaches one layer from its bytes (what a browser fetched); they are copied.
    pub fn attach_bytes(&mut self, bytes: &[u8]) -> Result<String> {
        if !bytes.starts_with(b"SQLite format 3\0") {
            return Err(RetrievalError::Storage(
                "these bytes are not an alignment layer".to_owned(),
            ));
        }
        let schema = self.next_schema();
        self.conn
            .execute(&format!("ATTACH DATABASE ':memory:' AS \"{schema}\""), [])
            .map_err(storage)?;
        if let Err(error) =
            self.conn
                .deserialize_read_exact(schema.as_str(), Cursor::new(bytes), bytes.len(), true)
        {
            let _ = self
                .conn
                .execute_batch(&format!("DETACH DATABASE \"{schema}\""));
            return Err(storage(error));
        }
        self.adopt(schema)
    }

    fn adopt(&mut self, schema: String) -> Result<String> {
        match self.read_layer(&schema) {
            Ok(layer) => {
                let id = layer.info.alignment_id.clone();
                self.layers.push(layer);
                Ok(id)
            }
            Err(error) => {
                let _ = self
                    .conn
                    .execute_batch(&format!("DETACH DATABASE \"{schema}\""));
                Err(error)
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn read_layer(&self, schema: &str) -> Result<Layer> {
        let value = |key: &str| -> Result<String> {
            self.conn
                .query_row(
                    &format!("SELECT value FROM \"{schema}\".manifest WHERE key = ?1"),
                    params![key],
                    |r| r.get(0),
                )
                .map_err(|e| RetrievalError::Storage(format!("manifest `{key}`: {e}")))
        };
        let format = value("alignment_format")?;
        if format != ALIGNMENT_FORMAT {
            return Err(RetrievalError::Storage(format!(
                "alignment format {format} is not the supported {ALIGNMENT_FORMAT}"
            )));
        }
        let info = AlignmentInfo {
            alignment_id: value("alignment_id")?,
            title: value("title")?,
            source_edition: value("source_edition")?,
            target_edition: value("target_edition")?,
            revision: value("revision")?,
            license: value("license")?,
            attribution: value("attribution")?,
            source: value("source")?,
            origin: value("origin")?,
            source_text_content_revision: value("source_text_content_revision")?,
            source_text_manifest_sha256: value("source_text_manifest_sha256")?,
            source_tokenization_hash: value("source_tokenization_hash")?,
            source_morphology_layer: value("source_morphology_layer")?,
            source_morphology_revision: value("source_morphology_revision")?,
            target_text_content_revision: value("target_text_content_revision")?,
            target_text_manifest_sha256: value("target_text_manifest_sha256")?,
            target_tokenization_hash: value("target_tokenization_hash")?,
        };
        if self
            .layers
            .iter()
            .any(|l| l.info.alignment_id == info.alignment_id)
        {
            return Err(RetrievalError::Storage(format!(
                "alignment `{}` is already attached",
                info.alignment_id
            )));
        }
        let mut strings: Vec<String> = Vec::new();
        {
            let mut stmt = self
                .conn
                .prepare(&format!(
                    "SELECT id, value FROM \"{schema}\".string ORDER BY id"
                ))
                .map_err(storage)?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                .map_err(storage)?;
            for row in rows {
                let (id, value) = row.map_err(storage)?;
                let id = usize::try_from(id).unwrap_or(0);
                if strings.len() <= id {
                    strings.resize(id + 1, String::new());
                }
                strings[id] = value;
            }
        }
        let mut layer = Layer {
            schema: schema.to_owned(),
            info,
            strings,
            source_pos: Vec::new(),
            source_verse: Vec::new(),
            source_word: Vec::new(),
            source_morpheme: Vec::new(),
            source_morphemes: Vec::new(),
            source_join: Vec::new(),
            source_by_token: Vec::new(),
            source_records: Csr::default(),
            target_verse: Vec::new(),
            target_word: Vec::new(),
            target_span: Vec::new(),
            target_text: Vec::new(),
            target_join: Vec::new(),
            target_records: Csr::default(),
            target_by_token: Vec::new(),
            record_meta: Vec::new(),
            record_sources: Csr::default(),
            record_targets: Csr::default(),
            absent_target: HashMap::new(),
            absent_source: HashMap::new(),
            absent_pos: HashMap::new(),
        };
        {
            let mut stmt = self
                .conn
                .prepare(&format!(
                    "SELECT pos, verse_id, word, morpheme, morphemes, join_class
                     FROM \"{schema}\".source_word ORDER BY id"
                ))
                .map_err(storage)?;
            let mut rows = stmt.query([]).map_err(storage)?;
            while let Some(r) = rows.next().map_err(storage)? {
                let get = |i: usize| -> Result<i64> { r.get::<_, i64>(i).map_err(storage) };
                layer.source_pos.push(get(0)? as u32);
                layer.source_verse.push(get(1)? as u32);
                layer.source_word.push(get(2)? as u16);
                layer.source_morpheme.push(get(3)? as u8);
                layer.source_morphemes.push(get(4)? as u8);
                layer.source_join.push(get(5)? as u32);
            }
        }
        {
            let mut stmt = self
                .conn
                .prepare(&format!(
                    "SELECT verse_id, word, span_start, span_end, text, join_class
                     FROM \"{schema}\".target_word ORDER BY id"
                ))
                .map_err(storage)?;
            let mut rows = stmt.query([]).map_err(storage)?;
            while let Some(r) = rows.next().map_err(storage)? {
                let get = |i: usize| -> Result<i64> { r.get::<_, i64>(i).map_err(storage) };
                layer.target_verse.push(get(0)? as u32);
                layer.target_word.push(get(1)? as u16);
                let start: Option<i64> = r.get(2).map_err(storage)?;
                let end: Option<i64> = r.get(3).map_err(storage)?;
                layer
                    .target_span
                    .push(start.zip(end).map(|(s, e)| (s as u32, e as u32)));
                layer.target_text.push(get(4)? as u32);
                layer.target_join.push(get(5)? as u32);
            }
        }
        let mut source_lists: Vec<Vec<u32>> = vec![Vec::new(); layer.source_pos.len()];
        let mut target_lists: Vec<Vec<u32>> = vec![Vec::new(); layer.target_verse.len()];
        let mut record_sources: Vec<Vec<u32>> = Vec::new();
        let mut record_targets: Vec<Vec<u32>> = Vec::new();
        {
            let mut stmt = self
                .conn
                .prepare(&format!(
                    "SELECT meta, sources, targets FROM \"{schema}\".record ORDER BY id"
                ))
                .map_err(storage)?;
            let mut rows = stmt.query([]).map_err(storage)?;
            while let Some(r) = rows.next().map_err(storage)? {
                let record = layer.record_meta.len() as u32;
                layer
                    .record_meta
                    .push(r.get::<_, i64>(0).map_err(storage)? as u32);
                let sources = u32s(&r.get::<_, Vec<u8>>(1).map_err(storage)?);
                let targets = u32s(&r.get::<_, Vec<u8>>(2).map_err(storage)?);
                for s in &sources {
                    let list = source_lists.get_mut(*s as usize).ok_or_else(|| {
                        RetrievalError::Storage(format!("record {record} names source {s}"))
                    })?;
                    list.push(record);
                }
                for t in &targets {
                    let list = target_lists.get_mut(*t as usize).ok_or_else(|| {
                        RetrievalError::Storage(format!("record {record} names target {t}"))
                    })?;
                    list.push(record);
                }
                record_sources.push(sources);
                record_targets.push(targets);
            }
        }
        layer.source_records = Csr::from_lists(&source_lists);
        layer.target_records = Csr::from_lists(&target_lists);
        layer.record_sources = Csr::from_lists(&record_sources);
        layer.record_targets = Csr::from_lists(&record_targets);
        layer.source_by_token = (0..layer.source_pos.len())
            .filter(|i| layer.source_word[*i] != 0)
            .map(|i| {
                (
                    token_key(layer.source_verse[i], u32::from(layer.source_word[i])),
                    i as u32,
                )
            })
            .collect();
        layer.source_by_token.sort_unstable();
        layer.target_by_token = (0..layer.target_verse.len())
            .filter(|i| layer.target_word[*i] != 0)
            .map(|i| {
                (
                    token_key(layer.target_verse[i], u32::from(layer.target_word[i])),
                    i as u32,
                )
            })
            .collect();
        layer.target_by_token.sort_by_key(|(k, _)| *k);
        {
            let mut stmt = self
                .conn
                .prepare(&format!(
                    "SELECT side, verse_id, word, pos, reason FROM \"{schema}\".absence"
                ))
                .map_err(storage)?;
            let mut rows = stmt.query([]).map_err(storage)?;
            while let Some(r) = rows.next().map_err(storage)? {
                let side: i64 = r.get(0).map_err(storage)?;
                let verse: i64 = r.get(1).map_err(storage)?;
                let word: i64 = r.get(2).map_err(storage)?;
                let pos: Option<i64> = r.get(3).map_err(storage)?;
                let reason: String = r.get(4).map_err(storage)?;
                let key = token_key(verse as u32, word as u32);
                if side == Side::Target.code() {
                    layer.absent_target.insert(key, reason);
                } else if let Some(pos) = pos {
                    layer.absent_pos.insert(pos as u32, reason.clone());
                    layer.absent_source.entry(key).or_insert(reason);
                } else {
                    layer.absent_source.insert(key, reason);
                }
            }
        }
        Ok(layer)
    }

    /// The attached layers' manifests, in attach order.
    #[must_use]
    pub fn layers(&self) -> Vec<AlignmentInfo> {
        self.layers.iter().map(|l| l.info.clone()).collect()
    }

    /// The editions some layer covers (either side), sorted, once each.
    #[must_use]
    pub fn editions(&self) -> Vec<String> {
        let mut editions: Vec<String> = self
            .layers
            .iter()
            .flat_map(|l| [l.info.source_edition.clone(), l.info.target_edition.clone()])
            .collect();
        editions.sort();
        editions.dedup();
        editions
    }

    /// Where the installed morphology layers do not match what an alignment was joined to:
    /// `(alignment id, reason)`. Source words are keyed by morphology layer positions, so a
    /// mismatch makes lemma and morphology answers through that alignment unavailable.
    #[must_use]
    pub fn morphology_mismatches(&self, morphology: &MorphologyLayers) -> Vec<(String, String)> {
        let installed = morphology.layers();
        self.layers
            .iter()
            .filter_map(|l| {
                let found = installed.iter().find(|m| {
                    m.edition == l.info.source_edition
                        && m.layer_id == l.info.source_morphology_layer
                });
                let reason = match found {
                    None => format!(
                        "no morphology layer `{}` over {} is installed",
                        l.info.source_morphology_layer, l.info.source_edition
                    ),
                    Some(m) if m.revision != l.info.source_morphology_revision => format!(
                        "the installed morphology layer is {}; {} was joined to {}",
                        m.revision, l.info.alignment_id, l.info.source_morphology_revision
                    ),
                    Some(_) => return None,
                };
                Some((l.info.alignment_id.clone(), reason))
            })
            .collect()
    }

    fn string(layer: &Layer, id: u32) -> &str {
        layer.strings.get(id as usize).map_or("", String::as_str)
    }

    fn source_external(&self, layer: &Layer, id: u32) -> Result<(String, String)> {
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "SELECT external_id, text FROM \"{}\".source_word WHERE id = ?1",
                layer.schema
            ))
            .map_err(storage)?;
        stmt.query_row(params![i64::from(id)], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(storage)
    }

    fn target_external(&self, layer: &Layer, id: u32) -> Result<String> {
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "SELECT external_id FROM \"{}\".target_word WHERE id = ?1",
                layer.schema
            ))
            .map_err(storage)?;
        stmt.query_row(params![i64::from(id)], |r| r.get(0))
            .map_err(storage)
    }

    fn record_external(&self, layer: &Layer, id: u32) -> Result<String> {
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "SELECT external_id FROM \"{}\".record WHERE id = ?1",
                layer.schema
            ))
            .map_err(storage)?;
        stmt.query_row(params![i64::from(id)], |r| r.get(0))
            .map_err(storage)
    }

    /// Target words of a token, by id, in reading order.
    fn targets_of(layer: &Layer, verse: u32, word: u32) -> Vec<u32> {
        let key = token_key(verse, word);
        let lo = layer.target_by_token.partition_point(|(k, _)| *k < key);
        layer.target_by_token[lo..]
            .iter()
            .take_while(|(k, _)| *k == key)
            .map(|(_, id)| *id)
            .collect()
    }

    /// Source words of a token, by id.
    fn sources_of(layer: &Layer, verse: u32, word: u32) -> Vec<u32> {
        let key = token_key(verse, word);
        let lo = layer.source_by_token.partition_point(|(k, _)| *k < key);
        layer.source_by_token[lo..]
            .iter()
            .take_while(|(k, _)| *k == key)
            .map(|(_, id)| *id)
            .collect()
    }

    fn source_word(
        &self,
        layer: &Layer,
        id: u32,
        morphology: Option<&MorphologyLayers>,
        analyses: &mut HashMap<String, Vec<Morpheme>>,
    ) -> Result<AlignedSourceWord> {
        let i = id as usize;
        let edition = &layer.info.source_edition;
        let verse = VerseId(layer.source_verse[i]);
        let word = u32::from(layer.source_word[i]);
        let (external_id, text) = self.source_external(layer, id)?;
        let token = (word != 0).then(|| token_id(edition, verse, word));
        let morpheme = (word != 0).then(|| {
            morpheme_id(
                edition,
                verse,
                word,
                layer.source_morpheme[i],
                layer.source_morphemes[i],
            )
        });
        let analysis = match (morphology, &token) {
            (Some(m), Some(t)) => {
                if !analyses.contains_key(t) {
                    let found = m
                        .analyze(t)?
                        .analyses
                        .into_iter()
                        .find(|a| a.layer_id == layer.info.source_morphology_layer)
                        .map(|a| a.morphemes)
                        .unwrap_or_default();
                    analyses.insert(t.clone(), found);
                }
                analyses[t]
                    .iter()
                    .find(|m| m.external_id == external_id)
                    .cloned()
            }
            _ => None,
        };
        Ok(AlignedSourceWord {
            token_id: token,
            morpheme_id: morpheme,
            external_id,
            text,
            join: Self::string(layer, layer.source_join[i]).to_owned(),
            morphology: analysis,
        })
    }

    fn target_word(&self, layer: &Layer, id: u32) -> Result<AlignedTargetWord> {
        let i = id as usize;
        let word = u32::from(layer.target_word[i]);
        let span = layer.target_span[i];
        Ok(AlignedTargetWord {
            token_id: (word != 0).then(|| {
                token_id(
                    &layer.info.target_edition,
                    VerseId(layer.target_verse[i]),
                    word,
                )
            }),
            start: span.map(|s| s.0),
            end: span.map(|s| s.1),
            external_id: self.target_external(layer, id)?,
            text: Self::string(layer, layer.target_text[i]).to_owned(),
            join: Self::string(layer, layer.target_join[i]).to_owned(),
        })
    }

    fn group(
        &self,
        layer: &Layer,
        record: u32,
        morphology: Option<&MorphologyLayers>,
        analyses: &mut HashMap<String, Vec<Morpheme>>,
    ) -> Result<AlignmentGroup> {
        let r = record as usize;
        let meta: BTreeMap<String, String> =
            serde_json::from_str(Self::string(layer, layer.record_meta[r])).unwrap_or_default();
        let sources = layer
            .record_sources
            .get(r)
            .iter()
            .map(|s| self.source_word(layer, *s, morphology, analyses))
            .collect::<Result<Vec<_>>>()?;
        let target_ids = layer.record_targets.get(r);
        let targets = target_ids
            .iter()
            .map(|t| self.target_word(layer, *t))
            .collect::<Result<Vec<_>>>()?;
        Ok(AlignmentGroup {
            alignment_id: layer.info.alignment_id.clone(),
            record_id: self.record_external(layer, record)?,
            origin: layer.info.origin.clone(),
            meta,
            sources,
            targets,
            discontinuous: Self::discontinuous(target_ids),
        })
    }

    /// Whether a record's target words (ids in reading order) leave a word of the
    /// alignment's target text out between them. Words of one token count as adjacent.
    fn discontinuous(targets: &[u32]) -> bool {
        targets.windows(2).any(|w| w[1] != w[0] + 1)
    }

    /// What the layers say about a selection of tokens (`bsb:43001001004`, or
    /// `sblgnt:43001001001`, `wlc:01001001001b`): per token, its words and whether they are
    /// aligned; and every record they are in, with both sides' words and, when `morphology`
    /// is given and is the layer the alignment was joined to, each source word's analysis.
    pub fn align(
        &self,
        tokens: &[String],
        morphology: Option<&MorphologyLayers>,
    ) -> Result<AlignmentAnswer> {
        let mismatched: Vec<String> = morphology
            .map(|m| self.morphology_mismatches(m))
            .unwrap_or_default()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        let mut answer = AlignmentAnswer {
            tokens: Vec::new(),
            groups: Vec::new(),
            attribution: Vec::new(),
        };
        let mut wanted: Vec<(usize, u32)> = Vec::new();
        for token in tokens {
            let id = TokenId::parse(token)
                .map_err(|e| RetrievalError::InvalidQuery(format!("token id `{token}`: {e}")))?;
            let verse = VerseId::new(
                id.book.number(),
                u16::try_from(id.chapter).unwrap_or(0),
                u16::try_from(id.verse).unwrap_or(0),
            );
            let mut result = TokenAlignment {
                token_id: token.clone(),
                edition: id.edition.clone(),
                status: TokenStatus::NoLayer,
                reason: None,
                words: Vec::new(),
            };
            let mut covered = false;
            for (index, layer) in self.layers.iter().enumerate() {
                let side = if layer.info.target_edition == id.edition {
                    Side::Target
                } else if layer.info.source_edition == id.edition {
                    Side::Source
                } else {
                    continue;
                };
                let mut words = Vec::new();
                match side {
                    Side::Target => {
                        for t in Self::targets_of(layer, verse.0, id.word) {
                            let t = t as usize;
                            let records = layer.target_records.get(t).to_vec();
                            let span = layer.target_span[t];
                            words.push((
                                self.target_external(layer, t as u32)?,
                                Self::string(layer, layer.target_text[t]).to_owned(),
                                span,
                                None,
                                Self::string(layer, layer.target_join[t]).to_owned(),
                                records,
                            ));
                        }
                    }
                    Side::Source => {
                        for s in Self::sources_of(layer, verse.0, id.word) {
                            let i = s as usize;
                            let morpheme = morpheme_id(
                                &id.edition,
                                verse,
                                id.word,
                                layer.source_morpheme[i],
                                layer.source_morphemes[i],
                            );
                            if let Some(suffix) = &id.suffix
                                && !morpheme.ends_with(suffix.as_str())
                            {
                                continue;
                            }
                            let (external, text) = self.source_external(layer, s)?;
                            words.push((
                                external,
                                text,
                                None,
                                Some(morpheme),
                                Self::string(layer, layer.source_join[i]).to_owned(),
                                layer.source_records.get(i).to_vec(),
                            ));
                        }
                    }
                }
                covered = true;
                if words.is_empty() {
                    // A layer with no word and no absence for the token does not cover it
                    // (the NT layer and a BSB token of Genesis).
                    let key = token_key(verse.0, id.word);
                    let reason = match side {
                        Side::Target => layer.absent_target.get(&key),
                        Side::Source => layer.absent_source.get(&key),
                    };
                    if let Some(reason) = reason
                        && matches!(result.status, TokenStatus::NoLayer | TokenStatus::NotJoined)
                    {
                        result.status = TokenStatus::NotJoined;
                        result.reason = Some(reason.clone());
                    }
                    continue;
                }
                let any_aligned = words.iter().any(|w| !w.5.is_empty());
                result.status = match (result.status, any_aligned) {
                    (_, true) | (TokenStatus::Aligned, _) => TokenStatus::Aligned,
                    _ => TokenStatus::Unaligned,
                };
                result.reason = None;
                if !answer.attribution.contains(&layer.info.attribution) {
                    answer.attribution.push(layer.info.attribution.clone());
                }
                for (external_id, text, span, morpheme_id, join, records) in words {
                    for r in &records {
                        if !wanted.contains(&(index, *r)) {
                            wanted.push((index, *r));
                        }
                    }
                    result.words.push(WordInToken {
                        external_id,
                        text,
                        start: span.map(|s| s.0),
                        end: span.map(|s| s.1),
                        morpheme_id,
                        join,
                        status: if records.is_empty() {
                            TokenStatus::Unaligned
                        } else {
                            TokenStatus::Aligned
                        },
                        records: records
                            .iter()
                            .map(|r| self.record_external(layer, *r))
                            .collect::<Result<Vec<_>>>()?,
                    });
                }
            }
            if covered && result.status == TokenStatus::NoLayer {
                result.status = TokenStatus::NotJoined;
                result.reason = Some("unknown_token".to_owned());
            }
            answer.tokens.push(result);
        }
        wanted.sort_unstable();
        let mut analyses = HashMap::new();
        let mut morphology_used: Vec<String> = Vec::new();
        for (index, record) in wanted {
            let layer = &self.layers[index];
            let usable = morphology.filter(|_| !mismatched.contains(&layer.info.alignment_id));
            let group = self.group(layer, record, usable, &mut analyses)?;
            if group.sources.iter().any(|s| s.morphology.is_some())
                && !morphology_used.contains(&layer.info.source_edition)
            {
                morphology_used.push(layer.info.source_edition.clone());
            }
            answer.groups.push(group);
        }
        if let Some(m) = morphology {
            for info in m.layers() {
                if morphology_used.contains(&info.edition)
                    && !answer.attribution.contains(&info.attribution)
                {
                    answer.attribution.push(info.attribution);
                }
            }
        }
        Ok(answer)
    }

    /// How each translation with an alignment renders a lemma: every occurrence of the
    /// lemmas the query matches (as lemma search matches: `λόγος` exactly, `λογος` by key)
    /// in the source editions asked for (all with an alignment when empty), optionally in one
    /// book (`John`), grouped by the target words of its record. The table states its scope
    /// and denominator; occurrences in no record and occurrences the alignment does not cover
    /// are counted, not hidden.
    pub fn renderings(
        &self,
        morphology: &MorphologyLayers,
        editions: &[String],
        query: &str,
        book: Option<&str>,
    ) -> Result<Renderings> {
        let book = match book.map(str::trim).filter(|b| !b.is_empty()) {
            Some(b) => Some(BookId::from_osis(b).ok_or_else(|| {
                RetrievalError::InvalidQuery(format!("`{b}` is not an OSIS book"))
            })?),
            None => None,
        };
        let mismatches = self.morphology_mismatches(morphology);
        let layers: Vec<&Layer> = self
            .layers
            .iter()
            .filter(|l| editions.is_empty() || editions.contains(&l.info.source_edition))
            .collect();
        let mut unavailable: Vec<String> = editions
            .iter()
            .filter(|e| !layers.iter().any(|l| &l.info.source_edition == *e))
            .map(|e| format!("{e}: no alignment layer"))
            .collect();
        let usable: Vec<&Layer> = layers
            .into_iter()
            .filter(
                |l| match mismatches.iter().find(|(id, _)| *id == l.info.alignment_id) {
                    Some((_, reason)) => {
                        unavailable.push(format!("{}: {reason}", l.info.source_edition));
                        false
                    }
                    None => true,
                },
            )
            .collect();
        let source_editions: Vec<String> = usable
            .iter()
            .map(|l| l.info.source_edition.clone())
            .collect();
        let found = if source_editions.is_empty() {
            None
        } else {
            Some(morphology.lemma_positions(&source_editions, query)?)
        };
        let mut result = Renderings {
            query: query.trim().to_owned(),
            matched_by: found
                .as_ref()
                .map_or_else(|| "key".to_owned(), |f| f.matched_by.clone()),
            tables: Vec::new(),
            unavailable,
            note: RENDERING_NOTE.to_owned(),
        };
        let Some(found) = found else {
            return Ok(result);
        };
        for group in &found.lemmas {
            for layer in usable
                .iter()
                .filter(|l| l.info.source_edition == group.edition)
            {
                result
                    .tables
                    .push(self.rendering_table(layer, morphology, group, book)?);
            }
        }
        result.tables.sort_by(|a, b| {
            b.occurrences
                .cmp(&a.occurrences)
                .then(a.lemma.cmp(&b.lemma))
        });
        Ok(result)
    }

    fn rendering_table(
        &self,
        layer: &Layer,
        morphology: &MorphologyLayers,
        group: &crate::morphology::LemmaGroup,
        book: Option<BookId>,
    ) -> Result<RenderingTable> {
        const EXAMPLES: usize = 3;
        let edition_title = layer.info.source_edition.to_uppercase();
        let mut table = RenderingTable {
            lemma: group.lemma.clone(),
            strong: group.strong.clone(),
            source_edition: layer.info.source_edition.clone(),
            target_edition: layer.info.target_edition.clone(),
            alignment_id: layer.info.alignment_id.clone(),
            scope: match book {
                Some(b) => format!("{edition_title}, {}", b.title()),
                None => format!("{edition_title}, all books"),
            },
            occurrences: 0,
            aligned: 0,
            unaligned: 0,
            not_covered: 0,
            rows: Vec::new(),
            attribution: vec![layer.info.attribution.clone()],
        };
        if let Some(m) = morphology
            .layers()
            .into_iter()
            .find(|m| m.edition == layer.info.source_edition)
        {
            table.attribution.push(m.attribution);
        }
        let mut rows: HashMap<String, Tally> = HashMap::new();
        let mut unaligned_examples: Vec<String> = Vec::new();
        for (order, pos) in group.positions.iter().enumerate() {
            if let Some(b) = book {
                let Some((verse, _)) = morphology.place_of(&group.edition, *pos) else {
                    continue;
                };
                if verse.book() != b.number() {
                    continue;
                }
            }
            table.occurrences += 1;
            let Ok(id) = layer.source_pos.binary_search(pos) else {
                table.not_covered += 1;
                continue;
            };
            let records = layer.source_records.get(id);
            let verse = VerseId(layer.source_verse[id]);
            if records.is_empty() {
                table.unaligned += 1;
                if unaligned_examples.len() < EXAMPLES {
                    unaligned_examples.push(osis(verse));
                }
                continue;
            }
            table.aligned += 1;
            let mut targets: Vec<u32> = Vec::new();
            let mut others = false;
            for r in records {
                targets.extend_from_slice(layer.record_targets.get(*r as usize));
                others |= layer.record_sources.get(*r as usize).len() > 1;
            }
            targets.sort_unstable();
            targets.dedup();
            let mut rendering = String::new();
            for (k, t) in targets.iter().enumerate() {
                if k > 0 {
                    rendering.push_str(if *t == targets[k - 1] + 1 {
                        " "
                    } else {
                        " … "
                    });
                }
                rendering.push_str(Self::string(layer, layer.target_text[*t as usize]));
            }
            let entry = rows.entry(rendering).or_insert((0, 0, Vec::new(), order));
            entry.0 += 1;
            entry.1 += u64::from(others);
            if entry.2.len() < EXAMPLES {
                entry.2.push(osis(verse));
            }
        }
        let mut rows: Vec<(String, Tally)> = rows.into_iter().collect();
        rows.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.1.3.cmp(&b.1.3)));
        table.rows = rows
            .into_iter()
            .map(|(rendering, (count, others, examples, _))| RenderingRow {
                rendering,
                status: TokenStatus::Aligned,
                count,
                with_other_source_words: others,
                examples,
            })
            .collect();
        if table.unaligned > 0 {
            table.rows.push(RenderingRow {
                rendering: String::new(),
                status: TokenStatus::Unaligned,
                count: table.unaligned,
                with_other_source_words: 0,
                examples: unaligned_examples,
            });
        }
        Ok(table)
    }

    /// The records of one verse range of a layer in the `core::identity::AlignmentSet` form
    /// (specs/03): token ids on both sides (Hebrew morpheme ids where a token holds several),
    /// status `aligned`, the layer's origin, no confidence, both tokenization hashes. Words
    /// with no pack token cannot be named there and are left out; `omitted` counts them.
    pub fn alignment_set(
        &self,
        alignment_id: &str,
        from: VerseId,
        to: VerseId,
    ) -> Result<(AlignmentSet, usize)> {
        let layer = self
            .layers
            .iter()
            .find(|l| l.info.alignment_id == alignment_id)
            .ok_or_else(|| RetrievalError::LayerUnavailable(alignment_id.to_owned()))?;
        let mut records = Vec::new();
        let mut omitted = 0;
        let parse = |text: String| {
            TokenId::parse(&text).map_err(|e| RetrievalError::Storage(format!("{text}: {e}")))
        };
        for r in 0..layer.record_meta.len() {
            let sources = layer.record_sources.get(r);
            let targets = layer.record_targets.get(r);
            let in_range = sources
                .iter()
                .any(|s| (from.0..=to.0).contains(&layer.source_verse[*s as usize]))
                || targets
                    .iter()
                    .any(|t| (from.0..=to.0).contains(&layer.target_verse[*t as usize]));
            if !in_range {
                continue;
            }
            let mut source_ids = Vec::new();
            for s in sources {
                let i = *s as usize;
                if layer.source_word[i] == 0 {
                    omitted += 1;
                    continue;
                }
                source_ids.push(parse(morpheme_id(
                    &layer.info.source_edition,
                    VerseId(layer.source_verse[i]),
                    u32::from(layer.source_word[i]),
                    layer.source_morpheme[i],
                    layer.source_morphemes[i],
                ))?);
            }
            let mut target_ids: Vec<TokenId> = Vec::new();
            for t in targets {
                let i = *t as usize;
                if layer.target_word[i] == 0 {
                    omitted += 1;
                    continue;
                }
                let id = parse(token_id(
                    &layer.info.target_edition,
                    VerseId(layer.target_verse[i]),
                    u32::from(layer.target_word[i]),
                ))?;
                if !target_ids.contains(&id) {
                    target_ids.push(id);
                }
            }
            // A record whose words on one side all lack a token cannot be stated as a
            // non-null record; its words are already counted as omitted.
            if source_ids.is_empty() || target_ids.is_empty() {
                continue;
            }
            records.push(AlignmentRecord {
                source_token_ids: source_ids,
                target_token_ids: target_ids,
                status: AlignmentStatus::Aligned,
                origin: layer.info.origin.clone(),
                confidence: None,
            });
        }
        Ok((
            AlignmentSet {
                source_edition: layer.info.source_edition.clone(),
                source_revision: layer.info.source_text_content_revision.clone(),
                source_tokenization_hash: layer.info.source_tokenization_hash.clone(),
                target_edition: layer.info.target_edition.clone(),
                target_revision: layer.info.target_text_content_revision.clone(),
                target_tokenization_hash: layer.info.target_tokenization_hash.clone(),
                records,
            },
            omitted,
        ))
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::morphology::{Language, LayerInfo, LayerWriter, MorphemeRow, Reading};

    const HASH: &str = "1111111111111111111111111111111111111111111111111111111111111111";

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("openlogos-b067-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// John 1:1a in the shape of the SBLGNT morphology layer: Ἐν ἀρχῇ ἦν ὁ λόγος; and a
    /// sixth word, λόγος again, in John 1:14 (ὁ λόγος σὰρξ, cut to two words).
    fn morphology(dir: &Path, revision: &str) -> PathBuf {
        let path = dir.join("sblgnt.test.sqlite");
        let info = LayerInfo {
            layer_id: "macula-greek".into(),
            edition: "sblgnt".into(),
            title: "MACULA Greek".into(),
            revision: revision.into(),
            license: "CC-BY-4.0".into(),
            attribution: "Morphology attribution".into(),
            source: "test".into(),
            text_content_revision: "t".into(),
            text_manifest_sha256: "0".repeat(64),
            excluded_columns: vec![],
        };
        let mut w = LayerWriter::create(&path, &info, "org", &[]).unwrap();
        let words = [
            (VerseId::new(44, 1, 1), 1, "Ἐν", "ἐν", "In"),
            (VerseId::new(44, 1, 1), 2, "ἀρχῇ", "ἀρχή", "beginning"),
            (VerseId::new(44, 1, 1), 3, "ἦν", "εἰμί", "was"),
            (VerseId::new(44, 1, 1), 4, "ὁ", "ὁ", "the"),
            (VerseId::new(44, 1, 1), 5, "λόγος", "λόγος", "Word"),
            (VerseId::new(44, 1, 14), 1, "ὁ", "ὁ", "the"),
            (VerseId::new(44, 1, 14), 2, "λόγος", "λόγος", "Word"),
        ];
        for (i, (verse, word, surface, lemma, gloss)) in words.iter().enumerate() {
            w.add(&MorphemeRow {
                verse: *verse,
                word: *word,
                morpheme: 0,
                morphemes: 1,
                word_start: true,
                span: Some((0, surface.chars().count() as u32)),
                surface: (*surface).into(),
                reading: Reading::Written,
                join: "exact".into(),
                external_id: format!("n{:02}", i + 1),
                language: Language::Greek,
                lemma: (*lemma).into(),
                strong: String::new(),
                pos: "noun".into(),
                morph: "N-NSM".into(),
                parsing: BTreeMap::new(),
                gloss: (*gloss).into(),
                english: String::new(),
                transliteration: String::new(),
            })
            .unwrap();
        }
        w.finish().unwrap();
        path
    }

    fn info(revision: &str) -> AlignmentInfo {
        AlignmentInfo {
            alignment_id: "clear-sblgnt-bsb".into(),
            title: "Test".into(),
            source_edition: "sblgnt".into(),
            target_edition: "bsb".into(),
            revision: "a".into(),
            license: "CC-BY-4.0".into(),
            attribution: "Alignment attribution".into(),
            source: "test".into(),
            origin: "curated:clear-alignments@c99bd0a".into(),
            source_text_content_revision: "sblgnt-r".into(),
            source_text_manifest_sha256: "0".repeat(64),
            source_tokenization_hash: HASH.into(),
            source_morphology_layer: "macula-greek".into(),
            source_morphology_revision: revision.into(),
            target_text_content_revision: "bsb-r".into(),
            target_text_manifest_sha256: "0".repeat(64),
            target_tokenization_hash: HASH.into(),
        }
    }

    fn meta() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("origin".to_owned(), "manual".to_owned()),
            ("status".to_owned(), "created".to_owned()),
        ])
    }

    /// BSB John 1:1a `In the beginning was the Word,` (tokens 1–6) and 1:14a `The Word`
    /// (tokens 1–2), one changed token in 1:1 (7, `extra`). Records: Ἐν → In; ἀρχῇ → the
    /// beginning; ἦν → was; ὁ λόγος → the Word (two to two); 1:14 ὁ unaligned, λόγος → Word.
    fn alignment(dir: &Path) -> PathBuf {
        let path = dir.join("sblgnt-bsb.test.sqlite");
        let mut w = AlignmentWriter::create(&path, &info("m1")).unwrap();
        let v1 = VerseId::new(44, 1, 1);
        let v14 = VerseId::new(44, 1, 14);
        let s: Vec<u32> = [
            (v1, 1),
            (v1, 2),
            (v1, 3),
            (v1, 4),
            (v1, 5),
            (v14, 1),
            (v14, 2),
        ]
        .iter()
        .enumerate()
        .map(|(pos, (verse, word))| {
            w.add_source(SourceWordRow {
                pos: pos as u32,
                verse: *verse,
                word: *word,
                morpheme: 0,
                morphemes: 1,
                external_id: format!("n{:02}", pos + 1),
                text: String::new(),
                join: "exact".into(),
            })
        })
        .collect();
        let target =
            |w: &mut AlignmentWriter, verse: VerseId, word: u32, text: &str, join: &str| {
                w.add_target(TargetWordRow {
                    verse,
                    word,
                    span: (word != 0).then(|| (0, text.chars().count() as u32)),
                    external_id: format!("{}{word:03}", verse.0),
                    text: text.into(),
                    join: join.into(),
                })
            };
        let t_in = target(&mut w, v1, 1, "In", "exact");
        let t_the = target(&mut w, v1, 2, "the", "exact");
        let t_beginning = target(&mut w, v1, 3, "beginning", "exact");
        let t_was = target(&mut w, v1, 4, "was", "exact");
        let t_the2 = target(&mut w, v1, 5, "the", "exact");
        let t_word = target(&mut w, v1, 6, "Word", "punctuation");
        let t_gone = target(&mut w, v1, 0, "gone", "text_differs");
        let t14_the = target(&mut w, v14, 1, "The", "exact");
        let t14_word = target(&mut w, v14, 2, "Word", "exact");
        let _ = t14_the;
        let mut record = |id: &str, sources: Vec<u32>, targets: Vec<u32>| {
            w.add_record(RecordRow {
                external_id: id.into(),
                sources,
                targets,
                meta: meta(),
            })
            .unwrap();
        };
        record("43001001.001", vec![s[0]], vec![t_in]);
        record("43001001.002", vec![s[1]], vec![t_the, t_beginning]);
        record("43001001.003", vec![s[2]], vec![t_was, t_gone]);
        record("43001001.004", vec![s[3], s[4]], vec![t_the2, t_word]);
        record("43001014.002", vec![s[6]], vec![t14_word]);
        w.absent(Side::Target, v1, 7, None, "not_in_clear_target");
        w.finish().unwrap();
        path
    }

    fn layers(name: &str, revision: &str) -> (PathBuf, MorphologyLayers, AlignmentLayers) {
        let dir = temp(name);
        let mut m = MorphologyLayers::new().unwrap();
        m.attach_file(&morphology(&dir, revision)).unwrap();
        let mut a = AlignmentLayers::new().unwrap();
        a.attach_file(&alignment(&dir)).unwrap();
        (dir, m, a)
    }

    #[test]
    fn a_bsb_word_answers_its_greek_with_morphology_and_metadata() {
        let (dir, m, a) = layers("bsb", "m1");
        let answer = a.align(&["bsb:44001001003".to_owned()], Some(&m)).unwrap();
        let t = &answer.tokens[0];
        assert_eq!(t.status, TokenStatus::Aligned);
        assert_eq!(t.words[0].records, ["43001001.002"]);
        let g = &answer.groups[0];
        assert_eq!(g.record_id, "43001001.002");
        assert_eq!(g.origin, "curated:clear-alignments@c99bd0a");
        assert_eq!(g.meta["origin"], "manual");
        assert_eq!(g.sources[0].token_id.as_deref(), Some("sblgnt:44001001002"));
        let morph = g.sources[0].morphology.as_ref().unwrap();
        assert_eq!(
            (morph.lemma.as_str(), morph.gloss.as_str()),
            ("ἀρχή", "beginning")
        );
        let texts: Vec<&str> = g.targets.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["the", "beginning"]);
        assert!(!g.discontinuous);
        assert_eq!(
            answer.attribution,
            ["Alignment attribution", "Morphology attribution"]
        );
        // Two-to-two: either BSB word of `the Word` answers both Greek words.
        let answer = a.align(&["bsb:44001001006".to_owned()], Some(&m)).unwrap();
        let ids: Vec<&str> = answer.groups[0]
            .sources
            .iter()
            .map(|s| s.token_id.as_deref().unwrap())
            .collect();
        assert_eq!(ids, ["sblgnt:44001001004", "sblgnt:44001001005"]);
        // A selection gives each record once.
        let answer = a
            .align(
                &["bsb:44001001005".to_owned(), "bsb:44001001006".to_owned()],
                Some(&m),
            )
            .unwrap();
        assert_eq!(answer.groups.len(), 1);
        // A target word with no token stays in its record, said so.
        let answer = a.align(&["bsb:44001001004".to_owned()], None).unwrap();
        let gone = &answer.groups[0].targets[1];
        assert_eq!(
            (gone.token_id.as_deref(), gone.join.as_str()),
            (None, "text_differs")
        );
        assert!(answer.groups[0].sources[0].morphology.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unaligned_not_joined_unknown_and_no_layer_are_said() {
        let (dir, m, a) = layers("status", "m1");
        let ask = |t: &str| a.align(&[t.to_owned()], Some(&m)).unwrap().tokens.remove(0);
        let the = ask("bsb:44001014001");
        assert_eq!(the.status, TokenStatus::Unaligned);
        assert_eq!(the.words[0].status, TokenStatus::Unaligned);
        let changed = ask("bsb:44001001007");
        assert_eq!(
            (changed.status, changed.reason.as_deref()),
            (TokenStatus::NotJoined, Some("not_in_clear_target"))
        );
        let unknown = ask("bsb:44001001099");
        assert_eq!(
            (unknown.status, unknown.reason.as_deref()),
            (TokenStatus::NotJoined, Some("unknown_token"))
        );
        assert_eq!(ask("wlc:01001001001").status, TokenStatus::NoLayer);
        let greek_article = ask("sblgnt:44001014001");
        assert_eq!(greek_article.status, TokenStatus::Unaligned);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_greek_word_answers_its_bsb_words() {
        let (dir, m, a) = layers("greek", "m1");
        let answer = a
            .align(&["sblgnt:44001001005".to_owned()], Some(&m))
            .unwrap();
        assert_eq!(answer.tokens[0].status, TokenStatus::Aligned);
        let words: Vec<(&str, Option<&str>)> = answer.groups[0]
            .targets
            .iter()
            .map(|t| (t.text.as_str(), t.token_id.as_deref()))
            .collect();
        assert_eq!(
            words,
            [
                ("the", Some("bsb:44001001005")),
                ("Word", Some("bsb:44001001006"))
            ]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn renderings_state_scope_denominator_and_unaligned() {
        let (dir, m, a) = layers("render", "m1");
        let r = a.renderings(&m, &[], "λογος", None).unwrap();
        assert_eq!(r.matched_by, "key");
        assert_eq!(r.note, RENDERING_NOTE);
        let t = &r.tables[0];
        assert_eq!(
            (t.lemma.as_str(), t.scope.as_str()),
            ("λόγος", "SBLGNT, all books")
        );
        assert_eq!(
            (t.occurrences, t.aligned, t.unaligned, t.not_covered),
            (2, 2, 0, 0)
        );
        let rows: Vec<(&str, u64, u64)> = t
            .rows
            .iter()
            .map(|r| (r.rendering.as_str(), r.count, r.with_other_source_words))
            .collect();
        assert_eq!(rows, [("the Word", 1, 1), ("Word", 1, 0)]);
        assert_eq!(t.rows[1].examples, ["John.1.14"]);
        // ὁ: one aligned with λόγος, one unaligned, which is its own row.
        let t = &a.renderings(&m, &[], "ὁ", None).unwrap().tables[0];
        assert_eq!((t.occurrences, t.aligned, t.unaligned), (2, 1, 1));
        assert_eq!(t.rows.last().unwrap().status, TokenStatus::Unaligned);
        // A book scope counts only its occurrences; another book has none.
        let t = &a.renderings(&m, &[], "λόγος", Some("John")).unwrap().tables[0];
        assert_eq!((t.scope.as_str(), t.occurrences), ("SBLGNT, John", 2));
        let t = &a.renderings(&m, &[], "λόγος", Some("Matt")).unwrap().tables[0];
        assert_eq!(t.occurrences, 0);
        assert!(a.renderings(&m, &[], "λόγος", Some("Nope")).is_err());
        // An edition with no alignment is named.
        let r = a
            .renderings(&m, &["wlc".to_owned()], "λόγος", None)
            .unwrap();
        assert!(r.tables.is_empty());
        assert_eq!(r.unavailable, ["wlc: no alignment layer"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn another_morphology_revision_makes_lemma_answers_unavailable() {
        let (dir, m, a) = layers("mismatch", "m2");
        assert_eq!(a.morphology_mismatches(&m).len(), 1);
        let r = a.renderings(&m, &[], "λόγος", None).unwrap();
        assert!(r.tables.is_empty());
        assert!(r.unavailable[0].contains("m2"), "{:?}", r.unavailable);
        // Token answers still come, without morphology.
        let answer = a.align(&["bsb:44001001003".to_owned()], Some(&m)).unwrap();
        assert!(answer.groups[0].sources[0].morphology.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn records_export_as_a_valid_alignment_set() {
        let (dir, _m, a) = layers("set", "m1");
        let (set, omitted) = a
            .alignment_set(
                "clear-sblgnt-bsb",
                VerseId::new(44, 1, 1),
                VerseId::new(44, 1, 1),
            )
            .unwrap();
        set.validate().unwrap();
        set.check_tokenization(HASH, HASH).unwrap();
        assert!(set.check_tokenization(HASH, &"2".repeat(64)).is_err());
        assert_eq!(set.records.len(), 4);
        assert_eq!(omitted, 1, "the word with no token");
        assert_eq!(set.records[3].source_token_ids.len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn bytes_attach_like_files_and_wrong_input_is_refused() {
        let dir = temp("bytes");
        let path = alignment(&dir);
        let bytes = std::fs::read(&path).unwrap();
        let mut a = AlignmentLayers::new().unwrap();
        assert_eq!(a.attach_bytes(&bytes).unwrap(), "clear-sblgnt-bsb");
        assert!(a.attach_bytes(&bytes).is_err(), "attached twice");
        assert!(a.attach_bytes(b"not a database").is_err());
        assert_eq!(a.editions(), ["bsb", "sblgnt"]);
        let answer = a.align(&["bsb:44001001001".to_owned()], None).unwrap();
        assert_eq!(answer.groups[0].record_id, "43001001.001");
        let morph = morphology(&dir, "m1");
        let mut b = AlignmentLayers::new().unwrap();
        assert!(
            b.attach_file(&morph).is_err(),
            "a morphology layer is not an alignment"
        );
        assert!(
            AlignmentWriter::create(&path, &info("m1")).is_err(),
            "written once"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_tokenization_hash_is_the_specs_lines() {
        // SHA-256 of "bsb:44003016001\tFor\n".
        let one = tokenization_hash([("bsb:44003016001", "For")]);
        use sha2::{Digest, Sha256};
        let expected: String = Sha256::digest(b"bsb:44003016001\tFor\n")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(one, expected);
        assert_eq!(tokenization_hash(std::iter::empty()).len(), 64);
    }
}
