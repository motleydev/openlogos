//! Morphology layers (B026): one SQLite file per edition and source, keyed to the tokens of
//! that edition's text pack, read beside it. The file is a B064 pack (`token`, dictionary
//! columns and roaring postings per `(feature, value)`), so the columnar engine answers lemma
//! and morphology predicates from it, plus two tables the engine does not need:
//!
//! - `analysis`: per position (one morpheme) what is shown and not searched — the morpheme's
//!   place in its token, its scalar range inside the token's text, the reading it analyses
//!   (written, qere, or a qere with no written word), how it was joined, the upstream id, gloss,
//!   English and transliteration;
//! - `absence`: tokens of the text pack the layer has no analysis for, with the reason
//!   (`ketiv_not_read`: written in the codex, not read).
//!
//! A position is a morpheme: one per Greek word, one or more per Hebrew word (prefixes and
//! suffixes are their own positions; the article the vowel of a prefix implies is a position
//! with no letters of its own). `token.word` is the text pack's word number, 0 when the
//! morpheme belongs to no written token (a qere read but not written). Morpheme ids follow
//! `core::identity::mint`: a token with one morpheme keeps its id, a token with `n >= 2` gets
//! suffixes `a`, `b`, … in reading order.
//!
//! [`MorphologyLayers`] attaches layers (files natively, bytes in a browser) and answers the
//! two questions the core asks: what is this token ([`MorphologyLayers::analyze`]) and where
//! else is this lemma ([`MorphologyLayers::lemma_search`]). Analyses are never merged: two
//! layers over one edition that disagree make the token [`AnalysisStatus::Conflicting`] with
//! both analyses, each with its own attribution.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use openlogos_core::anchor::TokenId;
use openlogos_core::reference::{BookId, Reference};
use openlogos_retrieval::{Result, RetrievalError, VerseId};
use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};

use crate::pack::PACK_SCHEMA_VERSION;
use crate::token_engine::TokenColumns;
use crate::tokenizer::{NORMALIZATION_MODE, feature_key, normalize};

/// `layer_format` in the manifest of every morphology layer; readers refuse others.
pub const LAYER_FORMAT: &str = "openlogos-morphology-1";

/// The features every layer declares first, in this order; the parsing features follow.
pub const BASE_FEATURES: [&str; 6] = ["lemma", "lemma_key", "strong", "morph", "pos", "language"];

/// What a reading of a token analyses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reading {
    /// The word as written in the text pack.
    Written,
    /// The qere read for a written ketiv token: the analysis is of the reading, not of the
    /// letters on the page.
    Qere,
    /// A qere read where nothing is written (`Ruth 3:5`): no token of the text pack holds it.
    QereWithoutKetiv,
}

impl Reading {
    fn code(self) -> i64 {
        match self {
            Reading::Written => 0,
            Reading::Qere => 1,
            Reading::QereWithoutKetiv => 2,
        }
    }

    fn from_code(code: i64) -> Reading {
        match code {
            1 => Reading::Qere,
            2 => Reading::QereWithoutKetiv,
            _ => Reading::Written,
        }
    }

    /// `written`, `qere`, `qere_without_ketiv`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Reading::Written => "written",
            Reading::Qere => "qere",
            Reading::QereWithoutKetiv => "qere_without_ketiv",
        }
    }
}

/// The language of a morpheme, as a BCP 47 tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    /// Koine Greek, `grc`.
    Greek,
    /// Biblical Hebrew, `hbo`.
    Hebrew,
    /// Biblical Aramaic, `arc`.
    Aramaic,
}

impl Language {
    /// `grc`, `hbo`, `arc`.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Language::Greek => "grc",
            Language::Hebrew => "hbo",
            Language::Aramaic => "arc",
        }
    }

    /// The language a tag names.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Language> {
        match tag {
            "grc" => Some(Language::Greek),
            "hbo" => Some(Language::Hebrew),
            "arc" => Some(Language::Aramaic),
            _ => None,
        }
    }

    /// `Greek`, `Hebrew`, `Aramaic`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Language::Greek => "Greek",
            Language::Hebrew => "Hebrew",
            Language::Aramaic => "Aramaic",
        }
    }
}

// --- The layer's identity

/// A layer's manifest: who made it, under what terms, over which text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerInfo {
    /// `macula-greek`, `macula-hebrew`.
    pub layer_id: String,
    /// The edition whose tokens it analyses (`sblgnt`).
    pub edition: String,
    /// What a reader is shown as its name (`MACULA Greek`).
    pub title: String,
    /// The layer's revision (`macula-greek-8423afe`); a rebuild from other bytes is another.
    pub revision: String,
    /// SPDX licence, with the deed.
    pub license: String,
    /// The attribution the licence asks for, which a host shows with every analysis.
    pub attribution: String,
    /// Where it came from: repository, commit and the manifest hash of the checked bytes.
    pub source: String,
    /// The text pack's `content_revision` the tokens belong to.
    pub text_content_revision: String,
    /// SHA-256 of that text pack's `manifest.json`.
    pub text_manifest_sha256: String,
    /// The source columns deliberately not carried (the rights record's restricted modules).
    pub excluded_columns: Vec<String>,
}

// --- Writing

/// One morpheme as the importer hands it over, in reading order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MorphemeRow {
    /// Its verse.
    pub verse: VerseId,
    /// The text pack's word number of the token it belongs to; 0 for none.
    pub word: u32,
    /// 0-based index among the morphemes joined to that token.
    pub morpheme: u8,
    /// How many morphemes are joined to that token.
    pub morphemes: u8,
    /// The first morpheme of a source word (a qere of two words read for one written token
    /// starts two).
    pub word_start: bool,
    /// Scalar range inside the token's text, when the morpheme is written there.
    pub span: Option<(u32, u32)>,
    /// The morpheme's letters as the source has them (empty for an implied article).
    pub surface: String,
    /// What the analysis is of.
    pub reading: Reading,
    /// How the word was joined to the token (`exact`, `punctuation`, `qere`, …).
    pub join: String,
    /// The upstream id (MACULA `xml:id`).
    pub external_id: String,
    /// The language.
    pub language: Language,
    /// Lemma as the source spells it.
    pub lemma: String,
    /// Strong's number as the source gives it.
    pub strong: String,
    /// Part of speech.
    pub pos: String,
    /// The source's morphology code.
    pub morph: String,
    /// The decoded parsing fields, by the names the layer declares.
    pub parsing: BTreeMap<String, String>,
    /// Gloss.
    pub gloss: String,
    /// English rendering.
    pub english: String,
    /// Transliteration, when the source has one.
    pub transliteration: String,
}

const EXTRA_SCHEMA: &str = "
CREATE TABLE analysis(pos INTEGER PRIMARY KEY, morpheme INTEGER NOT NULL, morphemes INTEGER NOT NULL,
    word_start INTEGER NOT NULL, span_start INTEGER, span_end INTEGER, reading INTEGER NOT NULL, join_class INTEGER NOT NULL,
    external_id TEXT NOT NULL, lemma INTEGER, gloss INTEGER, english INTEGER, transliteration INTEGER);
