//! Text search over the installed editions (B071, specs/22 §7): what the palette's Text section
//! asks on every keystroke. One connection with one B064 search pack attached per edition, from
//! files natively or from bytes in a browser, answered through the shared `openlogos` tokenizer.
//!
//! The field's text is the reader's, not a query language: it is split by the shared normalizer
//! and every word is quoted, so nothing typed can be an FTS5 syntax error. The last word is a
//! prefix while it is still being typed (no space after it). Hits run in two tiers: the words as
//! a phrase first, then all the words anywhere in the verse; inside a tier BM25, then edition and
//! verse order so every host shows the same list.
//!
//! The struct holds a `Connection` and nothing else that is tied to a thread, so a host can keep
//! it behind its own lock; [`crate::Engine`] cannot move between threads.
//!
//! A work pack (B016, [`crate::work_pack`]) attaches the same way. Its rows are sections, not
//! verses: a hit names `scheme:locator` (`institutes:3.2.7`) instead of an OSIS verse, its
//! snippet has the section's paragraph breaks as spaces, and the verse-shaped [`TextIndex`]
//! leaves it out.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use openlogos_core::reference::{BookId, Reference};
use openlogos_retrieval::{
    Result, RetrievalError, TextHit as IndexHit, TextIndex, TextQuery, VerseId,
};
use openlogos_workspaces::palette::{TextHit, TextProvider};
use rusqlite::{Connection, OpenFlags, params};

use crate::fts::register_tokenizer;
use crate::pack::PACK_SCHEMA_VERSION;
use crate::tokenizer::{NORMALIZATION_MODE, Token, normalize, tokenize};

/// Scalars a snippet aims for, as the fixture provider's rows show.
const SNIPPET_SCALARS: usize = 80;

/// Words kept before the first hit in a snippet.
const SNIPPET_WORDS_BEFORE: usize = 3;

/// Text search over attached editions.
pub struct EditionSearch {
    conn: Connection,
    /// `(schema, edition)` in attach order, which is the tie-break order of hits.
    packs: Vec<(String, String)>,
    /// Per pack in `packs`, the locator scheme of a work pack (B016); `None` for an edition.
    works: Vec<Option<String>>,
}

impl std::fmt::Debug for EditionSearch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditionSearch")
            .field("packs", &self.packs)
            .finish_non_exhaustive()
    }
}

fn storage(error: rusqlite::Error) -> RetrievalError {
    RetrievalError::Storage(error.to_string())
}

/// A path as a `file:` URI: the three characters a URI gives meaning to are escaped.
fn file_uri(path: &Path) -> String {
    let escaped = path
        .display()
        .to_string()
        .replace('%', "%25")
        .replace('?', "%3F")
        .replace('#', "%23");
    format!("file:{escaped}?immutable=1")
}

