//! The corpus pack: one read-only SQLite file per edition or work (specs/19, "Package
//! layers"). This module owns the schema and the writer; `engine` reads packs.
//!
//! Tables: `manifest`, `text` (byte-exact verse text) with the external-content FTS5 index
//! `text_fts`, the columnar token layer (`token`, `feature`, `feature_value`, `token_column`,
//! `posting`), containers for unit scoping (`container_kind`, `container`,
//! `token_container`), `verse_map`, the typed `edge` table with (src, rel) and (dst, rel)
//! indexes, and `vector_set` plus one sqlite-vec `vec0` virtual table per vector set.

use std::collections::HashMap;
use std::path::Path;

use openlogos_retrieval::{ProvenanceKind, Quantization, VerseId};
use roaring::RoaringBitmap;
use rusqlite::{Connection, OpenFlags, params};

use crate::fts::register_tokenizer;
use crate::tokenizer::{NORMALIZATION_MODE, feature_key, normalize};
use crate::vectors::install_vec_extension;

/// Schema version written to every pack; readers refuse other versions.
pub const PACK_SCHEMA_VERSION: &str = "b064-1";

/// Error type of the writer; the reader maps SQLite errors into `RetrievalError::Storage`.
pub type PackResult<T> = Result<T, rusqlite::Error>;

/// Identity and provenance of a pack, stored in `manifest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackManifest {
    /// Edition id (prefix of its token ids), or the work id for a non-Bible pack.
    pub edition: String,
    /// Versification scheme of the stored text.
    pub scheme: String,
    /// Where the data came from (repository and commit, or archive hash).
    pub source: String,
    /// SPDX license of the data.
    pub license: String,
    /// Attribution text the license asks for.
    pub attribution: String,
}

/// One token to add: its verse, 1-based word number, byte-exact surface, and one optional
/// value per declared feature, in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRow<'a> {
    /// The verse.
    pub verse: VerseId,
    /// 1-based word number inside the verse.
    pub word: u32,
    /// Byte-exact surface form.
    pub surface: &'a str,
    /// One value per declared feature.
    pub features: Vec<Option<&'a str>>,
}

/// An edge to add; the two locators are `(work, locator)` pairs.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeRow<'a> {
    /// Source work.
    pub src_work: &'a str,
    /// Source locator.
    pub src_locator: &'a str,
    /// Relation type.
    pub rel: &'a str,
    /// Destination work.
    pub dst_work: &'a str,
    /// Destination locator.
    pub dst_locator: &'a str,
    /// Weight.
    pub weight: f32,
    /// Asserting work.
    pub source_work: &'a str,
    /// Method (parser or model version, or the curated source's name).
    pub method: &'a str,
    /// Provenance kind.
    pub provenance: ProvenanceKind,
}

/// Text of the provenance kind as stored in `edge.provenance_kind`.
pub fn provenance_name(kind: ProvenanceKind) -> &'static str {
    match kind {
        ProvenanceKind::Curated => "curated",
        ProvenanceKind::Extracted => "extracted",
        ProvenanceKind::ModelSimilarity => "model_similarity",
        ProvenanceKind::User => "user",
    }
}

/// Parses `edge.provenance_kind`.
pub fn parse_provenance(text: &str) -> Option<ProvenanceKind> {
    Some(match text {
        "curated" => ProvenanceKind::Curated,
        "extracted" => ProvenanceKind::Extracted,
        "model_similarity" => ProvenanceKind::ModelSimilarity,
        "user" => ProvenanceKind::User,
        _ => return None,
    })
}

/// Relation types only a curated or extracted edge may carry (specs/19 item 4).
pub const CURATED_ONLY_RELATIONS: &[&str] = &[
    "quotes",
    "explicitly_cites",
    "cites",
    "cross_reference",
    "alludes_to",
];