CREATE TABLE string(id INTEGER PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE join_class(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
CREATE TABLE absence(verse_id INTEGER NOT NULL, word INTEGER NOT NULL, reason TEXT NOT NULL,
    PRIMARY KEY(verse_id, word)) WITHOUT ROWID;
";

/// One `analysis` row waiting for the pack to be finished.
#[cfg(not(target_arch = "wasm32"))]
struct PendingRow {
    pos: u32,
    morpheme: u8,
    morphemes: u8,
    word_start: bool,
    span: Option<(u32, u32)>,
    reading: Reading,
    join: i64,
    external_id: String,
    lemma: String,
    gloss: String,
    english: String,
    transliteration: String,
}

/// Writes one layer: a B064 pack through [`crate::PackBuilder`], then the layer's own tables
/// and manifest keys.
#[cfg(not(target_arch = "wasm32"))]
pub struct LayerWriter {
    path: PathBuf,
    builder: crate::pack::PackBuilder,
    info: LayerInfo,
    parsing: Vec<String>,
    rows: Vec<PendingRow>,
    join_classes: BTreeMap<String, i64>,
    absent: Vec<(VerseId, u32, String)>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LayerWriter {
    /// Creates the file (it must not exist) with the base features and `parsing` after them.
    pub fn create(
        path: &Path,
        info: &LayerInfo,
        scheme: &str,
        parsing: &[&str],
    ) -> Result<LayerWriter> {
        let mut features: Vec<&str> = BASE_FEATURES.to_vec();
        for p in parsing {
            if features.contains(p) {
                return Err(RetrievalError::InvalidQuery(format!(
                    "feature `{p}` declared twice"
                )));
            }
            features.push(p);
        }
        let manifest = crate::pack::PackManifest {
            edition: info.edition.clone(),
            scheme: scheme.to_owned(),
            source: info.source.clone(),
            license: info.license.clone(),
            attribution: info.attribution.clone(),
            work: None,
            locator_scheme: None,
        };
        let builder =
            crate::pack::PackBuilder::create(path, &manifest, &features).map_err(storage)?;
        Ok(LayerWriter {
            path: path.to_path_buf(),
            builder,
            info: info.clone(),
            parsing: parsing.iter().map(|p| (*p).to_owned()).collect(),
            rows: Vec::new(),
            join_classes: BTreeMap::new(),
            absent: Vec::new(),
        })
    }

    /// Adds one morpheme. Morphemes must arrive in reading order (verse order).
    pub fn add(&mut self, row: &MorphemeRow) -> Result<()> {
        if let Some(unknown) = row.parsing.keys().find(|k| !self.parsing.contains(k)) {
            return Err(RetrievalError::InvalidQuery(format!(
                "parsing feature `{unknown}` is not declared"
            )));
        }
        let key = normalize(&row.lemma);
        let mut features: Vec<Option<&str>> = vec![
            Some(row.lemma.as_str()),
            Some(key.as_str()),
            Some(row.strong.as_str()),
            Some(row.morph.as_str()),
            Some(row.pos.as_str()),
            Some(row.language.tag()),
        ];
        for p in &self.parsing {
            features.push(row.parsing.get(p).map(String::as_str));
        }
        let pos = self
            .builder
            .add_token(&crate::pack::TokenRow {
                verse: row.verse,
                word: row.word,
                surface: &row.surface,
                features,
            })
            .map_err(storage)?;
        let next = self.join_classes.len() as i64 + 1;
        let join = *self.join_classes.entry(row.join.clone()).or_insert(next);
        self.rows.push(PendingRow {
            pos,
            morpheme: row.morpheme,
            morphemes: row.morphemes,
            word_start: row.word_start,
            span: row.span,
            reading: row.reading,
            join,
            external_id: row.external_id.clone(),
            lemma: row.lemma.clone(),
            gloss: row.gloss.clone(),
            english: row.english.clone(),
            transliteration: row.transliteration.clone(),
        });
        Ok(())
    }

    /// Records a token of the text pack the layer has no analysis for, and why.
    pub fn absent(&mut self, verse: VerseId, word: u32, reason: &str) {
        self.absent.push((verse, word, reason.to_owned()));
    }

    /// Finishes the pack, then writes the layer's tables and manifest keys.
    pub fn finish(self) -> Result<()> {
        let LayerWriter {
            path,
            builder,
            info,
            parsing,
            rows,
            join_classes,
            absent,
        } = self;
        builder.finish().map_err(storage)?;
        let mut conn = Connection::open(&path).map_err(storage)?;
        let tx = conn.transaction().map_err(storage)?;
        tx.execute_batch(EXTRA_SCHEMA).map_err(storage)?;
        {
            let mut insert = tx
                .prepare(
                    "INSERT INTO analysis(pos, morpheme, morphemes, word_start, span_start, span_end,
                     reading, join_class, external_id, lemma, gloss, english, transliteration)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                )
                .map_err(storage)?;
            // Repeated text (glosses, renderings, transliterations, a lemma whose bytes are not
            // its NFC dictionary key) is stored once; NULL is empty, or for the lemma, the key.
            let mut strings: std::collections::HashMap<String, i64> =
                std::collections::HashMap::new();
            let mut id_of = |text: &str| -> Option<i64> {
                if text.is_empty() {
                    return None;
                }
                let next = strings.len() as i64 + 1;
                Some(*strings.entry(text.to_owned()).or_insert(next))
            };
            for r in &rows {
                let lemma = (feature_key(&r.lemma) != r.lemma)
                    .then(|| id_of(&r.lemma))
                    .flatten();
                insert
                    .execute(params![
                        i64::from(r.pos),
                        i64::from(r.morpheme),
                        i64::from(r.morphemes),
                        i64::from(r.word_start),
                        r.span.map(|s| i64::from(s.0)),
                        r.span.map(|s| i64::from(s.1)),
                        r.reading.code(),
                        r.join,
                        r.external_id,
                        lemma,
                        id_of(&r.gloss),
                        id_of(&r.english),
                        id_of(&r.transliteration)
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
                .prepare("INSERT INTO join_class(id, name) VALUES (?1, ?2)")
                .map_err(storage)?;
            for (name, id) in &join_classes {
                insert.execute(params![id, name]).map_err(storage)?;
            }
            let mut insert = tx
                .prepare("INSERT INTO absence(verse_id, word, reason) VALUES (?1, ?2, ?3)")
                .map_err(storage)?;
            for (verse, word, reason) in &absent {
                insert
                    .execute(params![i64::from(verse.0), i64::from(*word), reason])
                    .map_err(storage)?;
            }
            let mut insert = tx
                .prepare("INSERT INTO manifest(key, value) VALUES (?1, ?2)")
                .map_err(storage)?;
            for (key, value) in [
                ("layer_format", LAYER_FORMAT.to_owned()),
                ("layer_id", info.layer_id.clone()),
                ("layer_title", info.title.clone()),
                ("layer_revision", info.revision.clone()),
                ("text_content_revision", info.text_content_revision.clone()),
                ("text_manifest_sha256", info.text_manifest_sha256.clone()),
                ("excluded_columns", info.excluded_columns.join(",")),
                ("parsing_features", parsing.join(",")),
            ] {
                insert.execute(params![key, value]).map_err(storage)?;
            }
        }
        // The engine reads `token` by position only; the verse index is the B064 writer's, for
        // packs whose text it serves, and a layer has none.
        tx.execute_batch("DROP INDEX token_verse")
            .map_err(storage)?;
        tx.commit().map_err(storage)?;
        conn.execute_batch("VACUUM").map_err(storage)?;
        conn.close().map_err(|(_, e)| storage(e))
    }
}

// --- Reading

/// One named parsing field (`tense` = `aorist`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MorphFeature {
    /// `tense`, `stem`, `case`, …
    pub name: String,
    /// The source's value.
    pub value: String,
}

/// One analysed morpheme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Morpheme {
    /// The morpheme's id (`wlc:01001001001b`), or the token id when the token is one
    /// morpheme; `None` for a qere with no written token.
    pub id: Option<String>,
    /// The upstream id (MACULA `xml:id`).
    pub external_id: String,
    /// Its letters as the source has them; empty for an article only the vowel implies.
    pub surface: String,
    /// Scalar range inside the token's text, when written there.
    pub start: Option<u32>,
    /// End of that range, exclusive.
    pub end: Option<u32>,
    /// A morpheme with no letters of its own (the article in `בָּעוֹף`).
    pub implied: bool,
    /// Its language.
    pub language: Language,
    /// The lemma as the source spells it.
    pub lemma: String,
    /// Strong's number as the source gives it (namespaced by the layer, never alone).
    pub strong: String,
    /// Part of speech.
    pub pos: String,
    /// The source's morphology code (`V-AAI-3S`, `Vqp3ms`).
    pub morph_code: String,
    /// The decoded parsing fields that have a value, in the layer's order.
    pub features: Vec<MorphFeature>,
    /// The parsing in words (`verb · qal qatal · third person masculine singular`).
    pub description: String,
    /// Gloss.
    pub gloss: String,
    /// English rendering.
    pub english: String,
    /// Transliteration, when the source has one.
    pub transliteration: String,
}

/// One layer's analysis of a token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Analysis {
    /// The layer.
    pub layer_id: String,
    /// Its title.
    pub layer_title: String,
    /// The attribution the layer's licence asks for; show it with the analysis.
    pub attribution: String,
    /// What is analysed: the written word, or the qere read for it.
    pub reading: Reading,
    /// How the layer's word was joined to the token.
    pub join: String,
    /// The analysed reading's letters, morphemes run together and words split by a space;
    /// for a qere this is the reading the text pack does not print.
    pub text: String,
    /// The morphemes in reading order.
    pub morphemes: Vec<Morpheme>,
}

/// Whether the token has an analysis, one, or several that disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    /// Every installed layer that analyses the token agrees (often there is one).
    Analyzed,
    /// Two or more layers analyse it differently; all are kept, none preferred.
    Conflicting,
    /// No analysis: see the reason.
    Absent,
}

/// What the layers say about one token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenAnalysis {
    /// The token asked about (a morpheme suffix is dropped: the whole token is answered).
    pub token_id: String,
    /// Its edition.
    pub edition: String,
    /// The status.
    pub status: AnalysisStatus,
    /// Why there is none: `no_layer` (no layer for this edition is installed),
    /// `ketiv_not_read` (written in the codex, not read, so not analysed), `not_analyzed`
    /// (the layer has no word for this token).
    pub absent_reason: Option<String>,
    /// One per layer that analyses the token.
    pub analyses: Vec<Analysis>,
}