impl EditionSearch {
    /// No editions yet.
    pub fn new() -> Result<EditionSearch> {
        let conn = Connection::open_in_memory_with_flags(
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(storage)?;
        register_tokenizer(&conn).map_err(storage)?;
        Ok(EditionSearch {
            conn,
            packs: Vec::new(),
            works: Vec::new(),
        })
    }

    /// Attaches every `*.sqlite` of a directory, in name order (`bsb`, `sblgnt`, `wlc`).
    pub fn open_directory(directory: &Path) -> Result<EditionSearch> {
        let mut search = EditionSearch::new()?;
        let read = std::fs::read_dir(directory)
            .map_err(|e| RetrievalError::Storage(format!("{}: {e}", directory.display())))?;
        let mut paths: Vec<PathBuf> = read
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "sqlite"))
            .collect();
        paths.sort();
        if paths.is_empty() {
            return Err(RetrievalError::Storage(format!(
                "{} holds no search packs",
                directory.display()
            )));
        }
        for path in paths {
            search.attach_file(&path)?;
        }
        Ok(search)
    }

    fn next_schema(&self) -> String {
        format!("p{}", self.packs.len())
    }

    /// Attaches one search pack from a file, read-only. Returns its edition.
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

    /// Attaches one search pack from its bytes, read-only: what a browser does with a pack it
    /// fetched. The bytes are copied into SQLite's memory.
    pub fn attach_bytes(&mut self, bytes: &[u8]) -> Result<String> {
        // Bytes that are no database would poison every later statement of the connection.
        if !bytes.starts_with(b"SQLite format 3\0") {
            return Err(RetrievalError::Storage(
                "these bytes are not a search pack".to_owned(),
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

    /// Checks the attached pack's manifest and records it, or detaches it again.
    fn adopt(&mut self, schema: String) -> Result<String> {
        let read = || -> Result<(String, Option<String>)> {
            let value = |key: &str| -> Result<String> {
                self.conn
                    .query_row(
                        &format!("SELECT value FROM \"{schema}\".manifest WHERE key = ?1"),
                        params![key],
                        |r| r.get(0),
                    )
                    .map_err(storage)
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
            let edition = value("edition")?;
            if self.packs.iter().any(|(_, e)| *e == edition) {
                return Err(RetrievalError::Storage(format!(
                    "edition `{edition}` is already attached"
                )));
            }
            // B016: a work pack names its format and its locator scheme.
            let work = match value("work_format") {
                Ok(format) if format == crate::work_pack::WORK_FORMAT => {
                    Some(value("locator_scheme")?)
                }
                Ok(format) => {
                    return Err(RetrievalError::Storage(format!(
                        "work format `{format}` is not {}",
                        crate::work_pack::WORK_FORMAT
                    )));
                }
                Err(_) => None,
            };
            Ok((edition, work))
        };
        match read() {
            Ok((edition, work)) => {
                self.packs.push((schema, edition.clone()));
                self.works.push(work);
                Ok(edition)
            }
            Err(error) => {
                let _ = self
                    .conn
                    .execute_batch(&format!("DETACH DATABASE \"{schema}\""));
                Err(error)
            }
        }
    }

    /// The editions attached, in order.
    #[must_use]
    pub fn editions(&self) -> Vec<String> {
        self.packs.iter().map(|(_, e)| e.clone()).collect()
    }

    /// One tier: every pack's best `limit` rows for an FTS5 expression, best first.
    fn run_tier(&self, expression: &str, limit: usize) -> Result<Vec<(f64, usize, i64)>> {
        let mut rows = Vec::new();
        for (index, (schema, _)) in self.packs.iter().enumerate() {
            let mut stmt = self
                .conn
                .prepare_cached(&format!(
                    "SELECT rowid, bm25(text_fts) FROM \"{schema}\".text_fts
                     WHERE text_fts MATCH ?1 ORDER BY bm25(text_fts), rowid LIMIT ?2"
                ))
                .map_err(storage)?;
            let found = stmt
                .query_map(params![expression, limit as i64], |r| {
                    Ok((r.get::<_, f64>(1)?, index, r.get::<_, i64>(0)?))
                })
                .map_err(storage)?;
            for row in found {
                rows.push(row.map_err(storage)?);
            }
        }
        rows.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
                .then(a.2.cmp(&b.2))
        });
        rows.truncate(limit);
        Ok(rows)
    }

    /// Hits for what the reader typed, best first.
    pub fn search(&self, typed: &str, limit: usize) -> Result<Vec<TextHit>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let terms = QueryTerms::parse(typed);
        if terms.keys.is_empty() {
            return Ok(Vec::new());
        }
        let mut rows = self.run_tier(&terms.phrase(), limit)?;
        if terms.keys.len() > 1 && rows.len() < limit {
            for row in self.run_tier(&terms.all_words(), limit)? {
                if rows.len() == limit {
                    break;
                }
                if !rows.iter().any(|r| r.1 == row.1 && r.2 == row.2) {
                    rows.push(row);
                }
            }
        }
        rows.iter()
            .map(|(_, index, rowid)| self.hit(*index, *rowid, &terms))
            .collect()
    }

    fn hit(&self, index: usize, rowid: i64, terms: &QueryTerms) -> Result<TextHit> {
        let (schema, edition) = &self.packs[index];
        if let Some(scheme) = &self.works[index] {
            return self.work_hit(schema, edition, scheme, rowid, terms);
        }
        let body: String = self
            .conn
            .prepare_cached(&format!(
                "SELECT body FROM \"{schema}\".text WHERE verse_id = ?1"
            ))
            .map_err(storage)?
            .query_row(params![rowid], |r| r.get(0))
            .map_err(storage)?;
        let verse = VerseId(u32::try_from(rowid).unwrap_or(0));
        let book = BookId::from_number(verse.book())
            .ok_or_else(|| RetrievalError::Storage(format!("verse id {rowid} names no book")))?;
        Ok(TextHit {
            edition_id: edition.clone(),
            reference: Reference::verse(book, u32::from(verse.chapter()), u32::from(verse.verse()))
                .to_osis(),
            snippet: snippet(&body, terms),
        })
    }
}

impl EditionSearch {
    /// A hit in a work pack: the section's `scheme:locator` and a snippet on one line.
    fn work_hit(
        &self,
        schema: &str,
        edition: &str,
        scheme: &str,
        rowid: i64,
        terms: &QueryTerms,
    ) -> Result<TextHit> {
        let (locator, body): (String, String) = self
            .conn
            .prepare_cached(&format!(
                "SELECT s.locator, t.body FROM \"{schema}\".work_section s
                 JOIN \"{schema}\".text t ON t.verse_id = s.ordinal WHERE s.ordinal = ?1"
            ))
            .map_err(storage)?
            .query_row(params![rowid], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(storage)?;
        let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");
        Ok(TextHit {
            edition_id: edition.to_owned(),
            reference: format!("{scheme}:{locator}"),
            snippet: snippet(&flat, terms),
        })
    }
}

impl TextIndex for EditionSearch {
    fn search_text(&self, query: &TextQuery) -> Result<Vec<IndexHit>> {
        let terms = QueryTerms::parse(&query.text);
        if terms.keys.is_empty() {
            return Ok(Vec::new());
        }
        let rows = self.run_tier(&terms.all_words(), query.limit)?;
        Ok(rows
            .into_iter()
            .filter(|(_, index, _)| {
                // Verse hits only: a work pack's rows are sections (B016).
                self.works[*index].is_none()
                    && (query.editions.is_empty() || query.editions.contains(&self.packs[*index].1))
            })
            .filter(|(_, _, rowid)| {
                query
                    .book
                    .is_none_or(|book| rowid / 1_000_000 == i64::from(book))
            })
            .map(|(score, index, rowid)| IndexHit {
                edition: self.packs[index].1.clone(),
                verse: VerseId(u32::try_from(rowid).unwrap_or(0)),
                score,
            })
            .collect())
    }

    fn normalization_mode(&self) -> &'static str {
        NORMALIZATION_MODE
    }

    fn normalize(&self, text: &str) -> String {
        normalize(text)
    }
}

impl TextProvider for EditionSearch {
    /// A failure is no hits: the palette's other sections still answer, and the query itself
    /// cannot be malformed (the module documentation), so what fails is the storage.
    fn search(&self, query: &str, limit: usize) -> Vec<TextHit> {
        EditionSearch::search(self, query, limit).unwrap_or_default()
    }
}

/// What the reader typed, as search keys.
struct QueryTerms {
    keys: Vec<String>,
    last_is_prefix: bool,
}

impl QueryTerms {
    fn parse(typed: &str) -> QueryTerms {
        QueryTerms {
            keys: tokenize(typed).into_iter().map(|t| t.key).collect(),
            last_is_prefix: !typed.ends_with(char::is_whitespace),
        }
    }

    /// The keys hold letters and digits only, so quoting is enough to make them literal.
    fn phrase(&self) -> String {
        let star = if self.last_is_prefix { "*" } else { "" };
        format!("\"{}\"{star}", self.keys.join(" "))
    }

    fn all_words(&self) -> String {
        let last = self.keys.len() - 1;
        let words: Vec<String> = self
            .keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let star = if i == last && self.last_is_prefix {
                    "*"
                } else {
                    ""
                };
                format!("\"{key}\"{star}")
            })
            .collect();
        words.join(" AND ")
    }

    fn matches(&self, token: &Token) -> bool {
        let last = self.keys.len() - 1;
        self.keys.iter().enumerate().any(|(i, key)| {
            if i == last && self.last_is_prefix {
                token.key.starts_with(key.as_str())
            } else {
                token.key == *key
            }
        })
    }
}