const SCHEMA: &str = "
CREATE TABLE manifest(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE text(verse_id INTEGER PRIMARY KEY, body TEXT NOT NULL);
CREATE VIRTUAL TABLE text_fts USING fts5(body, content='text', content_rowid='verse_id', tokenize='openlogos');
CREATE TABLE feature(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
CREATE TABLE feature_value(feature_id INTEGER NOT NULL, value_id INTEGER NOT NULL, value TEXT NOT NULL,
    PRIMARY KEY(feature_id, value_id)) WITHOUT ROWID;
CREATE TABLE token(pos INTEGER PRIMARY KEY, verse_id INTEGER NOT NULL, word INTEGER NOT NULL,
    surface TEXT NOT NULL, key TEXT NOT NULL);
CREATE INDEX token_verse ON token(verse_id);
CREATE TABLE token_column(feature_id INTEGER PRIMARY KEY, value_ids BLOB NOT NULL);
CREATE TABLE posting(feature_id INTEGER NOT NULL, value_id INTEGER NOT NULL, bitmap BLOB NOT NULL,
    PRIMARY KEY(feature_id, value_id)) WITHOUT ROWID;
CREATE TABLE container_kind(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
CREATE TABLE container(id INTEGER PRIMARY KEY, kind_id INTEGER NOT NULL, parent_id INTEGER,
    first_pos INTEGER NOT NULL, last_pos INTEGER NOT NULL);
CREATE TABLE token_container(kind_id INTEGER PRIMARY KEY, container_ids BLOB NOT NULL);
CREATE TABLE verse_map(scheme_from TEXT NOT NULL, ref_from INTEGER NOT NULL, scheme_to TEXT NOT NULL,
    ref_to INTEGER NOT NULL, kind TEXT NOT NULL);
CREATE INDEX verse_map_from ON verse_map(scheme_from, ref_from, scheme_to);
CREATE TABLE edge(src_work TEXT NOT NULL, src_locator TEXT NOT NULL, rel TEXT NOT NULL,
    dst_work TEXT NOT NULL, dst_locator TEXT NOT NULL, weight REAL NOT NULL DEFAULT 0,
    source_work TEXT NOT NULL, method TEXT NOT NULL, provenance_kind TEXT NOT NULL,
    confidence REAL);
CREATE INDEX edge_src ON edge(src_work, src_locator, rel);
CREATE INDEX edge_dst ON edge(dst_work, dst_locator, rel);
CREATE TABLE vector_set(name TEXT PRIMARY KEY, model TEXT NOT NULL, revision TEXT NOT NULL,
    dimensions INTEGER NOT NULL, quantization TEXT NOT NULL, count INTEGER NOT NULL) WITHOUT ROWID;
";

/// Writes a pack. Tokens, containers, edges and vectors are added in any order; `finish`
/// derives the dictionary columns, postings, container maps and FTS index.
pub struct PackBuilder {
    conn: Connection,
    features: Vec<String>,
    /// Per feature: value text → value id (1-based; 0 means absent).
    dictionaries: Vec<HashMap<String, u32>>,
    /// Per feature: value id per token position.
    columns: Vec<Vec<u32>>,
    verses_of_tokens: Vec<u32>,
    container_kinds: Vec<String>,
    /// `(kind index, parent id, first, last)` per container id (1-based ids).
    containers: Vec<(usize, Option<u32>, u32, u32)>,
    /// Exact slot lists of the containers whose slots are not one contiguous range, by
    /// container id − 1; `None` means the container is its hull span.
    container_slots: Vec<Option<Vec<u32>>>,
    vector_dims: HashMap<String, (u32, Quantization)>,
}

impl PackBuilder {
    /// Creates a new pack file (fails if it exists) with the given features in order.
    pub fn create(
        path: &Path,
        manifest: &PackManifest,
        features: &[&str],
    ) -> PackResult<PackBuilder> {
        install_vec_extension();
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        register_tokenizer(&conn)?;
        conn.execute_batch(
            "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA page_size = 4096;",
        )?;
        conn.execute_batch(SCHEMA)?;
        let mut insert = conn.prepare("INSERT INTO manifest(key, value) VALUES (?1, ?2)")?;
        for (key, value) in [
            ("schema_version", PACK_SCHEMA_VERSION),
            ("engine", "openlogos-retrieval-sqlite"),
            ("normalization_mode", NORMALIZATION_MODE),
            ("edition", manifest.edition.as_str()),
            ("scheme", manifest.scheme.as_str()),
            ("source", manifest.source.as_str()),
            ("license", manifest.license.as_str()),
            ("attribution", manifest.attribution.as_str()),
        ] {
            insert.execute(params![key, value])?;
        }
        drop(insert);
        let mut insert = conn.prepare("INSERT INTO feature(id, name) VALUES (?1, ?2)")?;
        for (i, f) in features.iter().enumerate() {
            insert.execute(params![i as i64 + 1, f])?;
        }
        drop(insert);
        conn.execute_batch("BEGIN")?;
        Ok(PackBuilder {
            conn,
            features: features.iter().map(|f| (*f).to_owned()).collect(),
            dictionaries: vec![HashMap::new(); features.len()],
            columns: vec![Vec::new(); features.len()],
            verses_of_tokens: Vec::new(),
            container_kinds: Vec::new(),
            containers: Vec::new(),
            container_slots: Vec::new(),
            vector_dims: HashMap::new(),
        })
    }

    /// Adds the display text of one verse.
    pub fn add_verse(&mut self, verse: VerseId, body: &str) -> PackResult<()> {
        self.conn.execute(
            "INSERT INTO text(verse_id, body) VALUES (?1, ?2)",
            params![i64::from(verse.0), body],
        )?;
        Ok(())
    }

    /// Adds one token; returns its position. Tokens must arrive in reading order.
    pub fn add_token(&mut self, row: &TokenRow<'_>) -> PackResult<u32> {
        assert_eq!(
            row.features.len(),
            self.features.len(),
            "one value per declared feature"
        );
        let pos = self.verses_of_tokens.len() as u32;
        self.verses_of_tokens.push(row.verse.0);
        for (i, value) in row.features.iter().enumerate() {
            let id = match value {
                Some(v) if !v.is_empty() => {
                    let next = self.dictionaries[i].len() as u32 + 1;
                    *self.dictionaries[i].entry(feature_key(v)).or_insert(next)
                }
                _ => 0,
            };
            self.columns[i].push(id);
        }
        self.conn.execute(
            "INSERT INTO token(pos, verse_id, word, surface, key) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                i64::from(pos),
                i64::from(row.verse.0),
                i64::from(row.word),
                row.surface,
                normalize(row.surface)
            ],
        )?;
        Ok(pos)
    }

    /// Declares a container kind (`sentence`, `clause`, `phrase`, ...); returns its index.
    pub fn add_container_kind(&mut self, name: &str) -> usize {
        if let Some(i) = self.container_kinds.iter().position(|k| k == name) {
            return i;
        }
        self.container_kinds.push(name.to_owned());
        self.container_kinds.len() - 1
    }

    /// Adds a container spanning `first..=last` token positions; returns its id (1-based).
    pub fn add_container(
        &mut self,
        kind: usize,
        parent: Option<u32>,
        first: u32,
        last: u32,
    ) -> u32 {
        self.containers.push((kind, parent, first, last));
        self.container_slots.push(None);
        self.containers.len() as u32
    }

    /// Adds a discontinuous container: `slots` are its exact token positions, ascending. The
    /// `container` row stores the hull span (used by ANCESTOR); the innermost-container maps
    /// (used by PEER, XPEER and unit scopes) use the exact slots.
    pub fn add_container_with_slots(
        &mut self,
        kind: usize,
        parent: Option<u32>,
        slots: &[u32],
    ) -> u32 {
        let first = slots.first().copied().unwrap_or(0);
        let last = slots.last().copied().unwrap_or(0);
        self.containers.push((kind, parent, first, last));
        self.container_slots.push(Some(slots.to_vec()));
        self.containers.len() as u32
    }

    /// Adds one `verse_map` row.
    pub fn add_verse_map(
        &mut self,
        from_scheme: &str,
        from: VerseId,
        to_scheme: &str,
        to: VerseId,
        kind: &str,
    ) -> PackResult<()> {
        self.conn.execute(
            "INSERT INTO verse_map(scheme_from, ref_from, scheme_to, ref_to, kind) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![from_scheme, i64::from(from.0), to_scheme, i64::from(to.0), kind],
        )?;
        Ok(())
    }

    /// Adds one edge. A `model_similarity` edge with a curated relation type is refused.
    pub fn add_edge(&mut self, edge: &EdgeRow<'_>) -> PackResult<()> {
        if edge.provenance == ProvenanceKind::ModelSimilarity
            && CURATED_ONLY_RELATIONS.contains(&edge.rel)
        {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "model_similarity edges may not carry the curated relation `{}`",
                edge.rel
            )));
        }
        self.conn.execute(
            "INSERT INTO edge(src_work, src_locator, rel, dst_work, dst_locator, weight, source_work, method, provenance_kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                edge.src_work,
                edge.src_locator,
                edge.rel,
                edge.dst_work,
                edge.dst_locator,
                f64::from(edge.weight),
                edge.source_work,
                edge.method,
                provenance_name(edge.provenance)
            ],
        )?;
        Ok(())
    }

    /// Declares a vector set and creates its `vec0` table.
    pub fn add_vector_set(
        &mut self,
        name: &str,
        model: &str,
        revision: &str,
        dimensions: u32,
        quantization: Quantization,
    ) -> PackResult<()> {
        assert!(
            name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "vector set names are identifiers"
        );
        let kind = match quantization {
            Quantization::Int8 => "int8",
            Quantization::Binary => "bit",
        };
        self.conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE vec_{name} USING vec0(embedding {kind}[{dimensions}]);"
        ))?;
        self.conn.execute(
            "INSERT INTO vector_set(name, model, revision, dimensions, quantization, count) VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            params![name, model, revision, i64::from(dimensions), kind],
        )?;
        self.vector_dims
            .insert(name.to_owned(), (dimensions, quantization));
        Ok(())
    }

    /// Adds one vector: `dimensions` bytes for int8, `dimensions / 8` bytes for binary.
    pub fn add_vector(&mut self, set: &str, id: i64, bytes: &[u8]) -> PackResult<()> {
        let (dims, quant) = self.vector_dims[set];
        let expected = match quant {
            Quantization::Int8 => dims as usize,
            Quantization::Binary => dims as usize / 8,
        };
        assert_eq!(bytes.len(), expected, "vector byte length for set {set}");
        let wrap = match quant {
            Quantization::Int8 => "vec_int8",
            Quantization::Binary => "vec_bit",
        };
        self.conn.execute(
            &format!("INSERT INTO vec_{set}(rowid, embedding) VALUES (?1, {wrap}(?2))"),
            params![id, bytes],
        )?;
        self.conn.execute(
            "UPDATE vector_set SET count = count + 1 WHERE name = ?1",
            params![set],
        )?;
        Ok(())
    }

    /// Adds many vectors inside one statement loop; same contract as `add_vector`.
    pub fn add_vectors<'a>(
        &mut self,
        set: &str,
        rows: impl Iterator<Item = (i64, &'a [u8])>,
    ) -> PackResult<u64> {
        let (dims, quant) = self.vector_dims[set];
        let expected = match quant {
            Quantization::Int8 => dims as usize,
            Quantization::Binary => dims as usize / 8,
        };
        let wrap = match quant {
            Quantization::Int8 => "vec_int8",
            Quantization::Binary => "vec_bit",
        };
        let mut stmt = self.conn.prepare(&format!(
            "INSERT INTO vec_{set}(rowid, embedding) VALUES (?1, {wrap}(?2))"
        ))?;
        let mut n = 0u64;
        for (id, bytes) in rows {
            assert_eq!(bytes.len(), expected, "vector byte length for set {set}");
            stmt.execute(params![id, bytes])?;
            n += 1;
        }
        drop(stmt);
        self.conn.execute(
            "UPDATE vector_set SET count = count + ?2 WHERE name = ?1",
            params![set, n as i64],
        )?;
        Ok(n)
    }

    /// Derives the indexes, commits and closes the file.
    pub fn finish(self) -> PackResult<()> {
        let PackBuilder {
            conn,
            features,
            dictionaries,
            columns,
            verses_of_tokens,
            container_kinds,
            containers,
            container_slots,
            ..
        } = self;
        let n_tokens = verses_of_tokens.len() as u32;

        // Dictionaries, dense columns and postings.
        {
            let mut insert_value = conn.prepare(
                "INSERT INTO feature_value(feature_id, value_id, value) VALUES (?1, ?2, ?3)",
            )?;
            let mut insert_column =
                conn.prepare("INSERT INTO token_column(feature_id, value_ids) VALUES (?1, ?2)")?;
            let mut insert_posting = conn
                .prepare("INSERT INTO posting(feature_id, value_id, bitmap) VALUES (?1, ?2, ?3)")?;
            for (i, _name) in features.iter().enumerate() {
                let feature_id = i as i64 + 1;
                let mut values: Vec<(&String, &u32)> = dictionaries[i].iter().collect();
                values.sort_by_key(|(_, id)| **id);
                for (value, id) in &values {
                    insert_value.execute(params![feature_id, i64::from(**id), value])?;
                }
                let mut bytes = Vec::with_capacity(columns[i].len() * 4);
                for id in &columns[i] {
                    bytes.extend_from_slice(&id.to_le_bytes());
                }
                insert_column.execute(params![feature_id, bytes])?;
                let mut bitmaps: Vec<RoaringBitmap> =
                    vec![RoaringBitmap::new(); dictionaries[i].len() + 1];
                for (pos, id) in columns[i].iter().enumerate() {
                    if *id != 0 {
                        bitmaps[*id as usize].insert(pos as u32);
                    }
                }
                for (id, bitmap) in bitmaps.iter().enumerate().skip(1) {
                    let mut buf = Vec::with_capacity(bitmap.serialized_size());
                    bitmap
                        .serialize_into(&mut buf)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                    insert_posting.execute(params![feature_id, id as i64, buf])?;
                }
            }
        }

        // Containers: rows, then the innermost container of each kind per token.
        {
            let mut insert_kind =
                conn.prepare("INSERT INTO container_kind(id, name) VALUES (?1, ?2)")?;
            for (i, k) in container_kinds.iter().enumerate() {
                insert_kind.execute(params![i as i64 + 1, k])?;
            }
            let mut insert_container = conn.prepare(
                "INSERT INTO container(id, kind_id, parent_id, first_pos, last_pos) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (id, (kind, parent, first, last)) in containers.iter().enumerate() {
                insert_container.execute(params![
                    id as i64 + 1,
                    *kind as i64 + 1,
                    parent.map(i64::from),
                    i64::from(*first),
                    i64::from(*last)
                ])?;
            }
            let mut insert_map = conn
                .prepare("INSERT INTO token_container(kind_id, container_ids) VALUES (?1, ?2)")?;
            for kind in 0..container_kinds.len() {
                // Largest containers first so smaller (inner) ones overwrite them; size is the
                // exact slot count when known, the hull length otherwise.
                let mut of_kind: Vec<(u32, u32, u32, usize)> = containers
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.0 == kind)
                    .map(|(id, c)| {
                        let size = container_slots[id]
                            .as_ref()
                            .map_or((c.3 - c.2 + 1) as usize, Vec::len);
                        (id as u32 + 1, c.2, c.3, size)
                    })
                    .collect();
                of_kind.sort_by_key(|(_, _, _, size)| std::cmp::Reverse(*size));
                let mut innermost = vec![0u32; n_tokens as usize];
                for (id, first, last, _) in of_kind {
                    match &container_slots[id as usize - 1] {
                        Some(slots) => {
                            for s in slots {
                                innermost[*s as usize] = id;
                            }
                        }
                        None => {
                            for slot in innermost
                                .iter_mut()
                                .take(last as usize + 1)
                                .skip(first as usize)
                            {
                                *slot = id;
                            }
                        }
                    }
                }
                let mut bytes = Vec::with_capacity(innermost.len() * 4);
                for id in innermost {
                    bytes.extend_from_slice(&id.to_le_bytes());
                }
                insert_map.execute(params![kind as i64 + 1, bytes])?;
            }
        }

        conn.execute_batch("INSERT INTO text_fts(text_fts) VALUES ('rebuild'); COMMIT; VACUUM;")?;
        conn.close().map_err(|(_, e)| e)
    }
}