/// One lemma among a search's matches, with how often it occurs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LemmaCount {
    /// The edition.
    pub edition: String,
    /// The lemma as the source spells it.
    pub lemma: String,
    /// Its Strong's number(s) as the source gives them, most frequent first.
    pub strong: String,
    /// Its language.
    pub language: Language,
    /// Morphemes with this lemma in the edition.
    pub count: u64,
}

/// One occurrence a lemma search found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LemmaHit {
    /// The edition.
    pub edition: String,
    /// The verse, OSIS (`Gen.1.1`), which is also the segment id.
    pub reference: String,
    /// The text pack's token holding it; `None` for a qere with no written token.
    pub token_id: Option<String>,
    /// The morpheme's id.
    pub morpheme_id: Option<String>,
    /// The morpheme's letters.
    pub surface: String,
    /// Its lemma.
    pub lemma: String,
    /// What it analyses.
    pub reading: Reading,
}

/// A lemma search's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LemmaSearch {
    /// What was asked.
    pub query: String,
    /// `exact` when the query is a lemma as spelled (points and accents included), else
    /// `key`: the query and the lemmas compared without points, accents or case.
    pub matched_by: String,
    /// The lemmas matched, most frequent first: homographs that share a key stay apart.
    pub lemmas: Vec<LemmaCount>,
    /// Every match, across the editions searched.
    pub total: u64,
    /// The first matches in edition and reading order, at most the limit.
    pub hits: Vec<LemmaHit>,
    /// Editions asked for that have no layer installed.
    pub unavailable: Vec<String>,
}

/// Per matched lemma: count, its first position, and counts per Strong's value id.
type LemmaTally = (u64, usize, BTreeMap<u32, u64>);

struct Layer {
    schema: String,
    info: LayerInfo,
    parsing: Vec<String>,
    columns: TokenColumns,
    /// Text pack word number per position.
    word: Vec<u16>,
}

/// The installed morphology layers.
pub struct MorphologyLayers {
    conn: Connection,
    layers: Vec<Layer>,
}

