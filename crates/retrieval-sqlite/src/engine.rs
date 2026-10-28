//! The engine: one connection with packs attached read-only on demand, the token columns of
//! each edition loaded lazily, and an optional link index. Implements the five ADR 0002
//! traits; nothing here is visible to callers except through them and the open/attach calls.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use openlogos_retrieval::{
    Edge, EditionInfo, Facet, Locator, QueryVector, Reached, RelationStore, Result, RetrievalError,
    TextHit, TextIndex, TextQuery, TextStore, TokenEngine, TokenHits, TokenInfo, TokenPos,
    TokenQuery, TraversalOptions, VectorHit, VectorIndex, VectorSetInfo, VerseId, VerseMapping,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

use crate::fts::register_tokenizer;
use crate::links::LinkIndex;
use crate::pack::PACK_SCHEMA_VERSION;
use crate::token_engine::TokenColumns;
use crate::tokenizer::{NORMALIZATION_MODE, normalize};
use crate::vectors::{install_vec_extension, knn, parse_quantization, storage};

/// One attached pack.
#[derive(Debug, Clone)]
pub struct AttachedPack {
    /// Schema name inside the connection (`p1`, `p2`, ...).
    pub schema: String,
    /// Edition id from the manifest.
    pub edition: String,
    /// Scheme from the manifest.
    pub scheme: String,
    /// File path.
    pub path: PathBuf,
}

/// The SQLite retrieval engine.
pub struct Engine {
    conn: Connection,
    packs: Vec<AttachedPack>,
    next_schema: u32,
    columns: RefCell<HashMap<String, Rc<TokenColumns>>>,
    links: Option<LinkIndex>,
}

impl Engine {
    /// An engine with an in-memory main database and no link index.
    pub fn new() -> Result<Engine> {
        install_vec_extension();
        let conn = Connection::open_in_memory_with_flags(
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(storage)?;
        register_tokenizer(&conn).map_err(storage)?;
        Ok(Engine {
            conn,
            packs: Vec::new(),
            next_schema: 1,
            columns: RefCell::new(HashMap::new()),
            links: None,
        })
    }

    /// Attaches a link index database (created if missing) for relation queries.
    pub fn with_link_index(mut self, path: &Path) -> Result<Engine> {
        self.links = Some(LinkIndex::open(path)?);
        Ok(self)
    }

    /// The link index, when one is attached.
    pub fn link_index(&self) -> Option<&LinkIndex> {
        self.links.as_ref()
    }

    /// Mutable access to the link index for installs and removals.
    pub fn link_index_mut(&mut self) -> Option<&mut LinkIndex> {
        self.links.as_mut()
    }

    /// Attaches a pack file read-only (`immutable=1`) and returns what it serves.
    pub fn attach_pack(&mut self, path: &Path) -> Result<EditionInfo> {
        let schema = format!("p{}", self.next_schema);
        let uri = format!("file:{}?immutable=1", path.display());
        self.conn
            .execute(&format!("ATTACH DATABASE ?1 AS \"{schema}\""), params![uri])
            .map_err(storage)?;
        self.next_schema += 1;
        let manifest = |key: &str| -> Result<String> {
            self.conn
                .query_row(
                    &format!("SELECT value FROM \"{schema}\".manifest WHERE key = ?1"),
                    params![key],
                    |r| r.get(0),
                )
                .map_err(storage)
        };
        let version = manifest("schema_version")?;
        if version != PACK_SCHEMA_VERSION {
            let _ = self
                .conn
                .execute_batch(&format!("DETACH DATABASE \"{schema}\""));
            return Err(RetrievalError::Storage(format!(
                "pack schema {version} is not the supported {PACK_SCHEMA_VERSION}"
            )));
        }
        let mode = manifest("normalization_mode")?;
        if mode != NORMALIZATION_MODE {
            let _ = self
                .conn
                .execute_batch(&format!("DETACH DATABASE \"{schema}\""));
            return Err(RetrievalError::Storage(format!(
                "pack normalization {mode} is not this engine's {NORMALIZATION_MODE}"
            )));
        }
        let edition = manifest("edition")?;
        let scheme = manifest("scheme")?;
        if self.packs.iter().any(|p| p.edition == edition) {
            let _ = self
                .conn
                .execute_batch(&format!("DETACH DATABASE \"{schema}\""));
            return Err(RetrievalError::Storage(format!(
                "edition `{edition}` is already attached"
            )));
        }
        self.packs.push(AttachedPack {
            schema,
            edition: edition.clone(),
            scheme,
            path: path.to_path_buf(),
        });
        self.edition_info(&edition)
    }

    /// Detaches an edition's pack.
    pub fn detach_pack(&mut self, edition: &str) -> Result<()> {
        let i = self
            .packs
            .iter()
            .position(|p| p.edition == edition)
            .ok_or_else(|| RetrievalError::UnknownEdition(edition.to_owned()))?;
        let pack = self.packs.remove(i);
        self.columns.borrow_mut().remove(edition);
        self.conn
            .execute_batch(&format!("DETACH DATABASE \"{}\"", pack.schema))
            .map_err(storage)
    }

    /// Attached packs.
    pub fn packs(&self) -> &[AttachedPack] {
        &self.packs
    }

    /// The connection (bench harness only).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    fn pack(&self, edition: &str) -> Result<&AttachedPack> {
        self.packs
            .iter()
            .find(|p| p.edition == edition)
            .ok_or_else(|| RetrievalError::UnknownEdition(edition.to_owned()))
    }