/// Opens an existing pack file read-only for inspection (tests and the bench harness).
pub fn open_read_only(path: &Path) -> PackResult<Connection> {
    install_vec_extension();
    let uri = format!("file:{}?immutable=1", path.display());
    let conn = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    register_tokenizer(&conn)?;
    Ok(conn)
}

#[cfg(test)]
pub(crate) mod fixture {
    //! A tiny Greek pack for the crate's tests: John 1:1–2 and 1 John 4:8 from the SBLGNT
    //! (Society of Biblical Literature / Logos, CC BY 4.0), lemmas and morphology written by
    //! hand after the MACULA conventions. Positions 0..21 are John 1:1–2 (17 + 5 tokens),
    //! 22..29 are 1 John 4:8.

    use super::*;

    pub const FEATURES: &[&str] = &[
        "lemma", "sp", "case", "number", "gender", "tense", "voice", "mood", "person",
    ];

    /// `(surface, lemma, sp, case, number, gender, tense, voice, mood, person)`.
    type Row = (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
    );

    pub const JOHN_1_1: &str =
        "Ἐν ἀρχῇ ἦν ὁ λόγος, καὶ ὁ λόγος ἦν πρὸς τὸν θεόν, καὶ θεὸς ἦν ὁ λόγος.";
    pub const JOHN_1_2: &str = "οὗτος ἦν ἐν ἀρχῇ πρὸς τὸν θεόν.";
    pub const FIRST_JOHN_4_8: &str = "ὁ μὴ ἀγαπῶν οὐκ ἔγνω τὸν θεόν, ὅτι ὁ θεὸς ἀγάπη ἐστίν.";