impl std::fmt::Debug for MorphologyLayers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MorphologyLayers")
            .field(
                "layers",
                &self
                    .layers
                    .iter()
                    .map(|l| &l.info.layer_id)
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

/// The id of a token: `edition:BBCCCVVVWWW`.
fn token_id(edition: &str, verse: VerseId, word: u32) -> String {
    format!(
        "{edition}:{:02}{:03}{:03}{word:03}",
        verse.book(),
        verse.chapter(),
        verse.verse()
    )
}

/// The id of a morpheme: the token's, with a suffix when the token has several.
fn morpheme_id(edition: &str, verse: VerseId, word: u32, morpheme: u8, morphemes: u8) -> String {
    let mut id = token_id(edition, verse, word);
    if morphemes >= 2 {
        id.push(char::from(b'a' + morpheme.min(25)));
    }
    id
}

fn osis(verse: VerseId) -> String {
    BookId::from_number(verse.book()).map_or_else(
        || verse.0.to_string(),
        |book| {
            Reference::verse(book, u32::from(verse.chapter()), u32::from(verse.verse())).to_osis()
        },
    )
}

impl MorphologyLayers {
    /// No layers yet.
    pub fn new() -> Result<MorphologyLayers> {
        let conn = Connection::open_in_memory_with_flags(
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(storage)?;
        Ok(MorphologyLayers {
            conn,
            layers: Vec::new(),
        })
    }

    /// Attaches every `*.sqlite` of a directory, in name order.
    pub fn open_directory(directory: &Path) -> Result<MorphologyLayers> {
        let mut layers = MorphologyLayers::new()?;
        let read = std::fs::read_dir(directory)
            .map_err(|e| RetrievalError::Storage(format!("{}: {e}", directory.display())))?;
        let mut paths: Vec<PathBuf> = read
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "sqlite"))
            .collect();
        paths.sort();
        if paths.is_empty() {
            return Err(RetrievalError::Storage(format!(
                "{} holds no morphology layers",
                directory.display()
            )));
        }
        for path in paths {
            layers.attach_file(&path)?;
        }
        Ok(layers)
    }

    fn next_schema(&self) -> String {
        format!("m{}", self.layers.len())
    }

    /// Attaches one layer from a file, read-only. Returns its layer id.
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
                "these bytes are not a morphology layer".to_owned(),
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
                let id = layer.info.layer_id.clone();
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
        let version = value("schema_version")?;
        if version != PACK_SCHEMA_VERSION {
            return Err(RetrievalError::Storage(format!(
                "pack schema {version} is not the supported {PACK_SCHEMA_VERSION}"
            )));
        }
        let mode = value("normalization_mode")?;
        if mode != NORMALIZATION_MODE {
            return Err(RetrievalError::Storage(format!(
                "pack normalization {mode} is not this engine's {NORMALIZATION_MODE}"
            )));
        }
        let format = value("layer_format")?;
        if format != LAYER_FORMAT {
            return Err(RetrievalError::Storage(format!(
                "layer format {format} is not the supported {LAYER_FORMAT}"
            )));
        }
        let split = |text: String| -> Vec<String> {
            text.split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        };
        let info = LayerInfo {
            layer_id: value("layer_id")?,
            edition: value("edition")?,
            title: value("layer_title")?,
            revision: value("layer_revision")?,
            license: value("license")?,
            attribution: value("attribution")?,
            source: value("source")?,
            text_content_revision: value("text_content_revision")?,
            text_manifest_sha256: value("text_manifest_sha256")?,
            excluded_columns: split(value("excluded_columns")?),
        };
        if self
            .layers
            .iter()
            .any(|l| l.info.layer_id == info.layer_id && l.info.edition == info.edition)
        {
            return Err(RetrievalError::Storage(format!(
                "layer `{}` over `{}` is already attached",
                info.layer_id, info.edition
            )));
        }
        let parsing = split(value("parsing_features")?);
        let columns = TokenColumns::load(&self.conn, schema)?;
        for name in BASE_FEATURES
            .iter()
            .copied()
            .chain(parsing.iter().map(String::as_str))
        {
            if columns.feature(name).is_none() {
                return Err(RetrievalError::Storage(format!(
                    "layer `{}` has no feature `{name}`",
                    info.layer_id
                )));
            }
        }
        let word: Vec<u16> = {
            let mut stmt = self
                .conn
                .prepare(&format!("SELECT word FROM \"{schema}\".token ORDER BY pos"))
                .map_err(storage)?;
            let rows = stmt
                .query_map([], |r| r.get::<_, i64>(0))
                .map_err(storage)?;
            rows.map(|r| r.map(|w| u16::try_from(w).unwrap_or(0)))
                .collect::<std::result::Result<_, _>>()
                .map_err(storage)?
        };
        Ok(Layer {
            schema: schema.to_owned(),
            info,
            parsing,
            columns,
            word,
        })
    }

    /// The attached layers, in attach order.
    #[must_use]
    pub fn layers(&self) -> Vec<LayerInfo> {
        self.layers.iter().map(|l| l.info.clone()).collect()
    }

    /// The editions some layer analyses, sorted, once each.
    #[must_use]
    pub fn editions(&self) -> Vec<String> {
        let mut editions: Vec<String> =
            self.layers.iter().map(|l| l.info.edition.clone()).collect();
        editions.sort();
        editions.dedup();
        editions
    }

    /// Whether some layer analyses `edition`.
    #[must_use]
    pub fn has_edition(&self, edition: &str) -> bool {
        self.layers.iter().any(|l| l.info.edition == edition)
    }

    /// The positions of a layer that belong to `word` of `verse`.
    fn positions(layer: &Layer, verse: VerseId, word: u32) -> std::ops::Range<usize> {
        let verses = &layer.columns.verse;
        let lo = verses.partition_point(|v| *v < verse.0);
        let hi = verses.partition_point(|v| *v <= verse.0);
        let mut first = None;
        let mut last = lo;
        for p in lo..hi {
            if u32::from(layer.word[p]) == word {
                first.get_or_insert(p);
                last = p + 1;
            }
        }
        first.map_or(lo..lo, |f| f..last)
    }

    fn value(layer: &Layer, feature: &str, pos: usize) -> String {
        layer
            .columns
            .feature(feature)
            .map_or_else(String::new, |c| c.values[c.ids[pos] as usize].clone())
    }

    fn language(layer: &Layer, pos: usize) -> Language {
        Language::from_tag(&Self::value(layer, "language", pos)).unwrap_or(Language::Greek)
    }

    /// Reads the morphemes at `positions` of a layer.
    fn morphemes(
        &self,
        layer: &Layer,
        positions: std::ops::Range<usize>,
    ) -> Result<Vec<(Morpheme, Reading, String, bool)>> {
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "SELECT t.verse_id, t.word, t.surface, a.morpheme, a.morphemes | (a.word_start << 8), a.span_start,
                        a.span_end, a.reading, j.name, a.external_id, coalesce(l.value, ''),
                        coalesce(g.value, ''), coalesce(e.value, ''), coalesce(x.value, '')
                 FROM \"{s}\".token t JOIN \"{s}\".analysis a ON a.pos = t.pos
                 JOIN \"{s}\".join_class j ON j.id = a.join_class
                 LEFT JOIN \"{s}\".string l ON l.id = a.lemma
                 LEFT JOIN \"{s}\".string g ON g.id = a.gloss
                 LEFT JOIN \"{s}\".string e ON e.id = a.english
                 LEFT JOIN \"{s}\".string x ON x.id = a.transliteration
                 WHERE t.pos >= ?1 AND t.pos < ?2 ORDER BY t.pos",
                s = layer.schema
            ))
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![positions.start as i64, positions.end as i64], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, String>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, String>(10)?,
                    r.get::<_, String>(11)?,
                    r.get::<_, String>(12)?,
                    r.get::<_, String>(13)?,
                ))
            })
            .map_err(storage)?;
        let mut out = Vec::new();
        for (pos, row) in positions.zip(rows) {
            let (
                verse,
                word,
                surface,
                morpheme,
                morphemes,
                start,
                end,
                reading,
                join,
                external_id,
                lemma,
                gloss,
                english,
                transliteration,
            ) = row.map_err(storage)?;
            let verse = VerseId(u32::try_from(verse).unwrap_or(0));
            let word = u32::try_from(word).unwrap_or(0);
            let word_start = morphemes >> 8 != 0;
            let morphemes = morphemes & 0xff;
            let language = Self::language(layer, pos);
            let pos_name = Self::value(layer, "pos", pos);
            let features: Vec<MorphFeature> = layer
                .parsing
                .iter()
                .filter_map(|name| {
                    let value = Self::value(layer, name, pos);
                    (!value.is_empty()).then(|| MorphFeature {
                        name: name.clone(),
                        value,
                    })
                })
                .collect();
            let description = describe(language, &pos_name, &features);
            let implied = surface.is_empty();
            // Stored only when its bytes are not its dictionary key.
            let lemma = if lemma.is_empty() {
                Self::value(layer, "lemma", pos)
            } else {
                lemma
            };
            out.push((
                Morpheme {
                    id: (word != 0).then(|| {
                        morpheme_id(
                            &layer.info.edition,
                            verse,
                            word,
                            u8::try_from(morpheme).unwrap_or(0),
                            u8::try_from(morphemes).unwrap_or(1),
                        )
                    }),
                    external_id,
                    surface,
                    start: start.and_then(|s| u32::try_from(s).ok()),
                    end: end.and_then(|s| u32::try_from(s).ok()),
                    implied,
                    language,
                    lemma,
                    strong: Self::value(layer, "strong", pos),
                    pos: pos_name,
                    morph_code: Self::value(layer, "morph", pos),
                    features,
                    description,
                    gloss,
                    english,
                    transliteration,
                },
                Reading::from_code(reading),
                join,
                word_start,
            ));
        }
        Ok(out)
    }

    fn absence(&self, layer: &Layer, verse: VerseId, word: u32) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "SELECT reason FROM \"{}\".absence WHERE verse_id = ?1 AND word = ?2",
                layer.schema
            ))
            .map_err(storage)?;
        let mut rows = stmt
            .query(params![i64::from(verse.0), i64::from(word)])
            .map_err(storage)?;
        Ok(match rows.next().map_err(storage)? {
            Some(row) => Some(row.get(0).map_err(storage)?),
            None => None,
        })
    }

    /// What the installed layers say about a token of an edition's text pack.
    pub fn analyze(&self, token: &str) -> Result<TokenAnalysis> {
        let id = TokenId::parse(token)
            .map_err(|e| RetrievalError::InvalidQuery(format!("token id `{token}`: {e}")))?;
        let verse = VerseId::new(
            id.book.number(),
            u16::try_from(id.chapter).unwrap_or(0),
            u16::try_from(id.verse).unwrap_or(0),
        );
        let whole = token_id(&id.edition, verse, id.word);
        let mut analyses = Vec::new();
        let mut reasons = Vec::new();
        let mut any_layer = false;
        for layer in self.layers.iter().filter(|l| l.info.edition == id.edition) {
            any_layer = true;
            let positions = Self::positions(layer, verse, id.word);
            if positions.is_empty() {
                reasons.push(
                    self.absence(layer, verse, id.word)?
                        .unwrap_or_else(|| "not_analyzed".to_owned()),
                );
                continue;
            }
            let rows = self.morphemes(layer, positions)?;
            let reading = rows.first().map_or(Reading::Written, |r| r.1);
            let join = rows.first().map(|r| r.2.clone()).unwrap_or_default();
            analyses.push(Analysis {
                layer_id: layer.info.layer_id.clone(),
                layer_title: layer.info.title.clone(),
                attribution: layer.info.attribution.clone(),
                reading,
                join,
                text: reading_text(&rows),
                morphemes: rows.into_iter().map(|r| r.0).collect(),
            });
        }
        let status = match analyses.len() {
            0 => AnalysisStatus::Absent,
            1 => AnalysisStatus::Analyzed,
            _ => {
                let key = |a: &Analysis| -> Vec<(String, String)> {
                    a.morphemes
                        .iter()
                        .map(|m| (feature_key(&m.lemma), m.morph_code.clone()))
                        .collect()
                };
                let first = key(&analyses[0]);
                if analyses.iter().all(|a| key(a) == first) {
                    AnalysisStatus::Analyzed
                } else {
                    AnalysisStatus::Conflicting
                }
            }
        };
        let absent_reason = (status == AnalysisStatus::Absent).then(|| {
            if any_layer {
                reasons
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "not_analyzed".to_owned())
            } else {
                "no_layer".to_owned()
            }
        });
        Ok(TokenAnalysis {
            token_id: whole,
            edition: id.edition,
            status,
            absent_reason,
            analyses,
        })
    }

    /// Every occurrence of a lemma in the editions asked for (every edition with a layer when
    /// `editions` is empty). A query spelled exactly as a lemma (points and accents included)
    /// matches that lemma only; otherwise query and lemmas are compared by the shared search
    /// key (`ol-norm-1`: no points, accents or case), so `ראשית` finds `רֵאשִׁית`, which is
    /// how `בְּרֵאשִׁית` is found, and `λογος` every form of `λόγος`.
    pub fn lemma_search(
        &self,
        editions: &[String],
        query: &str,
        limit: usize,
    ) -> Result<LemmaSearch> {
        let query = query.trim();
        let key = normalize(query);
        let unavailable: Vec<String> = editions
            .iter()
            .filter(|e| !self.has_edition(e))
            .cloned()
            .collect();
        let layers: Vec<&Layer> = self
            .layers
            .iter()
            .filter(|l| editions.is_empty() || editions.contains(&l.info.edition))
            .collect();
        let mut result = LemmaSearch {
            query: query.to_owned(),
            matched_by: "key".to_owned(),
            lemmas: Vec::new(),
            total: 0,
            hits: Vec::new(),
            unavailable,
        };
        if key.is_empty() {
            return Ok(result);
        }
        // Exact when any layer has the lemma as typed.
        let exact = layers.iter().any(|l| {
            l.columns
                .feature("lemma")
                .is_some_and(|c| c.lookup.contains_key(&feature_key(query)))
        });
        let (feature, value) = if exact {
            result.matched_by = "exact".to_owned();
            ("lemma", query.to_owned())
        } else {
            ("lemma_key", key)
        };
        let mut lemma_rows: BTreeMap<(String, u32), LemmaTally> = BTreeMap::new();
        let mut wanted: Vec<(usize, u32)> = Vec::new();
        for (index, layer) in layers.iter().enumerate() {
            let bitmap = layer
                .columns
                .posting(&self.conn, &layer.schema, feature, &value)?;
            result.total += bitmap.len();
            let lemma = layer.columns.feature("lemma").expect("checked on attach");
            let strong = layer.columns.feature("strong").expect("checked on attach");
            for pos in bitmap.iter() {
                let entry = lemma_rows
                    .entry((layer.info.edition.clone(), lemma.ids[pos as usize]))
                    .or_insert((0, pos as usize, BTreeMap::new()));
                entry.0 += 1;
                *entry.2.entry(strong.ids[pos as usize]).or_default() += 1;
                if wanted.len() < limit {
                    wanted.push((index, pos));
                }
            }
        }
        for ((edition, _), (count, first, strongs)) in lemma_rows {
            let layer = layers
                .iter()
                .find(|l| l.info.edition == edition)
                .expect("came from these layers");
            let mut strongs: Vec<(u32, u64)> = strongs.into_iter().collect();
            strongs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            let strong_column = layer.columns.feature("strong").expect("checked on attach");
            let display = self.morphemes(layer, first..first + 1)?;
            result.lemmas.push(LemmaCount {
                edition,
                lemma: display
                    .first()
                    .map(|m| m.0.lemma.clone())
                    .unwrap_or_default(),
                strong: strongs
                    .iter()
                    .map(|(id, _)| strong_column.values[*id as usize].clone())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" "),
                language: Self::language(layer, first),
                count,
            });
        }
        result
            .lemmas
            .sort_by(|a, b| b.count.cmp(&a.count).then(a.lemma.cmp(&b.lemma)));
        for (index, pos) in wanted {
            let layer = layers[index];
            let pos = pos as usize;
            let verse = VerseId(layer.columns.verse[pos]);
            let word = u32::from(layer.word[pos]);
            let (morpheme, reading, _, _) = self
                .morphemes(layer, pos..pos + 1)?
                .into_iter()
                .next()
                .ok_or_else(|| RetrievalError::Storage(format!("position {pos} has no row")))?;
            result.hits.push(LemmaHit {
                edition: layer.info.edition.clone(),
                reference: osis(verse),
                token_id: (word != 0).then(|| token_id(&layer.info.edition, verse, word)),
                morpheme_id: morpheme.id.clone(),
                surface: morpheme.surface,
                lemma: morpheme.lemma,
                reading,
            });
        }
        Ok(result)
    }

    /// The lemma a token is searched by: its last morpheme that is not a suffix (`רֵאשִׁית`
    /// of `בְּרֵאשִׁית`, `עוֹף` of `בָּעוֹף`), from its first analysis. `None` when the token
    /// has none.
    pub fn principal_lemma(&self, token: &str) -> Result<Option<String>> {
        let analysis = self.analyze(token)?;
        Ok(analysis.analyses.first().and_then(|a| {
            a.morphemes
                .iter()
                .rev()
                .find(|m| m.pos != "suffix")
                .or_else(|| a.morphemes.first())
                .map(|m| m.lemma.clone())
        }))
    }

    /// [`Self::lemma_search`] for a token's principal lemma, in the token's edition.
    pub fn lemma_search_token(&self, token: &str, limit: usize) -> Result<LemmaSearch> {
        let edition = TokenId::parse(token)
            .map_err(|e| RetrievalError::InvalidQuery(format!("token id `{token}`: {e}")))?
            .edition;
        let lemma = self
            .principal_lemma(token)?
            .ok_or_else(|| RetrievalError::LayerUnavailable(format!("no analysis of `{token}`")))?;
        self.lemma_search(&[edition], &lemma, limit)
    }

    /// The text pack word number of a position of an edition's first layer (0: none).
    #[must_use]
    pub fn word_of(&self, edition: &str, pos: usize) -> Option<u32> {
        self.layers
            .iter()
            .find(|l| l.info.edition == edition)
            .and_then(|l| l.word.get(pos))
            .map(|w| u32::from(*w))
    }

    /// The B064 token columns of an edition's first layer, for the engine's slot queries.
    #[must_use]
    pub fn columns(&self, edition: &str) -> Option<(&Connection, &str, &TokenColumns)> {
        self.layers
            .iter()
            .find(|l| l.info.edition == edition)
            .map(|l| (&self.conn, l.schema.as_str(), &l.columns))
    }
}