    /// The token columns of an edition, loaded on first use.
    pub fn columns(&self, edition: &str) -> Result<Rc<TokenColumns>> {
        if let Some(c) = self.columns.borrow().get(edition) {
            return Ok(c.clone());
        }
        let pack = self.pack(edition)?;
        let columns = Rc::new(TokenColumns::load(&self.conn, &pack.schema)?);
        self.columns
            .borrow_mut()
            .insert(edition.to_owned(), columns.clone());
        Ok(columns)
    }

    fn edition_info(&self, edition: &str) -> Result<EditionInfo> {
        let pack = self.pack(edition)?;
        let count = |sql: &str| -> Result<u64> {
            self.conn
                .query_row(&sql.replace("{s}", &pack.schema), [], |r| {
                    r.get::<_, i64>(0)
                })
                .map(|n| n as u64)
                .map_err(storage)
        };
        let names = |sql: &str| -> Result<Vec<String>> {
            let mut stmt = self
                .conn
                .prepare(&sql.replace("{s}", &pack.schema))
                .map_err(storage)?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(storage)?;
            rows.collect::<std::result::Result<_, _>>().map_err(storage)
        };
        Ok(EditionInfo {
            edition: pack.edition.clone(),
            scheme: pack.scheme.clone(),
            verses: count("SELECT count(*) FROM \"{s}\".text")?,
            tokens: count("SELECT count(*) FROM \"{s}\".token")?,
            features: names("SELECT name FROM \"{s}\".feature ORDER BY name")?,
            containers: names("SELECT name FROM \"{s}\".container_kind ORDER BY name")?,
        })
    }

    fn links(&self) -> Result<&LinkIndex> {
        self.links
            .as_ref()
            .ok_or_else(|| RetrievalError::LayerUnavailable("link index".into()))
    }
}

impl TextStore for Engine {
    fn editions(&self) -> Result<Vec<EditionInfo>> {
        self.packs
            .iter()
            .map(|p| self.edition_info(&p.edition))
            .collect()
    }

    fn verse_text(&self, edition: &str, verse: VerseId) -> Result<Option<String>> {
        let pack = self.pack(edition)?;
        self.conn
            .query_row(
                &format!(
                    "SELECT body FROM \"{}\".text WHERE verse_id = ?1",
                    pack.schema
                ),
                params![i64::from(verse.0)],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)
    }