/// A piece of the verse around the first word that matched, cut at spaces, byte-exact, with `…`
/// where the verse goes on. The whole verse when it is short.
fn snippet(body: &str, terms: &QueryTerms) -> String {
    if body.chars().count() <= SNIPPET_SCALARS {
        return body.to_owned();
    }
    let tokens = tokenize(body);
    let first = tokens.iter().position(|t| terms.matches(t)).unwrap_or(0);
    let start_token = first.saturating_sub(SNIPPET_WORDS_BEFORE);
    // Back to the space before that word, so a word joined by a maqqef stays whole.
    let start = tokens.get(start_token).map_or(0, |t| {
        body[..t.start].rfind(char::is_whitespace).map_or(0, |at| {
            at + body[at..].chars().next().map_or(0, char::len_utf8)
        })
    });
    let mut end = body.len();
    let mut taken = 0;
    for (at, c) in body[start..].char_indices() {
        taken += 1;
        if taken > SNIPPET_SCALARS && c.is_whitespace() {
            end = start + at;
            break;
        }
    }
    let mut out = String::with_capacity(end - start + 6);
    if start > 0 {
        out.push('…');
    }
    out.push_str(&body[start..end]);
    if end < body.len() {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::{PackBuilder, PackManifest};

    /// A hand-written search pack; none of this is corpus text.
    fn build(path: &Path, edition: &str, verses: &[(VerseId, &str)]) {
        let manifest = PackManifest {
            edition: edition.to_owned(),
            scheme: "eng".to_owned(),
            source: "test".to_owned(),
            license: "test".to_owned(),
            attribution: "test".to_owned(),
            work: None,
            locator_scheme: None,
        };
        let mut builder = PackBuilder::create(path, &manifest, &[]).unwrap();
        for (verse, body) in verses {
            builder.add_verse(*verse, body).unwrap();
        }
        builder.finish().unwrap();
    }

    fn directory(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "openlogos-edition-search-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        build(
            &dir.join("en.sqlite"),
            "en",
            &[
                (VerseId::new(1, 1, 1), "The light shines over the deep."),
                (VerseId::new(1, 1, 2), "The deep shines, and over it light."),
                (VerseId::new(19, 3, 0), "A song of the lighthouse."),
                (
                    VerseId::new(44, 1, 3),
                    "Some long verse with many plain words before the rare word appears here at last, and then it goes on a good while after that so a cut is needed.",
                ),
            ],
        );
        build(
            &dir.join("gr.sqlite"),
            "gr",
            &[(
                VerseId::new(44, 1, 1),
                "Ἐν ἀρχῇ ἦν ὁ λόγος, καὶ θεὸς ἦν ὁ λόγος.",
            )],
        );
        build(
            &dir.join("he.sqlite"),
            "he",
            &[(VerseId::new(1, 1, 1), "בְּרֵאשִׁ֖ית בָּרָ֣א אֶל־הָאָֽרֶץ׃")],
        );
        dir
    }

    fn hits(search: &EditionSearch, typed: &str) -> Vec<(String, String)> {
        search
            .search(typed, 10)
            .unwrap()
            .into_iter()
            .map(|h| (h.edition_id, h.reference))
            .collect()
    }

    #[test]
    fn three_scripts_find_their_verses_folded() {
        let dir = directory("scripts");
        let search = EditionSearch::open_directory(&dir).unwrap();
        assert_eq!(search.editions(), ["en", "gr", "he"]);
        assert_eq!(hits(&search, "λογος "), [("gr".into(), "John.1.1".into())]);
        assert_eq!(hits(&search, "ΛΌΓΟΣ "), [("gr".into(), "John.1.1".into())]);
        assert_eq!(hits(&search, "בראשית"), [("he".into(), "Gen.1.1".into())]);
        assert_eq!(hits(&search, "הארץ"), [("he".into(), "Gen.1.1".into())]);
    }

    #[test]
    fn the_last_word_is_a_prefix_until_a_space_follows_it() {
        let dir = directory("prefix");
        let search = EditionSearch::open_directory(&dir).unwrap();
        assert!(hits(&search, "lighth").contains(&("en".into(), "Ps.3.0".into())));
        assert_eq!(hits(&search, "lighth ").len(), 0);
        let light = hits(&search, "light");
        assert_eq!(light.len(), 3, "light and lighthouse: {light:?}");
        assert_eq!(hits(&search, "light ").len(), 2);
    }

    #[test]
    fn a_phrase_comes_before_the_same_words_apart() {
        let dir = directory("phrase");
        let search = EditionSearch::open_directory(&dir).unwrap();
        assert_eq!(
            hits(&search, "light shines "),
            [
                ("en".into(), "Gen.1.1".into()),
                ("en".into(), "Gen.1.2".into())
            ]
        );
    }

    #[test]
    fn nothing_typed_is_a_syntax_error() {
        let dir = directory("syntax");
        let search = EditionSearch::open_directory(&dir).unwrap();
        for typed in [
            "\"",
            "the\"",
            "a:b",
            "John 3:",
            "NEAR(",
            "AND",
            "OR light",
            "light*",
            "(",
            "-",
            "^x",
            "light NOT",
            "{}",
            "'",
            "the's",
            "*",
        ] {
            assert!(search.search(typed, 5).is_ok(), "{typed}");
        }
        assert_eq!(hits(&search, " , ; ").len(), 0);
        assert_eq!(hits(&search, "").len(), 0);
    }

    #[test]
    fn snippets_are_the_verse_when_short_and_a_window_on_the_hit_when_long() {
        let dir = directory("snippet");
        let search = EditionSearch::open_directory(&dir).unwrap();
        let short = search.search("deep ", 1).unwrap().remove(0);
        assert!(short.snippet.starts_with("The ") && !short.snippet.contains('…'));
        let long = search.search("rare ", 1).unwrap().remove(0);
        assert!(long.snippet.starts_with('…') && long.snippet.ends_with('…'));
        assert!(
            long.snippet.contains("before the rare word"),
            "{}",
            long.snippet
        );
        assert!(long.snippet.chars().count() < 110, "{}", long.snippet);
    }

    #[test]
    fn the_limit_holds_and_the_order_repeats() {
        let dir = directory("limit");
        let search = EditionSearch::open_directory(&dir).unwrap();
        assert_eq!(search.search("the", 2).unwrap().len(), 2);
        assert!(search.search("the", 0).unwrap().is_empty());
        assert_eq!(hits(&search, "the"), hits(&search, "the"));
    }

    #[test]
    fn bytes_answer_the_same_as_files() {
        let dir = directory("bytes");
        let files = EditionSearch::open_directory(&dir).unwrap();
        let mut memory = EditionSearch::new().unwrap();
        for name in ["en", "gr", "he"] {
            let bytes = std::fs::read(dir.join(format!("{name}.sqlite"))).unwrap();
            assert_eq!(memory.attach_bytes(&bytes).unwrap(), name);
        }
        for typed in ["light", "λογος", "ברא", "the deep"] {
            assert_eq!(hits(&files, typed), hits(&memory, typed), "{typed}");
        }
    }

    #[test]
    fn a_pack_of_another_schema_or_a_second_copy_is_refused() {
        let dir = directory("refuse");
        let mut search = EditionSearch::new().unwrap();
        search.attach_file(&dir.join("en.sqlite")).unwrap();
        assert!(search.attach_file(&dir.join("en.sqlite")).is_err());
        assert!(search.attach_bytes(b"not a database").is_err());
        // Still usable after the refusals.
        assert_eq!(hits(&search, "deep ").len(), 2);
    }

    #[test]
    fn the_trait_runs_through_the_same_index() {
        let dir = directory("trait");
        let search = EditionSearch::open_directory(&dir).unwrap();
        let hits = search
            .search_text(&TextQuery {
                text: "light".to_owned(),
                editions: vec!["en".to_owned()],
                book: Some(19),
                limit: 10,
            })
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].verse, VerseId::new(19, 3, 0));
        assert_eq!(search.normalization_mode(), "ol-norm-1");
    }

    #[test]
    fn it_answers_as_a_palette_text_provider() {
        let dir = directory("provider");
        let search = EditionSearch::open_directory(&dir).unwrap();
        let provider: &dyn TextProvider = &search;
        let hits = provider.search("λογος", 5);
        assert_eq!(hits[0].edition_id, "gr");
        assert_eq!(hits[0].reference, "John.1.1");
    }
}