/// The reading's letters: morphemes run together, a space between source words.
fn reading_text(rows: &[(Morpheme, Reading, String, bool)]) -> String {
    let mut text = String::new();
    for (m, _, _, word_start) in rows {
        if *word_start && !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&m.surface);
    }
    text
}

fn value_of<'a>(features: &'a [MorphFeature], name: &str) -> Option<&'a str> {
    features
        .iter()
        .find(|f| f.name == name)
        .map(|f| f.value.as_str())
}

/// The parsing in words, from the decoded fields the source supplies (the code itself is
/// not re-parsed): part of speech, then the verbal or nominal fields in the order a grammar
/// names them.
#[must_use]
pub fn describe(language: Language, pos: &str, features: &[MorphFeature]) -> String {
    let get = |name: &str| value_of(features, name);
    let pos_word = match pos {
        "det" => "article",
        "conj" | "cj" => "conjunction",
        "prep" => "preposition",
        "adj" => "adjective",
        "adv" => "adverb",
        "pron" => "pronoun",
        "ptcl" => "particle",
        "intj" | "ij" => "interjection",
        "num" => "numeral",
        other => other,
    };
    let mut head = pos_word.to_owned();
    if let Some(kind) = get("type").filter(|t| !t.is_empty() && *t != pos_word) {
        // Hebrew verbs carry the conjugation as their type; it reads with the stem below.
        if !(language != Language::Greek && pos == "verb") {
            head = format!("{head} ({kind})");
        }
    }
    let mut parts = vec![head];
    let join = |names: &[&str]| -> String {
        names
            .iter()
            .filter_map(|n| get(n))
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let person = get("person").map(|p| {
        if p.starts_with("unknown") {
            format!("person {p}")
        } else {
            format!("{p} person")
        }
    });
    match language {
        Language::Greek => {
            let verbal = join(&["tense", "voice", "mood"]);
            if !verbal.is_empty() {
                parts.push(verbal);
            }
            let mood = get("mood").unwrap_or("");
            let nominal = join(&["case", "number", "gender"]);
            if mood != "participle"
                && mood != "infinitive"
                && let Some(person) = &person
            {
                parts.push(format!("{person} {}", join(&["number"])).trim().to_owned());
            } else if !nominal.is_empty() {
                parts.push(nominal);
            }
            if let Some(degree) = get("degree").filter(|d| !d.is_empty()) {
                parts.push(degree.to_owned());
            }
        }
        Language::Hebrew | Language::Aramaic => {
            if pos == "verb" {
                let verbal = join(&["stem", "type"]);
                if !verbal.is_empty() {
                    parts.push(verbal);
                }
            }
            let mut agreement = Vec::new();
            if let Some(person) = person {
                agreement.push(person);
            }
            let rest = join(&["gender", "number", "state"]);
            if !rest.is_empty() {
                agreement.push(rest);
            }
            if !agreement.is_empty() {
                parts.push(agreement.join(" "));
            }
        }
    }
    parts.join(" · ")
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn info(layer: &str, edition: &str) -> LayerInfo {
        LayerInfo {
            layer_id: layer.to_owned(),
            edition: edition.to_owned(),
            title: format!("Test {layer}"),
            revision: "r1".to_owned(),
            license: "CC-BY-4.0".to_owned(),
            attribution: format!("Attribution of {layer}"),
            source: "test".to_owned(),
            text_content_revision: "t1".to_owned(),
            text_manifest_sha256: "0".repeat(64),
            excluded_columns: vec!["ln".to_owned()],
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        verse: VerseId,
        word: u32,
        morpheme: u8,
        morphemes: u8,
        span: Option<(u32, u32)>,
        surface: &str,
        lemma: &str,
        pos: &str,
        morph: &str,
        parsing: &[(&str, &str)],
        language: Language,
        reading: Reading,
    ) -> MorphemeRow {
        MorphemeRow {
            verse,
            word,
            morpheme,
            morphemes,
            word_start: morpheme == 0,
            span,
            surface: surface.to_owned(),
            reading,
            join: if reading == Reading::Written {
                "exact"
            } else {
                "qere"
            }
            .to_owned(),
            external_id: format!("x{}{word:03}{}", verse.0, morpheme + 1),
            language,
            lemma: lemma.to_owned(),
            strong: "0000".to_owned(),
            pos: pos.to_owned(),
            morph: morph.to_owned(),
            parsing: parsing
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            gloss: format!("gloss of {lemma}"),
            english: String::new(),
            transliteration: String::new(),
        }
    }

    fn temp(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "openlogos-morphology-{name}-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    const HEBREW_PARSING: [&str; 6] = ["person", "gender", "number", "state", "stem", "type"];

    /// Gen 1:1 words 1–2 and a made-up ketiv token 3 read as a qere, a ketiv not read
    /// (word 4) and a qere with no written token, in the shape of the WLC layer.
    fn hebrew_layer(name: &str, lemma_two: &str) -> PathBuf {
        let path = temp(name);
        let mut w = LayerWriter::create(&path, &info(name, "wlc"), "org", &HEBREW_PARSING).unwrap();
        let v = VerseId::new(1, 1, 1);
        let h = Language::Hebrew;
        for r in [
            row(
                v,
                1,
                0,
                2,
                Some((0, 2)),
                "בְּ",
                "בְּ",
                "preposition",
                "R",
                &[],
                h,
                Reading::Written,
            ),
            row(
                v,
                1,
                1,
                2,
                Some((2, 11)),
                "רֵאשִׁ֖ית",
                "רֵאשִׁית",
                "noun",
                "Ncfsa",
                &[
                    ("gender", "feminine"),
                    ("number", "singular"),
                    ("state", "absolute"),
                    ("type", "common"),
                ],
                h,
                Reading::Written,
            ),
            row(
                v,
                2,
                0,
                1,
                Some((0, 6)),
                "בָּרָ֣א",
                lemma_two,
                "verb",
                "Vqp3ms",
                &[
                    ("person", "third"),
                    ("gender", "masculine"),
                    ("number", "singular"),
                    ("stem", "qal"),
                    ("type", "qatal"),
                ],
                h,
                Reading::Written,
            ),
            row(
                v,
                3,
                0,
                1,
                None,
                "הַיְצֵ֣א",
                "יָצָא",
                "verb",
                "Vhv2ms",
                &[
                    ("person", "second"),
                    ("stem", "hiphil"),
                    ("type", "imperative"),
                ],
                h,
                Reading::Qere,
            ),
            row(
                v,
                0,
                0,
                1,
                None,
                "אֵלַ֖י",
                "אֶל",
                "preposition",
                "R",
                &[],
                h,
                Reading::QereWithoutKetiv,
            ),
        ] {
            w.add(&r).unwrap();
        }
        w.absent(v, 4, "ketiv_not_read");
        w.finish().unwrap();
        path
    }

    fn greek_layer() -> PathBuf {
        let path = temp("greek");
        let mut w = LayerWriter::create(
            &path,
            &info("greek", "sblgnt"),
            "org",
            &[
                "person", "number", "gender", "case", "tense", "voice", "mood",
            ],
        )
        .unwrap();
        let g = Language::Greek;
        for (verse, word, surface, morph, parsing) in [
            (
                VerseId::new(43, 1, 1),
                3,
                "λόγος",
                "N-NSM",
                vec![
                    ("case", "nominative"),
                    ("number", "singular"),
                    ("gender", "masculine"),
                ],
            ),
            (
                VerseId::new(43, 1, 14),
                2,
                "λόγος",
                "N-NSM",
                vec![
                    ("case", "nominative"),
                    ("number", "singular"),
                    ("gender", "masculine"),
                ],
            ),
            (
                VerseId::new(43, 2, 22),
                9,
                "λόγῳ",
                "N-DSM",
                vec![
                    ("case", "dative"),
                    ("number", "singular"),
                    ("gender", "masculine"),
                ],
            ),
            (
                VerseId::new(43, 3, 16),
                4,
                "ἠγάπησεν",
                "V-AAI-3S",
                vec![
                    ("tense", "aorist"),
                    ("voice", "active"),
                    ("mood", "indicative"),
                    ("person", "third"),
                    ("number", "singular"),
                ],
            ),
        ] {
            let lemma = if surface.starts_with('λ') {
                "λόγος"
            } else {
                "ἀγαπάω"
            };
            let pos = if surface.starts_with('λ') {
                "noun"
            } else {
                "verb"
            };
            w.add(&row(
                verse,
                word,
                0,
                1,
                Some((0, surface.chars().count() as u32)),
                surface,
                lemma,
                pos,
                morph,
                &parsing,
                g,
                Reading::Written,
            ))
            .unwrap();
        }
        w.finish().unwrap();
        path
    }

    #[test]
    fn a_hebrew_token_shows_its_morphemes_with_ids_spans_and_attribution() {
        let path = hebrew_layer("hebrew-a", "בָּרָא");
        let mut layers = MorphologyLayers::new().unwrap();
        assert_eq!(layers.attach_file(&path).unwrap(), "hebrew-a");
        let a = layers.analyze("wlc:01001001001").unwrap();
        assert_eq!(a.status, AnalysisStatus::Analyzed);
        let analysis = &a.analyses[0];
        assert_eq!(analysis.attribution, "Attribution of hebrew-a");
        assert_eq!(analysis.reading, Reading::Written);
        let ids: Vec<_> = analysis
            .morphemes
            .iter()
            .map(|m| m.id.clone().unwrap())
            .collect();
        assert_eq!(ids, ["wlc:01001001001a", "wlc:01001001001b"]);
        assert_eq!(analysis.morphemes[1].lemma, "רֵאשִׁית");
        assert_eq!(
            (analysis.morphemes[1].start, analysis.morphemes[1].end),
            (Some(2), Some(11))
        );
        assert_eq!(
            analysis.morphemes[1].description,
            "noun (common) · feminine singular absolute"
        );
        assert_eq!(analysis.text, "בְּרֵאשִׁ֖ית");
        let verb = layers.analyze("wlc:01001001002").unwrap();
        assert_eq!(
            verb.analyses[0].morphemes[0].id.as_deref(),
            Some("wlc:01001001002")
        );
        assert_eq!(
            verb.analyses[0].morphemes[0].description,
            "verb · qal qatal · third person masculine singular"
        );
        // A morpheme id answers for its whole token.
        assert_eq!(
            layers.analyze("wlc:01001001001b").unwrap().token_id,
            "wlc:01001001001"
        );
    }

    #[test]
    fn ketiv_qere_and_absence_are_explicit() {
        let path = hebrew_layer("hebrew-b", "בָּרָא");
        let mut layers = MorphologyLayers::new().unwrap();
        layers.attach_file(&path).unwrap();
        let qere = layers.analyze("wlc:01001001003").unwrap();
        assert_eq!(qere.analyses[0].reading, Reading::Qere);
        assert_eq!(qere.analyses[0].text, "הַיְצֵ֣א");
        assert_eq!(qere.analyses[0].morphemes[0].start, None);
        let not_read = layers.analyze("wlc:01001001004").unwrap();
        assert_eq!(not_read.status, AnalysisStatus::Absent);
        assert_eq!(not_read.absent_reason.as_deref(), Some("ketiv_not_read"));
        let missing = layers.analyze("wlc:01001001009").unwrap();
        assert_eq!(missing.absent_reason.as_deref(), Some("not_analyzed"));
        let other = layers.analyze("bsb:43003016001").unwrap();
        assert_eq!(other.absent_reason.as_deref(), Some("no_layer"));
        assert!(layers.analyze("not a token").is_err());
        // The qere with no written word is found by lemma search, without a token.
        let found = layers.lemma_search(&[], "אל", 10).unwrap();
        assert_eq!(found.hits.len(), 1);
        assert_eq!(found.hits[0].token_id, None);
        assert_eq!(found.hits[0].reading, Reading::QereWithoutKetiv);
    }

    #[test]
    fn two_layers_that_disagree_are_conflicting_and_both_are_kept() {
        let one = hebrew_layer("hebrew-c", "בָּרָא");
        let two = hebrew_layer("hebrew-d", "בָּרָה");
        let mut layers = MorphologyLayers::new().unwrap();
        layers.attach_file(&one).unwrap();
        layers.attach_file(&two).unwrap();
        let agree = layers.analyze("wlc:01001001001").unwrap();
        assert_eq!(agree.status, AnalysisStatus::Analyzed);
        assert_eq!(agree.analyses.len(), 2);
        let disagree = layers.analyze("wlc:01001001002").unwrap();
        assert_eq!(disagree.status, AnalysisStatus::Conflicting);
        let lemmas: Vec<_> = disagree
            .analyses
            .iter()
            .map(|a| a.morphemes[0].lemma.as_str())
            .collect();
        assert_eq!(lemmas, ["בָּרָא", "בָּרָה"]);
        assert!(layers.attach_file(&one).is_err(), "the same layer twice");
    }

    #[test]
    fn lemma_search_finds_a_prefixed_word_and_every_inflected_form() {
        let mut layers = MorphologyLayers::new().unwrap();
        layers
            .attach_file(&hebrew_layer("hebrew-e", "בָּרָא"))
            .unwrap();
        layers.attach_file(&greek_layer()).unwrap();
        let found = layers
            .lemma_search(&["wlc".to_owned()], "ראשית", 10)
            .unwrap();
        assert_eq!(found.matched_by, "key");
        assert_eq!(found.total, 1);
        assert_eq!(found.hits[0].token_id.as_deref(), Some("wlc:01001001001"));
        assert_eq!(
            found.hits[0].morpheme_id.as_deref(),
            Some("wlc:01001001001b")
        );
        assert_eq!(found.hits[0].reference, "Gen.1.1");
        let greek = layers.lemma_search(&[], "λογος", 10).unwrap();
        assert_eq!(greek.total, 3);
        assert_eq!(greek.lemmas.len(), 1);
        assert_eq!(greek.lemmas[0].lemma, "λόγος");
        let surfaces: Vec<_> = greek.hits.iter().map(|h| h.surface.as_str()).collect();
        assert_eq!(surfaces, ["λόγος", "λόγος", "λόγῳ"]);
        let exact = layers.lemma_search(&[], "λόγος", 2).unwrap();
        assert_eq!(
            (exact.matched_by.as_str(), exact.total, exact.hits.len()),
            ("exact", 3, 2)
        );
        let by_token = layers.lemma_search_token("wlc:01001001001", 10).unwrap();
        assert_eq!(by_token.lemmas[0].lemma, "רֵאשִׁית");
        let none = layers
            .lemma_search(&["bsb".to_owned()], "λογος", 10)
            .unwrap();
        assert_eq!(
            (none.total, none.unavailable.clone()),
            (0, vec!["bsb".to_owned()])
        );
        assert_eq!(
            layers.analyze("sblgnt:43003016004").unwrap().analyses[0].morphemes[0].description,
            "verb · aorist active indicative · third person singular"
        );
    }

    #[test]
    fn a_layer_attaches_from_bytes_and_bad_bytes_are_refused() {
        let path = greek_layer();
        let bytes = std::fs::read(&path).unwrap();
        let mut layers = MorphologyLayers::new().unwrap();
        assert!(layers.attach_bytes(b"not sqlite").is_err());
        layers.attach_bytes(&bytes).unwrap();
        assert_eq!(layers.editions(), ["sblgnt"]);
        assert_eq!(layers.layers()[0].excluded_columns, ["ln"]);
        assert_eq!(layers.lemma_search(&[], "αγαπαω", 5).unwrap().total, 1);
    }

    #[test]
    fn the_b064_engine_answers_slot_queries_over_a_layer() {
        let path = greek_layer();
        let mut engine = crate::Engine::new().unwrap();
        engine.attach_pack(&path).unwrap();
        use openlogos_retrieval::{FeaturePredicate, TokenEngine, TokenPattern, TokenQuery, Unit};
        let hits = engine
            .token_search(&TokenQuery {
                edition: "sblgnt".into(),
                slots: vec![TokenPattern {
                    predicates: vec![
                        FeaturePredicate {
                            feature: "lemma".into(),
                            value: "λόγος".into(),
                        },
                        FeaturePredicate {
                            feature: "case".into(),
                            value: "nominative".into(),
                        },
                    ],
                }],
                scope: Unit::Verse,
                constraints: vec![],
                book: None,
                limit: 10,
            })
            .unwrap();
        assert_eq!(hits.count, 2);
    }

    #[test]
    fn descriptions_read_like_a_grammar() {
        let f = |pairs: &[(&str, &str)]| -> Vec<MorphFeature> {
            pairs
                .iter()
                .map(|(n, v)| MorphFeature {
                    name: (*n).into(),
                    value: (*v).into(),
                })
                .collect()
        };
        assert_eq!(
            describe(
                Language::Greek,
                "noun",
                &f(&[
                    ("type", "proper"),
                    ("case", "genitive"),
                    ("number", "singular"),
                    ("gender", "masculine")
                ])
            ),
            "noun (proper) · genitive singular masculine"
        );
        assert_eq!(
            describe(
                Language::Greek,
                "verb",
                &f(&[
                    ("tense", "present"),
                    ("voice", "active"),
                    ("mood", "participle"),
                    ("case", "nominative"),
                    ("number", "plural"),
                    ("gender", "masculine")
                ])
            ),
            "verb · present active participle · nominative plural masculine"
        );
        assert_eq!(
            describe(
                Language::Aramaic,
                "suffix",
                &f(&[
                    ("type", "pronominal"),
                    ("person", "second"),
                    ("gender", "masculine"),
                    ("number", "singular")
                ])
            ),
            "suffix (pronominal) · second person masculine singular"
        );
    }
}