    fn chapter_text(
        &self,
        edition: &str,
        book: u8,
        chapter: u16,
    ) -> Result<Vec<(VerseId, String)>> {
        let pack = self.pack(edition)?;
        let key = i64::from(VerseId::new(book, chapter, 0).0);
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "SELECT verse_id, body FROM \"{}\".text WHERE verse_id >= ?1 AND verse_id < ?1 + 1000 ORDER BY verse_id",
                pack.schema
            ))
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![key], |r| {
                Ok((VerseId(r.get::<_, i64>(0)? as u32), r.get::<_, String>(1)?))
            })
            .map_err(storage)?;
        rows.collect::<std::result::Result<_, _>>().map_err(storage)
    }

    fn map_verse(
        &self,
        edition: &str,
        verse: VerseId,
        to_scheme: &str,
    ) -> Result<Vec<VerseMapping>> {
        let pack = self.pack(edition)?;
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "SELECT ref_to, kind FROM \"{}\".verse_map WHERE scheme_from = ?1 AND ref_from = ?2 AND scheme_to = ?3 ORDER BY ref_to",
                pack.schema
            ))
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![pack.scheme, i64::from(verse.0), to_scheme], |r| {
                Ok(VerseMapping {
                    scheme: to_scheme.to_owned(),
                    verse: VerseId(r.get::<_, i64>(0)? as u32),
                    kind: r.get(1)?,
                })
            })
            .map_err(storage)?;
        rows.collect::<std::result::Result<_, _>>().map_err(storage)
    }
}

impl TextIndex for Engine {
    fn search_text(&self, query: &TextQuery) -> Result<Vec<TextHit>> {
        let packs: Vec<&AttachedPack> = if query.editions.is_empty() {
            self.packs.iter().collect()
        } else {
            query
                .editions
                .iter()
                .map(|e| self.pack(e))
                .collect::<Result<_>>()?
        };
        let mut hits = Vec::new();
        for pack in packs {
            let mut stmt = self
                .conn
                .prepare_cached(&format!(
                    "SELECT rowid, bm25(text_fts) FROM \"{0}\".text_fts WHERE text_fts MATCH ?1
                     AND (?2 IS NULL OR (rowid >= ?2 * 1000000 AND rowid < (?2 + 1) * 1000000))
                     ORDER BY bm25(text_fts) LIMIT ?3",
                    pack.schema
                ))
                .map_err(storage)?;
            let rows = stmt
                .query_map(
                    params![query.text, query.book.map(i64::from), query.limit as i64],
                    |r| {
                        Ok(TextHit {
                            edition: pack.edition.clone(),
                            verse: VerseId(r.get::<_, i64>(0)? as u32),
                            score: r.get(1)?,
                        })
                    },
                )
                .map_err(|e| RetrievalError::InvalidQuery(format!("fts: {e}")))?;
            for row in rows {
                hits.push(row.map_err(|e| RetrievalError::InvalidQuery(format!("fts: {e}")))?);
            }
        }
        hits.sort_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(query.limit);
        Ok(hits)
    }

    fn normalization_mode(&self) -> &'static str {
        NORMALIZATION_MODE
    }

    fn normalize(&self, text: &str) -> String {
        normalize(text)
    }
}

impl TokenEngine for Engine {
    fn token_search(&self, query: &TokenQuery) -> Result<TokenHits> {
        let pack = self.pack(&query.edition)?;
        let columns = self.columns(&query.edition)?;
        columns.search(&self.conn, &pack.schema, query)
    }

    fn token_info(&self, edition: &str, pos: TokenPos) -> Result<TokenInfo> {
        let pack = self.pack(edition)?;
        let columns = self.columns(edition)?;
        if pos as usize >= columns.len() {
            return Err(RetrievalError::InvalidQuery(format!(
                "position {pos} is beyond the edition"
            )));
        }
        let surface: String = self
            .conn
            .query_row(
                &format!(
                    "SELECT surface FROM \"{}\".token WHERE pos = ?1",
                    pack.schema
                ),
                params![i64::from(pos)],
                |r| r.get(0),
            )
            .map_err(storage)?;
        let features = columns
            .features
            .iter()
            .filter_map(|(name, col)| {
                let id = col.ids[pos as usize];
                (id != 0).then(|| (name.clone(), col.values[id as usize].clone()))
            })
            .collect();
        Ok(TokenInfo {
            pos,
            verse: VerseId(columns.verse[pos as usize]),
            surface,
            features,
        })
    }