    const ROWS: &[(VerseId, &[Row])] = &[
        (
            VerseId(44_001_001),
            &[
                ("Ἐν", "ἐν", "prep", "", "", "", "", "", "", ""),
                (
                    "ἀρχῇ",
                    "ἀρχή",
                    "noun",
                    "dative",
                    "singular",
                    "feminine",
                    "",
                    "",
                    "",
                    "",
                ),
                (
                    "ἦν",
                    "εἰμί",
                    "verb",
                    "",
                    "singular",
                    "",
                    "imperfect",
                    "active",
                    "indicative",
                    "third",
                ),
                (
                    "ὁ",
                    "ὁ",
                    "det",
                    "nominative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                (
                    "λόγος",
                    "λόγος",
                    "noun",
                    "nominative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                ("καὶ", "καί", "conj", "", "", "", "", "", "", ""),
                (
                    "ὁ",
                    "ὁ",
                    "det",
                    "nominative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                (
                    "λόγος",
                    "λόγος",
                    "noun",
                    "nominative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                (
                    "ἦν",
                    "εἰμί",
                    "verb",
                    "",
                    "singular",
                    "",
                    "imperfect",
                    "active",
                    "indicative",
                    "third",
                ),
                ("πρὸς", "πρός", "prep", "", "", "", "", "", "", ""),
                (
                    "τὸν",
                    "ὁ",
                    "det",
                    "accusative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                (
                    "θεόν",
                    "θεός",
                    "noun",
                    "accusative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                ("καὶ", "καί", "conj", "", "", "", "", "", "", ""),
                (
                    "θεὸς",
                    "θεός",
                    "noun",
                    "nominative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                (
                    "ἦν",
                    "εἰμί",
                    "verb",
                    "",
                    "singular",
                    "",
                    "imperfect",
                    "active",
                    "indicative",
                    "third",
                ),
                (
                    "ὁ",
                    "ὁ",
                    "det",
                    "nominative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                (
                    "λόγος",
                    "λόγος",
                    "noun",
                    "nominative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
            ],
        ),
        (
            VerseId(44_001_002),
            &[
                (
                    "οὗτος",
                    "οὗτος",
                    "pron",
                    "nominative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                (
                    "ἦν",
                    "εἰμί",
                    "verb",
                    "",
                    "singular",
                    "",
                    "imperfect",
                    "active",
                    "indicative",
                    "third",
                ),
                ("ἐν", "ἐν", "prep", "", "", "", "", "", "", ""),
                (
                    "ἀρχῇ",
                    "ἀρχή",
                    "noun",
                    "dative",
                    "singular",
                    "feminine",
                    "",
                    "",
                    "",
                    "",
                ),
                ("πρὸς", "πρός", "prep", "", "", "", "", "", "", ""),
                (
                    "τὸν",
                    "ὁ",
                    "det",
                    "accusative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                (
                    "θεόν",
                    "θεός",
                    "noun",
                    "accusative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
            ],
        ),
        (
            VerseId(63_004_008),
            &[
                (
                    "ὁ",
                    "ὁ",
                    "det",
                    "nominative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                ("μὴ", "μή", "adv", "", "", "", "", "", "", ""),
                (
                    "ἀγαπῶν",
                    "ἀγαπάω",
                    "verb",
                    "nominative",
                    "singular",
                    "masculine",
                    "present",
                    "active",
                    "participle",
                    "",
                ),
                ("οὐκ", "οὐ", "adv", "", "", "", "", "", "", ""),
                (
                    "ἔγνω",
                    "γινώσκω",
                    "verb",
                    "",
                    "singular",
                    "",
                    "aorist",
                    "active",
                    "indicative",
                    "third",
                ),
                (
                    "τὸν",
                    "ὁ",
                    "det",
                    "accusative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                (
                    "θεόν",
                    "θεός",
                    "noun",
                    "accusative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                ("ὅτι", "ὅτι", "conj", "", "", "", "", "", "", ""),
                (
                    "ὁ",
                    "ὁ",
                    "det",
                    "nominative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                (
                    "θεὸς",
                    "θεός",
                    "noun",
                    "nominative",
                    "singular",
                    "masculine",
                    "",
                    "",
                    "",
                    "",
                ),
                (
                    "ἀγάπη",
                    "ἀγάπη",
                    "noun",
                    "nominative",
                    "singular",
                    "feminine",
                    "",
                    "",
                    "",
                    "",
                ),
                (
                    "ἐστίν",
                    "εἰμί",
                    "verb",
                    "",
                    "singular",
                    "",
                    "present",
                    "active",
                    "indicative",
                    "third",
                ),
            ],
        ),
    ];

    pub fn manifest() -> PackManifest {
        PackManifest {
            edition: "sblgnt-test".into(),
            scheme: "org".into(),
            source: "SBLGNT John 1:1-2, 1 John 4:8, typed for this test".into(),
            license: "CC-BY-4.0".into(),
            attribution:
                "SBL Greek New Testament, Society of Biblical Literature and Logos Bible Software"
                    .into(),
        }
    }

    /// Builds the fixture pack at `path`. Containers: one `sentence` per verse (John 1:1 is
    /// one sentence of three clauses; 1 John 4:8 is one sentence of two clauses), `clause`
    /// containers inside, and in John 1:1 a nested `clause` for `πρὸς τὸν θεόν` to exercise
    /// ANCESTOR versus PEER.
    pub fn build(path: &Path) -> PackResult<()> {
        let mut b = PackBuilder::create(path, &manifest(), FEATURES)?;
        b.add_verse(VerseId(44_001_001), JOHN_1_1)?;
        b.add_verse(VerseId(44_001_002), JOHN_1_2)?;
        b.add_verse(VerseId(63_004_008), FIRST_JOHN_4_8)?;
        for (verse, rows) in ROWS {
            for (i, r) in rows.iter().enumerate() {
                let values = [r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8, r.9];
                b.add_token(&TokenRow {
                    verse: *verse,
                    word: i as u32 + 1,
                    surface: r.0,
                    features: values
                        .iter()
                        .map(|v| if v.is_empty() { None } else { Some(*v) })
                        .collect(),
                })?;
            }
        }
        let sentence = b.add_container_kind("sentence");
        let clause = b.add_container_kind("clause");
        // John 1:1: tokens 0..=16; clauses 0..=4, 5..=11 (with nested 9..=11), 12..=16.
        let s1 = b.add_container(sentence, None, 0, 16);
        b.add_container(clause, Some(s1), 0, 4);
        let c2 = b.add_container(clause, Some(s1), 5, 11);
        b.add_container(clause, Some(c2), 9, 11);
        b.add_container(clause, Some(s1), 12, 16);
        // John 1:2: tokens 17..=23, one clause.
        let s2 = b.add_container(sentence, None, 17, 23);
        b.add_container(clause, Some(s2), 17, 23);
        // 1 John 4:8: tokens 24..=35; clauses 24..=30 and 31..=35.
        let s3 = b.add_container(sentence, None, 24, 35);
        b.add_container(clause, Some(s3), 24, 30);
        b.add_container(clause, Some(s3), 31, 35);
        b.add_verse_map(
            "org",
            VerseId(44_001_001),
            "eng",
            VerseId(44_001_001),
            "equal",
        )?;
        b.add_edge(&EdgeRow {
            src_work: "sblgnt-test",
            src_locator: "63004008",
            rel: "cross_reference",
            dst_work: "sblgnt-test",
            dst_locator: "44001001",
            weight: 3.0,
            source_work: "sblgnt-test",
            method: "typed",
            provenance: ProvenanceKind::Curated,
        })?;
        b.finish()
    }

    /// A fresh temporary path for one test; unique even when one helper serves many tests.
    pub fn temp_path(name: &str) -> std::path::PathBuf {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "openlogos-b064-{}-{}-{}",
            std::process::id(),
            name,
            n
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("pack.sqlite")
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::{build, temp_path};
    use super::*;

    #[test]
    fn a_pack_round_trips_text_tokens_postings_and_edges() {
        let path = temp_path("pack-roundtrip");
        build(&path).unwrap();
        let conn = open_read_only(&path).unwrap();
        let version: String = conn
            .query_row(
                "SELECT value FROM manifest WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(version, PACK_SCHEMA_VERSION);
        let verses: i64 = conn
            .query_row("SELECT count(*) FROM text", [], |r| r.get(0))
            .unwrap();
        let tokens: i64 = conn
            .query_row("SELECT count(*) FROM token", [], |r| r.get(0))
            .unwrap();
        assert_eq!((verses, tokens), (3, 36));
        // Byte-exact text survives.
        let body: String = conn
            .query_row("SELECT body FROM text WHERE verse_id = 44001001", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(body, fixture::JOHN_1_1);
        // The posting for lemma=θεός has five positions.
        let blob: Vec<u8> = conn
            .query_row(
                "SELECT bitmap FROM posting WHERE feature_id = 1 AND value_id =
                 (SELECT value_id FROM feature_value WHERE feature_id = 1 AND value = 'θεός')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let bitmap = RoaringBitmap::deserialize_from(&blob[..]).unwrap();
        assert_eq!(bitmap.iter().collect::<Vec<_>>(), vec![11, 13, 23, 30, 33]);
        // Dense column: 36 u32 values.
        let col: Vec<u8> = conn
            .query_row(
                "SELECT value_ids FROM token_column WHERE feature_id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(col.len(), 36 * 4);
        // Innermost clause of token 10 (τὸν in πρὸς τὸν θεόν) is the nested clause (id 4).
        let map: Vec<u8> = conn
            .query_row(
                "SELECT container_ids FROM token_container WHERE kind_id = 2",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let id = u32::from_le_bytes(map[40..44].try_into().unwrap());
        assert_eq!(id, 4);
        let edges: i64 = conn
            .query_row("SELECT count(*) FROM edge", [], |r| r.get(0))
            .unwrap();
        assert_eq!(edges, 1);
        // The FTS index was rebuilt from the external content.
        let hits: i64 = conn
            .query_row(
                "SELECT count(*) FROM text_fts WHERE text_fts MATCH 'λογος'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1);
    }

    #[test]
    fn model_similarity_cannot_carry_a_curated_relation() {
        let path = temp_path("pack-provenance");
        let mut b = PackBuilder::create(&path, &fixture::manifest(), &[]).unwrap();
        let err = b
            .add_edge(&EdgeRow {
                src_work: "a",
                src_locator: "1",
                rel: "quotes",
                dst_work: "b",
                dst_locator: "2",
                weight: 0.9,
                source_work: "model",
                method: "e5-small@1",
                provenance: ProvenanceKind::ModelSimilarity,
            })
            .unwrap_err();
        assert!(err.to_string().contains("model_similarity"));
        b.add_edge(&EdgeRow {
            src_work: "a",
            src_locator: "1",
            rel: "similar_to",
            dst_work: "b",
            dst_locator: "2",
            weight: 0.9,
            source_work: "model",
            method: "e5-small@1",
            provenance: ProvenanceKind::ModelSimilarity,
        })
        .unwrap();
        b.finish().unwrap();
    }

    #[test]
    fn vectors_are_stored_and_counted() {
        let path = temp_path("pack-vectors");
        let mut b = PackBuilder::create(&path, &fixture::manifest(), &[]).unwrap();
        b.add_vector_set("verses_i8", "test-model", "r1", 8, Quantization::Int8)
            .unwrap();
        b.add_vector("verses_i8", 1, &[1, 2, 3, 4, 5, 6, 7, 8])
            .unwrap();
        b.add_vector("verses_i8", 2, &[0xFF, 0xFE, 3, 4, 5, 6, 7, 8])
            .unwrap();
        b.add_vector_set("verses_bit", "test-model", "r1", 16, Quantization::Binary)
            .unwrap();
        b.add_vector("verses_bit", 1, &[0b1010_1010, 0x00]).unwrap();
        b.finish().unwrap();
        let conn = open_read_only(&path).unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT count FROM vector_set WHERE name = 'verses_i8'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2);
        let nearest: i64 = conn
            .query_row(
                "SELECT rowid FROM vec_verses_i8 WHERE embedding MATCH vec_int8(?1) AND k = 1",
                [&[1u8, 2, 3, 4, 5, 6, 7, 8][..]],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(nearest, 1);
    }
}