    fn facets(&self, edition: &str, positions: &[TokenPos], feature: &str) -> Result<Vec<Facet>> {
        self.columns(edition)?.facets(positions, feature)
    }
}

impl RelationStore for Engine {
    fn backlinks(&self, target: &Locator, rel: Option<&str>, limit: usize) -> Result<Vec<Edge>> {
        self.links()?.backlinks(target, rel, limit)
    }

    fn backlink_count(&self, target: &Locator, rel: Option<&str>) -> Result<u64> {
        self.links()?.backlink_count(target, rel)
    }

    fn outlinks(&self, source: &Locator, rel: Option<&str>, limit: usize) -> Result<Vec<Edge>> {
        self.links()?.outlinks(source, rel, limit)
    }

    fn traverse(&self, start: &Locator, options: &TraversalOptions) -> Result<Vec<Reached>> {
        self.links()?.traverse(start, options)
    }

    fn most_cited(
        &self,
        work: &str,
        rel: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Locator, u64)>> {
        self.links()?.most_cited(work, rel, limit)
    }
}

impl VectorIndex for Engine {
    fn vector_sets(&self) -> Result<Vec<VectorSetInfo>> {
        let mut sets = Vec::new();
        for pack in &self.packs {
            let mut stmt = self
                .conn
                .prepare(&format!(
                    "SELECT name, model, revision, dimensions, quantization, count FROM \"{}\".vector_set ORDER BY name",
                    pack.schema
                ))
                .map_err(storage)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(VectorSetInfo {
                        name: r.get(0)?,
                        model: r.get(1)?,
                        revision: r.get(2)?,
                        dimensions: r.get::<_, i64>(3)? as u32,
                        quantization: parse_quantization(&r.get::<_, String>(4)?)
                            .unwrap_or(openlogos_retrieval::Quantization::Int8),
                        count: r.get::<_, i64>(5)? as u64,
                    })
                })
                .map_err(storage)?;
            for row in rows {
                sets.push(row.map_err(storage)?);
            }
        }
        Ok(sets)
    }

    fn nearest(&self, set: &str, query: &QueryVector, k: usize) -> Result<Vec<VectorHit>> {
        for pack in &self.packs {
            let found: Option<String> = self
                .conn
                .query_row(
                    &format!(
                        "SELECT quantization FROM \"{}\".vector_set WHERE name = ?1",
                        pack.schema
                    ),
                    params![set],
                    |r| r.get(0),
                )
                .optional()
                .map_err(storage)?;
            if let Some(q) = found {
                let quantization = parse_quantization(&q)
                    .ok_or_else(|| RetrievalError::Storage(format!("unknown quantization {q}")))?;
                return knn(&self.conn, &pack.schema, set, quantization, query, k);
            }
        }
        Err(RetrievalError::LayerUnavailable(format!(
            "vector set `{set}`"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::fixture::{build, temp_path};
    use openlogos_retrieval::{Constraint, TokenPattern, Unit};

    fn engine() -> Engine {
        let path = temp_path("engine");
        build(&path).unwrap();
        let mut engine = Engine::new().unwrap();
        let info = engine.attach_pack(&path).unwrap();
        assert_eq!(info.edition, "sblgnt-test");
        assert_eq!((info.verses, info.tokens), (3, 36));
        assert_eq!(info.containers, vec!["clause", "sentence"]);
        engine
    }

    fn lemma(e: &Engine, l: &str) -> TokenHits {
        e.token_search(&TokenQuery::single(
            "sblgnt-test",
            TokenPattern::one("lemma", l),
        ))
        .unwrap()
    }

    #[test]
    fn text_store_reads_byte_exact_text_and_maps() {
        let e = engine();
        assert_eq!(
            e.verse_text("sblgnt-test", VerseId(44_001_001))
                .unwrap()
                .as_deref(),
            Some(crate::pack::fixture::JOHN_1_1)
        );
        assert_eq!(
            e.verse_text("sblgnt-test", VerseId(44_001_003)).unwrap(),
            None
        );
        assert_eq!(e.chapter_text("sblgnt-test", 44, 1).unwrap().len(), 2);
        let maps = e
            .map_verse("sblgnt-test", VerseId(44_001_001), "eng")
            .unwrap();
        assert_eq!(maps.len(), 1);
        assert_eq!(maps[0].kind, "equal");
        assert!(
            e.map_verse("sblgnt-test", VerseId(44_001_002), "eng")
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            e.verse_text("nope", VerseId(1)),
            Err(RetrievalError::UnknownEdition(_))
        ));
    }

    #[test]
    fn text_index_searches_through_the_shared_normalizer() {
        let e = engine();
        let q = |text: &str, book: Option<u8>| {
            e.search_text(&TextQuery {
                text: text.into(),
                editions: vec![],
                book,
                limit: 10,
            })
            .unwrap()
        };
        assert_eq!(q("θεον", None).len(), 3);
        assert_eq!(q("θεον", Some(63)).len(), 1);
        assert_eq!(q("\"προς τον θεον\"", None).len(), 2);
        assert_eq!(q("αγαπη", None)[0].verse, VerseId(63_004_008));
        assert_eq!(e.normalize("Ἀγάπη"), "αγαπη");
        assert_eq!(e.normalization_mode(), "ol-norm-1");
        assert!(matches!(q("θεον", None).first(), Some(h) if h.score < 0.0));
    }

    #[test]
    fn single_slot_queries_count_postings() {
        let e = engine();
        assert_eq!(lemma(&e, "θεός").count, 5);
        assert_eq!(
            lemma(&e, "λόγος")
                .matches
                .iter()
                .map(|m| m.positions[0])
                .collect::<Vec<_>>(),
            vec![4, 7, 16]
        );
        assert_eq!(lemma(&e, "ἀγάπη").count, 1);
        assert_eq!(lemma(&e, "missing").count, 0);
        let imperfects = e
            .token_search(&TokenQuery::single(
                "sblgnt-test",
                TokenPattern::one("lemma", "εἰμί").and("tense", "imperfect"),
            ))
            .unwrap();
        assert_eq!(imperfects.count, 4);
        let mut by_book = TokenQuery::single("sblgnt-test", TokenPattern::one("lemma", "θεός"));
        by_book.book = Some(63);
        assert_eq!(e.token_search(&by_book).unwrap().count, 2);
        let err = e
            .token_search(&TokenQuery::single(
                "sblgnt-test",
                TokenPattern::one("domain", "x"),
            ))
            .unwrap_err();
        assert!(matches!(err, RetrievalError::LayerUnavailable(_)));
    }

    #[test]
    fn proximity_agreement_and_order_follow_their_definitions() {
        let e = engine();
        let mut q = TokenQuery {
            edition: "sblgnt-test".into(),
            slots: vec![
                TokenPattern::one("lemma", "πρός"),
                TokenPattern::one("lemma", "θεός"),
            ],
            scope: Unit::Verse,
            constraints: vec![Constraint::Within {
                a: 0,
                b: 1,
                distance: 2,
            }],
            book: None,
            limit: 100,
        };
        // πρὸς τὸν θεόν in John 1:1 and 1:2; θεὸς at 13 is 4 after πρός at 9.
        assert_eq!(e.token_search(&q).unwrap().count, 2);
        q.constraints = vec![Constraint::Within {
            a: 0,
            b: 1,
            distance: 4,
        }];
        assert_eq!(e.token_search(&q).unwrap().count, 3);
        q.constraints = vec![Constraint::Before {
            a: 1,
            b: 0,
            distance: None,
        }];
        assert_eq!(e.token_search(&q).unwrap().count, 0);
        q.constraints = vec![Constraint::Before {
            a: 0,
            b: 1,
            distance: Some(2),
        }];
        assert_eq!(e.token_search(&q).unwrap().count, 2);
        // Article agreeing with θεός in case, number and gender within the verse:
        // John 1:1 has ὁ ×3 (nom) and τὸν (acc) for θεόν (acc) and θεὸς (nom): 1 + 3 = 4;
        // John 1:2: τὸν/θεόν 1; 1 John 4:8: τὸν/θεόν 1, ὁ ×2 /θεὸς 2 → 8 in all.
        q.slots = vec![
            TokenPattern::one("sp", "det"),
            TokenPattern::one("lemma", "θεός"),
        ];
        q.constraints = vec![Constraint::Agree {
            a: 0,
            b: 1,
            features: vec!["case".into(), "number".into(), "gender".into()],
        }];
        assert_eq!(e.token_search(&q).unwrap().count, 8);
        // Agreement on a feature one side lacks never holds.
        q.constraints = vec![Constraint::Agree {
            a: 0,
            b: 1,
            features: vec!["tense".into()],
        }];
        assert_eq!(e.token_search(&q).unwrap().count, 0);
        // The same lemma in both slots binds distinct tokens, in both orders.
        q.slots = vec![
            TokenPattern::one("lemma", "λόγος"),
            TokenPattern::one("lemma", "λόγος"),
        ];
        q.constraints = vec![];
        assert_eq!(e.token_search(&q).unwrap().count, 6);
        q.limit = 2;
        let hits = e.token_search(&q).unwrap();
        assert_eq!((hits.count, hits.matches.len()), (6, 2));
    }

    #[test]
    fn unit_scopes_and_structural_constraints_use_the_containers() {
        let e = engine();
        let mut q = TokenQuery {
            edition: "sblgnt-test".into(),
            slots: vec![
                TokenPattern::one("lemma", "ἀρχή"),
                TokenPattern::one("lemma", "θεός"),
            ],
            scope: Unit::Chapter,
            constraints: vec![],
            book: None,
            limit: 100,
        };
        // Chapter scope: 2 ἀρχή × 3 θεός in John 1 = 6; verse scope: 2 + 1 = 3; book: 6.
        assert_eq!(e.token_search(&q).unwrap().count, 6);
        q.scope = Unit::Verse;
        assert_eq!(e.token_search(&q).unwrap().count, 3);
        q.scope = Unit::Book;
        assert_eq!(e.token_search(&q).unwrap().count, 6);
        // Clause scope: ἀρχή (pos 1) shares no innermost clause with a θεός; John 1:2 is one
        // clause: 1 pair.
        q.scope = Unit::Container("clause".into());
        assert_eq!(e.token_search(&q).unwrap().count, 1);
        // PEER/ANCESTOR with nesting: πρός (9) and θεόν (11) share the nested clause; λόγος (7)
        // and θεόν (11) share only the outer clause.
        q.scope = Unit::Verse;
        q.slots = vec![
            TokenPattern::one("lemma", "λόγος"),
            TokenPattern::one("lemma", "θεός"),
        ];
        q.constraints = vec![Constraint::Peer {
            a: 0,
            b: 1,
            unit: Unit::Container("clause".into()),
        }];
        // λόγος 16 and θεὸς 13 share clause 12..=16: exactly one peer pair.
        assert_eq!(e.token_search(&q).unwrap().count, 1);
        q.constraints = vec![Constraint::Ancestor {
            a: 0,
            b: 1,
            unit: Unit::Container("clause".into()),
        }];
        // Plus λόγος 7 with θεόν 11 through the outer clause 5..=11.
        assert_eq!(e.token_search(&q).unwrap().count, 2);
        q.constraints = vec![Constraint::XPeer {
            a: 0,
            b: 1,
            unit: Unit::Container("clause".into()),
        }];
        // 3 λόγος × 2 θεός in the verse = 6 pairs, minus the one peer pair.
        assert_eq!(e.token_search(&q).unwrap().count, 5);
        q.constraints = vec![Constraint::Peer {
            a: 0,
            b: 1,
            unit: Unit::Container("paragraph".into()),
        }];
        assert!(matches!(
            e.token_search(&q).unwrap_err(),
            RetrievalError::LayerUnavailable(_)
        ));
        q.constraints = vec![Constraint::Within {
            a: 0,
            b: 2,
            distance: 1,
        }];
        assert!(matches!(
            e.token_search(&q).unwrap_err(),
            RetrievalError::InvalidQuery(_)
        ));
    }

    #[test]
    fn token_info_and_facets_read_the_columns() {
        let e = engine();
        let info = e.token_info("sblgnt-test", 26).unwrap();
        assert_eq!(info.surface, "ἀγαπῶν");
        assert_eq!(info.verse, VerseId(63_004_008));
        assert!(
            info.features
                .contains(&("mood".to_owned(), "participle".to_owned()))
        );
        let positions: Vec<u32> = lemma(&e, "θεός")
            .matches
            .iter()
            .map(|m| m.positions[0])
            .collect();
        let by_case = e.facets("sblgnt-test", &positions, "case").unwrap();
        assert_eq!(
            by_case[0],
            Facet {
                value: "accusative".into(),
                count: 3
            }
        );
        assert_eq!(
            by_case[1],
            Facet {
                value: "nominative".into(),
                count: 2
            }
        );
        let by_book = e.facets("sblgnt-test", &positions, "book").unwrap();
        assert_eq!(
            by_book,
            vec![
                Facet {
                    value: "44".into(),
                    count: 3
                },
                Facet {
                    value: "63".into(),
                    count: 2
                }
            ]
        );
    }

    #[test]
    fn relations_need_a_link_index_and_then_answer_from_it() {
        let path = temp_path("engine-links");
        build(&path).unwrap();
        let mut e = Engine::new().unwrap();
        e.attach_pack(&path).unwrap();
        let target = Locator::verse("sblgnt-test", VerseId(44_001_001));
        assert!(matches!(
            e.backlink_count(&target, None),
            Err(RetrievalError::LayerUnavailable(_))
        ));
        let mut e = e
            .with_link_index(&path.with_file_name("links.sqlite"))
            .unwrap();
        e.link_index_mut()
            .unwrap()
            .install_pack("sblgnt-test", &path)
            .unwrap();
        assert_eq!(e.backlink_count(&target, None).unwrap(), 1);
        assert_eq!(
            e.backlinks(&target, Some("cross_reference"), 10).unwrap()[0]
                .src
                .locator,
            "63004008"
        );
    }

    #[test]
    fn vector_sets_are_listed_and_searched_per_pack() {
        use openlogos_retrieval::Quantization;
        let path = temp_path("engine-vectors");
        let mut b = crate::pack::PackBuilder::create(&path, &crate::pack::fixture::manifest(), &[])
            .unwrap();
        b.add_vector_set("v", "m", "r", 8, Quantization::Int8)
            .unwrap();
        b.add_vector("v", 7, &[1, 1, 1, 1, 1, 1, 1, 1]).unwrap();
        b.add_vector("v", 8, &[9, 9, 9, 9, 9, 9, 9, 9]).unwrap();
        b.finish().unwrap();
        let mut e = Engine::new().unwrap();
        e.attach_pack(&path).unwrap();
        let sets = e.vector_sets().unwrap();
        assert_eq!(
            (sets[0].name.as_str(), sets[0].count, sets[0].quantization),
            ("v", 2, Quantization::Int8)
        );
        let hits = e.nearest("v", &QueryVector::Int8(vec![8; 8]), 2).unwrap();
        assert_eq!(hits[0].id, 8);
        assert!(matches!(
            e.nearest("w", &QueryVector::Int8(vec![8; 8]), 2),
            Err(RetrievalError::LayerUnavailable(_))
        ));
    }

    #[test]
    fn attach_rejects_duplicates_and_detach_frees_the_edition() {
        let path = temp_path("engine-attach");
        build(&path).unwrap();
        let mut e = Engine::new().unwrap();
        e.attach_pack(&path).unwrap();
        assert!(e.attach_pack(&path).is_err());
        assert_eq!(e.editions().unwrap().len(), 1);
        e.detach_pack("sblgnt-test").unwrap();
        assert!(e.editions().unwrap().is_empty());
        e.attach_pack(&path).unwrap();
        assert_eq!(lemma(&e, "θεός").count, 5);
    }
}
