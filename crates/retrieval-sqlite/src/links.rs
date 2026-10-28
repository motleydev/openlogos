//! The device-local link index (specs/19, Cross-resource links, item 3): one mutable SQLite
//! database that merges the outbound edges of every installed pack, keyed by destination, so
//! "who cites this" never attaches hundreds of packs (SQLite allows at most 125). It is
//! updated per pack on install and removal and rebuilt deterministically from the installed
//! set: packs are merged in pack-id order, rows are inserted in sorted order, nothing
//! wall-clock is stored, and a fresh build is vacuumed, so two builds from the same installed
//! set are byte-identical files (B068). Counts it answers are library-relative; the
//! `cited_count` table keeps them current per install (the B064 `most_cited` decision).
//!
//! User material (notes, collection items) enters as `user`-provenance edges that carry no
//! pack: they appear in backlinks like any other edge, survive a rebuild from packs, and are
//! refused by the pack writer (specs/19 item 9).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use openlogos_retrieval::{
    ChainHop, ChainOptions, ChainStop, Direction, Edge, Locator, ProvenanceKind, QuotationChain,
    Reached, RelationStore, Result, RetrievalError, ReviewState, TraversalOptions,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

use crate::vectors::storage;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS pack(id INTEGER PRIMARY KEY, pack_id TEXT NOT NULL UNIQUE, path TEXT NOT NULL,
    edges INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS work(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
CREATE TABLE IF NOT EXISTS rel(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
CREATE TABLE IF NOT EXISTS link(src_work INTEGER NOT NULL, src_locator TEXT NOT NULL, rel INTEGER NOT NULL,
    dst_work INTEGER NOT NULL, dst_locator TEXT NOT NULL, weight REAL NOT NULL, provenance INTEGER NOT NULL,
    source_work INTEGER NOT NULL, method TEXT NOT NULL DEFAULT '', review_state INTEGER NOT NULL DEFAULT 1,
    context TEXT, pack INTEGER, user_source TEXT);
CREATE INDEX IF NOT EXISTS link_dst ON link(dst_work, dst_locator, rel);
CREATE INDEX IF NOT EXISTS link_src ON link(src_work, src_locator, rel);
CREATE INDEX IF NOT EXISTS link_pack ON link(pack);
CREATE INDEX IF NOT EXISTS link_user ON link(user_source);
CREATE TABLE IF NOT EXISTS cited_count(dst_work INTEGER NOT NULL, dst_locator TEXT NOT NULL, rel INTEGER NOT NULL,
    n INTEGER NOT NULL, PRIMARY KEY(dst_work, dst_locator, rel)) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS cited_count_top ON cited_count(dst_work, rel, n DESC, dst_locator);
";

/// Relation id meaning "any relation" in `cited_count` (real ids start at 1).
const ANY_REL: i64 = 0;

/// The columns `edge_from_row` reads, in order.
const EDGE_COLUMNS: &str =
    "sw.name, l.src_locator, r.name, dw.name, l.dst_locator, l.weight, l.provenance, ow.name,
     l.method, l.review_state";
/// The joins that resolve the dictionary ids of `link l`.
const EDGE_FROM: &str =
    "FROM link l JOIN work sw ON sw.id = l.src_work JOIN work dw ON dw.id = l.dst_work
    JOIN rel r ON r.id = l.rel JOIN work ow ON ow.id = l.source_work";

fn provenance_from_code(code: i64) -> ProvenanceKind {
    match code {
        0 => ProvenanceKind::Curated,
        1 => ProvenanceKind::Extracted,
        2 => ProvenanceKind::ModelSimilarity,
        _ => ProvenanceKind::User,
    }
}

fn provenance_code(kind: ProvenanceKind) -> i64 {
    match kind {
        ProvenanceKind::Curated => 0,
        ProvenanceKind::Extracted => 1,
        ProvenanceKind::ModelSimilarity => 2,
        ProvenanceKind::User => 3,
    }
}

/// Review states as stored: the order of `ReviewState`'s variants.
const REVIEW_STATES: [ReviewState; 8] = [
    ReviewState::Proposed,
    ReviewState::SourceAttested,
    ReviewState::Reviewed,
    ReviewState::Published,
    ReviewState::Rejected,
    ReviewState::Disputed,
    ReviewState::Superseded,
    ReviewState::Retracted,
];

fn review_code(state: ReviewState) -> i64 {
    REVIEW_STATES
        .iter()
        .position(|s| *s == state)
        .map_or(1, |i| i as i64)
}

fn review_from_code(code: i64) -> ReviewState {
    usize::try_from(code)
        .ok()
        .and_then(|i| REVIEW_STATES.get(i).copied())
        .unwrap_or(ReviewState::SourceAttested)
}

/// The SQL that maps a pack's `review_state` text to the stored code, in `REVIEW_STATES` order.
const REVIEW_CASE: &str = "CASE e.review_state WHEN 'proposed' THEN 0 WHEN 'source_attested' THEN 1
    WHEN 'reviewed' THEN 2 WHEN 'published' THEN 3 WHEN 'rejected' THEN 4 WHEN 'disputed' THEN 5
    WHEN 'superseded' THEN 6 WHEN 'retracted' THEN 7 ELSE 1 END";

fn edge_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Edge> {
    Ok(Edge {
        src: Locator {
            work: r.get(0)?,
            locator: r.get(1)?,
        },
        rel: r.get(2)?,
        dst: Locator {
            work: r.get(3)?,
            locator: r.get(4)?,
        },
        weight: r.get::<_, f64>(5)? as f32,
        provenance: provenance_from_code(r.get(6)?),
        source_work: r.get(7)?,
        method: r.get(8)?,
        review_state: review_from_code(r.get(9)?),
    })
}

/// A user-provenance edge as the index stores it: the note (or collection item) that wrote
/// it, the edge, and the citing context the note supplied.
#[derive(Debug, Clone, PartialEq)]
pub struct UserEdge {
    /// Id of the note or item the edge comes from; removing the note removes its edges.
    pub source: String,
    /// The edge; its provenance is always `user`.
    pub edge: Edge,
    /// Citing context, if the note keeps one.
    pub context: Option<String>,
}

/// The link index database.
pub struct LinkIndex {
    conn: Connection,
}

impl LinkIndex {
    /// Opens or creates the index at `path` (WAL mode, as the user database beside it).
    pub fn open(path: &Path) -> Result<LinkIndex> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(storage)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
            .map_err(storage)?;
        conn.execute_batch(SCHEMA).map_err(storage)?;
        Ok(LinkIndex { conn })
    }

    /// Opens an in-memory index (tests).
    pub fn in_memory() -> Result<LinkIndex> {
        let conn = Connection::open_in_memory_with_flags(
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(storage)?;
        conn.execute_batch(SCHEMA).map_err(storage)?;
        Ok(LinkIndex { conn })
    }

    /// Builds a brand-new index file at `path` from `packs` (any order; merged in pack-id
    /// order), with the given user edges re-added after the packs, and closes it. The file
    /// is written in rollback-journal mode with a fixed page size and vacuumed, so two calls
    /// with the same installed set produce byte-identical files. Open it with [`LinkIndex::open`].
    pub fn build_fresh(
        path: &Path,
        packs: &[(String, std::path::PathBuf)],
        user_edges: &[UserEdge],
    ) -> Result<u64> {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut name = path.as_os_str().to_owned();
            name.push(suffix);
            let _ = std::fs::remove_file(&name);
        }
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(storage)?;
        conn.execute_batch(
            "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA page_size = 4096;",
        )
        .map_err(storage)?;
        conn.execute_batch(SCHEMA).map_err(storage)?;
        let mut index = LinkIndex { conn };
        let n = index.rebuild_with(packs, user_edges)?;
        index.conn.execute_batch("VACUUM").map_err(storage)?;
        index.conn.close().map_err(|(_, e)| storage(e))?;
        Ok(n)
    }

    /// The underlying connection (bench harness only).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Merges a pack's `edge` table into the index under `pack_id`, replacing any earlier
    /// edges of that pack, and updates the citation counts. Returns the number of edges merged.
    pub fn install_pack(&mut self, pack_id: &str, pack_path: &Path) -> Result<u64> {
        self.remove_pack(pack_id)?;
        let uri = format!("file:{}?immutable=1", pack_path.display());
        self.conn
            .execute("ATTACH DATABASE ?1 AS installing", params![uri])
            .map_err(storage)?;
        let result = (|| -> Result<u64> {
            let tx = self.conn.unchecked_transaction().map_err(storage)?;
            tx.execute(
                "INSERT INTO pack(pack_id, path, edges) VALUES (?1, ?2, 0)",
                params![pack_id, pack_path.display().to_string()],
            )
            .map_err(storage)?;
            let pack_row = tx.last_insert_rowid();
            // Dictionaries grow in sorted order so ids are a function of the installed set.
            tx.execute_batch(
                "INSERT OR IGNORE INTO work(name) SELECT name FROM (
                     SELECT src_work AS name FROM installing.edge UNION SELECT dst_work FROM installing.edge
                     UNION SELECT source_work FROM installing.edge) ORDER BY name;
                 INSERT OR IGNORE INTO rel(name) SELECT DISTINCT rel FROM installing.edge ORDER BY rel;",
            )
            .map_err(storage)?;
            let n = tx
                .execute(
                    &format!(
                        "INSERT INTO link(src_work, src_locator, rel, dst_work, dst_locator, weight, provenance,
                                          source_work, method, review_state, context, pack)
                         SELECT sw.id, e.src_locator, r.id, dw.id, e.dst_locator, e.weight,
                                CASE e.provenance_kind WHEN 'curated' THEN 0 WHEN 'extracted' THEN 1
                                     WHEN 'model_similarity' THEN 2 ELSE 3 END,
                                ow.id, e.method, {REVIEW_CASE}, e.context, ?1
                         FROM installing.edge e
                         JOIN work sw ON sw.name = e.src_work JOIN work dw ON dw.name = e.dst_work
                         JOIN rel r ON r.name = e.rel JOIN work ow ON ow.name = e.source_work
                         WHERE e.provenance_kind <> 'user'
                         ORDER BY e.src_work, e.src_locator, e.rel, e.dst_work, e.dst_locator, e.weight,
                                  e.source_work, e.method, e.provenance_kind, e.review_state, e.context"
                    ),
                    params![pack_row],
                )
                .map_err(storage)?;
            tx.execute(
                "UPDATE pack SET edges = ?1 WHERE id = ?2",
                params![n as i64, pack_row],
            )
            .map_err(storage)?;
            Self::add_counts(&tx, "pack = ?1", pack_row)?;
            tx.commit().map_err(storage)?;
            Ok(n as u64)
        })();
        self.conn
            .execute_batch("DETACH DATABASE installing")
            .map_err(storage)?;
        result
    }

    /// Adds the citation counts of the link rows selected by `filter` (a predicate over
    /// `link` with one parameter).
    fn add_counts(conn: &Connection, filter: &str, param: i64) -> Result<()> {
        conn.execute(
            &format!(
                "INSERT INTO cited_count(dst_work, dst_locator, rel, n)
                 SELECT dst_work, dst_locator, rel, count(*) FROM link WHERE {filter}
                 GROUP BY dst_work, dst_locator, rel ORDER BY dst_work, dst_locator, rel
                 ON CONFLICT(dst_work, dst_locator, rel) DO UPDATE SET n = n + excluded.n"
            ),
            params![param],
        )
        .map_err(storage)?;
        conn.execute(
            &format!(
                "INSERT INTO cited_count(dst_work, dst_locator, rel, n)
                 SELECT dst_work, dst_locator, {ANY_REL}, count(*) FROM link WHERE {filter}
                 GROUP BY dst_work, dst_locator ORDER BY dst_work, dst_locator
                 ON CONFLICT(dst_work, dst_locator, rel) DO UPDATE SET n = n + excluded.n"
            ),
            params![param],
        )
        .map_err(storage)?;
        Ok(())
    }

    /// Subtracts the citation counts of the link rows selected by `filter`.
    fn subtract_counts(conn: &Connection, filter: &str, param: i64) -> Result<()> {
        conn.execute(
            &format!(
                "UPDATE cited_count SET n = cited_count.n - c.n FROM (
                     SELECT dst_work, dst_locator, rel, count(*) AS n FROM link WHERE {filter}
                     GROUP BY dst_work, dst_locator, rel) AS c
                 WHERE cited_count.dst_work = c.dst_work AND cited_count.dst_locator = c.dst_locator
                   AND cited_count.rel = c.rel"
            ),
            params![param],
        )
        .map_err(storage)?;
        conn.execute(
            &format!(
                "UPDATE cited_count SET n = cited_count.n - c.n FROM (
                     SELECT dst_work, dst_locator, count(*) AS n FROM link WHERE {filter}
                     GROUP BY dst_work, dst_locator) AS c
                 WHERE cited_count.dst_work = c.dst_work AND cited_count.dst_locator = c.dst_locator
                   AND cited_count.rel = {ANY_REL}"
            ),
            params![param],
        )
        .map_err(storage)?;
        conn.execute_batch("DELETE FROM cited_count WHERE n <= 0")
            .map_err(storage)?;
        Ok(())
    }

    /// Removes a pack's edges. Returns how many were removed.
    pub fn remove_pack(&mut self, pack_id: &str) -> Result<u64> {
        let row: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM pack WHERE pack_id = ?1",
                params![pack_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        let Some(row) = row else { return Ok(0) };
        let tx = self.conn.unchecked_transaction().map_err(storage)?;
        Self::subtract_counts(&tx, "pack = ?1", row)?;
        let n = tx
            .execute("DELETE FROM link WHERE pack = ?1", params![row])
            .map_err(storage)?;
        tx.execute("DELETE FROM pack WHERE id = ?1", params![row])
            .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(n as u64)
    }

    /// Adds one user-provenance edge from a note or collection item. The edge's provenance
    /// must be `user`; its `source_work` is the user database's name for the note's origin.
    pub fn add_user_edge(&mut self, user_edge: &UserEdge) -> Result<()> {
        let edge = &user_edge.edge;
        if edge.provenance != ProvenanceKind::User {
            return Err(RetrievalError::InvalidQuery(
                "a user edge must carry provenance `user`".into(),
            ));
        }
        let tx = self.conn.unchecked_transaction().map_err(storage)?;
        for name in [&edge.src.work, &edge.dst.work, &edge.source_work] {
            tx.execute(
                "INSERT OR IGNORE INTO work(name) VALUES (?1)",
                params![name],
            )
            .map_err(storage)?;
        }
        tx.execute(
            "INSERT OR IGNORE INTO rel(name) VALUES (?1)",
            params![edge.rel],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO link(src_work, src_locator, rel, dst_work, dst_locator, weight, provenance,
                              source_work, method, review_state, context, pack, user_source)
             VALUES ((SELECT id FROM work WHERE name = ?1), ?2, (SELECT id FROM rel WHERE name = ?3),
                     (SELECT id FROM work WHERE name = ?4), ?5, ?6, ?7, (SELECT id FROM work WHERE name = ?8),
                     ?9, ?10, ?11, NULL, ?12)",
            params![
                edge.src.work,
                edge.src.locator,
                edge.rel,
                edge.dst.work,
                edge.dst.locator,
                f64::from(edge.weight),
                provenance_code(ProvenanceKind::User),
                edge.source_work,
                edge.method,
                review_code(edge.review_state),
                user_edge.context,
                user_edge.source
            ],
        )
        .map_err(storage)?;
        let rowid = tx.last_insert_rowid();
        Self::add_counts(&tx, "rowid = ?1", rowid)?;
        tx.commit().map_err(storage)?;
        Ok(())
    }

    /// Removes every user edge written by `source`. Returns how many were removed.
    pub fn remove_user_edges(&mut self, source: &str) -> Result<u64> {
        let tx = self.conn.unchecked_transaction().map_err(storage)?;
        let rowids: Vec<i64> = {
            let mut stmt = tx
                .prepare("SELECT rowid FROM link WHERE user_source = ?1")
                .map_err(storage)?;
            let rows = stmt
                .query_map(params![source], |r| r.get(0))
                .map_err(storage)?;
            rows.collect::<std::result::Result<_, _>>()
                .map_err(storage)?
        };
        for rowid in &rowids {
            Self::subtract_counts(&tx, "rowid = ?1", *rowid)?;
        }
        let n = tx
            .execute("DELETE FROM link WHERE user_source = ?1", params![source])
            .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(n as u64)
    }

    /// Every user edge, sorted by source then edge fields (the snapshot a rebuild keeps).
    pub fn user_edges(&self) -> Result<Vec<UserEdge>> {
        let sql = format!(
            "SELECT {EDGE_COLUMNS}, l.context, l.user_source {EDGE_FROM} WHERE l.user_source IS NOT NULL
             ORDER BY l.user_source, sw.name, l.src_locator, r.name, dw.name, l.dst_locator, l.weight,
                      ow.name, l.method, l.review_state, l.context"
        );
        let mut stmt = self.conn.prepare(&sql).map_err(storage)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(UserEdge {
                    edge: edge_from_row(r)?,
                    context: r.get(10)?,
                    source: r.get(11)?,
                })
            })
            .map_err(storage)?;
        rows.collect::<std::result::Result<_, _>>().map_err(storage)
    }

    /// Drops everything and installs the given packs in pack-id order, keeping the user
    /// edges the index held (re-added after the packs, in sorted order). Returns the number
    /// of pack edges merged.
    pub fn rebuild(&mut self, packs: &[(String, std::path::PathBuf)]) -> Result<u64> {
        let kept = self.user_edges()?;
        self.rebuild_with(packs, &kept)
    }

    fn rebuild_with(
        &mut self,
        packs: &[(String, std::path::PathBuf)],
        user_edges: &[UserEdge],
    ) -> Result<u64> {
        self.conn
            .execute_batch(
                "DELETE FROM link; DELETE FROM pack; DELETE FROM work; DELETE FROM rel; DELETE FROM cited_count;",
            )
            .map_err(storage)?;
        let mut ordered: Vec<&(String, std::path::PathBuf)> = packs.iter().collect();
        ordered.sort_by(|a, b| a.0.cmp(&b.0));
        let mut total = 0;
        for (id, path) in ordered {
            total += self.install_pack(id, path)?;
        }
        let mut sorted = user_edges.to_vec();
        sorted.sort_by(|a, b| {
            (
                &a.source,
                &a.edge.src,
                &a.edge.rel,
                &a.edge.dst,
                &a.edge.source_work,
            )
                .cmp(&(
                    &b.source,
                    &b.edge.src,
                    &b.edge.rel,
                    &b.edge.dst,
                    &b.edge.source_work,
                ))
                .then_with(|| a.edge.weight.total_cmp(&b.edge.weight))
                .then_with(|| a.context.cmp(&b.context))
        });
        for user_edge in &sorted {
            self.add_user_edge(user_edge)?;
        }
        Ok(total)
    }

    /// Installed packs with their edge counts.
    pub fn installed(&self) -> Result<Vec<(String, u64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT pack_id, edges FROM pack ORDER BY pack_id")
            .map_err(storage)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
            })
            .map_err(storage)?;
        rows.collect::<std::result::Result<_, _>>().map_err(storage)
    }

    /// Total edges in the index, user edges included.
    pub fn edge_count(&self) -> Result<u64> {
        self.conn
            .query_row("SELECT count(*) FROM link", [], |r| r.get::<_, i64>(0))
            .map(|n| n as u64)
            .map_err(storage)
    }

    /// Rows in the citation count table (per relation plus one "any" row per destination).
    pub fn count_rows(&self) -> Result<u64> {
        self.conn
            .query_row("SELECT count(*) FROM cited_count", [], |r| {
                r.get::<_, i64>(0)
            })
            .map(|n| n as u64)
            .map_err(storage)
    }

    /// Outbound edges of a locator with the citing context each row carries, by descending
    /// weight; the input of "cited, not in your library" displays.
    pub fn outlinks_with_context(
        &self,
        source: &Locator,
        limit: usize,
    ) -> Result<Vec<(Edge, Option<String>)>> {
        let Some(work) = self.work_id(&source.work)? else {
            return Ok(Vec::new());
        };
        let sql = format!(
            "SELECT {EDGE_COLUMNS}, l.context {EDGE_FROM} WHERE l.src_work = ?1 AND l.src_locator = ?2
             ORDER BY l.weight DESC, l.rowid LIMIT ?3"
        );
        let mut stmt = self.conn.prepare_cached(&sql).map_err(storage)?;
        let rows = stmt
            .query_map(params![work, source.locator, limit as i64], |r| {
                Ok((edge_from_row(r)?, r.get::<_, Option<String>>(10)?))
            })
            .map_err(storage)?;
        rows.collect::<std::result::Result<_, _>>().map_err(storage)
    }

    fn work_id(&self, name: &str) -> Result<Option<i64>> {
        self.conn
            .query_row("SELECT id FROM work WHERE name = ?1", params![name], |r| {
                r.get(0)
            })
            .optional()
            .map_err(storage)
    }

    fn rel_id(&self, name: Option<&str>) -> Result<Option<Option<i64>>> {
        match name {
            None => Ok(Some(None)),
            Some(n) => Ok(self
                .conn
                .query_row("SELECT id FROM rel WHERE name = ?1", params![n], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(storage)?
                .map(Some)),
        }
    }

    fn edges(
        &self,
        direction: Direction,
        node: &Locator,
        rel: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Edge>> {
        let Some(work) = self.work_id(&node.work)? else {
            return Ok(Vec::new());
        };
        let Some(rel) = self.rel_id(rel)? else {
            return Ok(Vec::new());
        };
        let column = match direction {
            Direction::Inbound => "dst",
            Direction::Outbound => "src",
            Direction::Both => {
                return Err(RetrievalError::InvalidQuery(
                    "edges() takes one direction".into(),
                ));
            }
        };
        let sql = format!(
            "SELECT {EDGE_COLUMNS} {EDGE_FROM} WHERE l.{column}_work = ?1 AND l.{column}_locator = ?2 AND (?3 IS NULL OR l.rel = ?3)
             ORDER BY l.weight DESC, l.rowid LIMIT ?4"
        );
        let mut stmt = self.conn.prepare_cached(&sql).map_err(storage)?;
        let rows = stmt
            .query_map(
                params![work, node.locator, rel, limit as i64],
                edge_from_row,
            )
            .map_err(storage)?;
        rows.collect::<std::result::Result<_, _>>().map_err(storage)
    }

    /// Number of distinct nodes within `depth` hops following `rel` outbound, computed by one
    /// recursive CTE without fan-out caps. The bench compares it with [`RelationStore::traverse`].
    pub fn reach_count_recursive(
        &self,
        start: &Locator,
        rel: Option<&str>,
        depth: u8,
    ) -> Result<u64> {
        let Some(work) = self.work_id(&start.work)? else {
            return Ok(0);
        };
        let Some(rel) = self.rel_id(rel)? else {
            return Ok(0);
        };
        self.conn
            .query_row(
                "WITH RECURSIVE reach(work, locator, depth) AS (
                     SELECT ?1, ?2, 0
                     UNION
                     SELECT l.dst_work, l.dst_locator, reach.depth + 1 FROM reach
                     JOIN link l ON l.src_work = reach.work AND l.src_locator = reach.locator
                     WHERE reach.depth < ?3 AND (?4 IS NULL OR l.rel = ?4)
                 )
                 SELECT count(DISTINCT work || ':' || locator) - 1 FROM reach",
                params![work, start.locator, i64::from(depth), rel],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n.max(0) as u64)
            .map_err(storage)
    }
}

/// The frontier-side node of an edge for a traversal direction.
fn from_of(edge: &Edge, direction: Direction) -> Locator {
    match direction {
        Direction::Outbound => edge.src.clone(),
        _ => edge.dst.clone(),
    }
}

/// The far node of an edge for a traversal direction.
fn other_of(edge: &Edge, direction: Direction) -> Locator {
    match direction {
        Direction::Outbound => edge.dst.clone(),
        _ => edge.src.clone(),
    }
}

impl LinkIndex {
    /// Fills the temporary `frontier` table with the nodes to expand in one hop, so one
    /// statement per (relation, direction) serves the whole hop instead of one per node.
    fn load_frontier(&self, frontier: &[Locator]) -> Result<()> {
        self.conn
            .execute_batch(
                "CREATE TEMP TABLE IF NOT EXISTS frontier(work INTEGER NOT NULL, locator TEXT NOT NULL,
                     PRIMARY KEY(work, locator)) WITHOUT ROWID;
                 DELETE FROM frontier;",
            )
            .map_err(storage)?;
        let mut insert = self
            .conn
            .prepare_cached("INSERT OR IGNORE INTO frontier(work, locator) VALUES (?1, ?2)")
            .map_err(storage)?;
        let mut works: HashMap<&str, Option<i64>> = HashMap::new();
        for node in frontier {
            let id = match works.get(node.work.as_str()) {
                Some(id) => *id,
                None => {
                    let id = self.work_id(&node.work)?;
                    works.insert(node.work.as_str(), id);
                    id
                }
            };
            if let Some(id) = id {
                insert.execute(params![id, node.locator]).map_err(storage)?;
            }
        }
        Ok(())
    }

    /// The edges leaving (or entering) the frontier nodes, at most `fan_out` per node by
    /// descending weight (`row_number()` over a partition per node), with the frontier node
    /// of each edge. `CROSS JOIN` pins the frontier as the outer loop: without it the planner,
    /// which has no statistics for a temp table, may scan all of `link` and probe the frontier
    /// (measured in B064 as 1.3 s per hop against 1 ms).
    fn expand_frontier(
        &self,
        direction: Direction,
        rel: Option<i64>,
        fan_out: usize,
    ) -> Result<Vec<(Locator, Edge)>> {
        let column = match direction {
            Direction::Inbound => "dst",
            _ => "src",
        };
        let sql = format!(
            "SELECT sw.name, x.src_locator, r.name, dw.name, x.dst_locator, x.weight, x.provenance, ow.name,
                    x.method, x.review_state FROM (
                 SELECT l.*, row_number() OVER (PARTITION BY l.{column}_work, l.{column}_locator ORDER BY l.weight DESC, l.rowid) AS rn
                 FROM frontier f CROSS JOIN link l ON l.{column}_work = f.work AND l.{column}_locator = f.locator
                 WHERE (?1 IS NULL OR l.rel = ?1)
             ) x JOIN work sw ON sw.id = x.src_work JOIN work dw ON dw.id = x.dst_work
             JOIN rel r ON r.id = x.rel JOIN work ow ON ow.id = x.source_work
             WHERE ?2 = 0 OR x.rn <= ?2
             ORDER BY x.{column}_work, x.{column}_locator, x.rn"
        );
        let mut stmt = self.conn.prepare_cached(&sql).map_err(storage)?;
        let rows = stmt
            .query_map(params![rel, fan_out as i64], |r| {
                let edge = edge_from_row(r)?;
                Ok((from_of(&edge, direction), edge))
            })
            .map_err(storage)?;
        rows.collect::<std::result::Result<_, _>>().map_err(storage)
    }

    /// Relation ids for a list of names; `None` when every named relation is unknown to the
    /// index (nothing to traverse), `vec![None]` for "any".
    fn rel_ids(&self, rels: &[String]) -> Result<Option<Vec<Option<i64>>>> {
        if rels.is_empty() {
            return Ok(Some(vec![None]));
        }
        let mut ids = Vec::new();
        for r in rels {
            if let Some(id) = self.rel_id(Some(r))? {
                ids.push(id);
            }
        }
        Ok((!ids.is_empty()).then_some(ids))
    }
}

fn directions(direction: Direction) -> &'static [Direction] {
    match direction {
        Direction::Outbound => &[Direction::Outbound],
        Direction::Inbound => &[Direction::Inbound],
        Direction::Both => &[Direction::Outbound, Direction::Inbound],
    }
}

impl RelationStore for LinkIndex {
    fn backlinks(&self, target: &Locator, rel: Option<&str>, limit: usize) -> Result<Vec<Edge>> {
        self.edges(Direction::Inbound, target, rel, limit)
    }

    fn backlink_count(&self, target: &Locator, rel: Option<&str>) -> Result<u64> {
        let Some(work) = self.work_id(&target.work)? else {
            return Ok(0);
        };
        let Some(rel) = self.rel_id(rel)? else {
            return Ok(0);
        };
        self.conn
            .query_row(
                "SELECT n FROM cited_count WHERE dst_work = ?1 AND dst_locator = ?2 AND rel = ?3",
                params![work, target.locator, rel.unwrap_or(ANY_REL)],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .map(|n| n.unwrap_or(0) as u64)
            .map_err(storage)
    }

    fn outlinks(&self, source: &Locator, rel: Option<&str>, limit: usize) -> Result<Vec<Edge>> {
        self.edges(Direction::Outbound, source, rel, limit)
    }

    fn traverse(&self, start: &Locator, options: &TraversalOptions) -> Result<Vec<Reached>> {
        if options.max_depth == 0 || options.max_depth > 6 {
            return Err(RetrievalError::InvalidQuery(
                "max_depth must be 1..=6".into(),
            ));
        }
        let fan_out = options.fan_out as usize;
        let Some(rels) = self.rel_ids(&options.rels)? else {
            return Ok(Vec::new());
        };
        let directions = directions(options.direction);
        let mut visited: HashSet<Locator> = HashSet::new();
        visited.insert(start.clone());
        let mut reached: Vec<Reached> = Vec::new();
        let mut paths: HashMap<Locator, Vec<Edge>> = HashMap::new();
        paths.insert(start.clone(), Vec::new());
        let mut frontier: Vec<Locator> = vec![start.clone()];
        for depth in 1..=options.max_depth {
            let mut next: Vec<Locator> = Vec::new();
            // Edges expanded per frontier node in this hop, across relations and directions.
            let mut expanded: HashMap<Locator, usize> = HashMap::new();
            self.load_frontier(&frontier)?;
            'hop: for rel in &rels {
                for direction in directions {
                    for (from, edge) in self.expand_frontier(*direction, *rel, fan_out)? {
                        let used = expanded.entry(from).or_insert(0);
                        if fan_out != 0 && *used >= fan_out {
                            continue;
                        }
                        *used += 1;
                        let other = other_of(&edge, *direction);
                        if !visited.insert(other.clone()) {
                            continue; // cycle or second path: counted once
                        }
                        let mut path = paths
                            .get(&from_of(&edge, *direction))
                            .cloned()
                            .unwrap_or_default();
                        path.push(edge);
                        paths.insert(other.clone(), path.clone());
                        reached.push(Reached {
                            node: other.clone(),
                            depth,
                            path,
                        });
                        if reached.len() >= options.max_nodes {
                            break 'hop;
                        }
                        next.push(other);
                    }
                }
            }
            if reached.len() >= options.max_nodes || next.is_empty() {
                break;
            }
            frontier = next;
        }
        Ok(reached)
    }

    fn most_cited(
        &self,
        work: &str,
        rel: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Locator, u64)>> {
        let Some(work_id) = self.work_id(work)? else {
            return Ok(Vec::new());
        };
        let Some(rel) = self.rel_id(rel)? else {
            return Ok(Vec::new());
        };
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT dst_locator, n FROM cited_count WHERE dst_work = ?1 AND rel = ?2
                 ORDER BY n DESC, dst_locator LIMIT ?3",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map(
                params![work_id, rel.unwrap_or(ANY_REL), limit as i64],
                |r| {
                    Ok((
                        Locator {
                            work: work.to_owned(),
                            locator: r.get(0)?,
                        },
                        r.get::<_, i64>(1)? as u64,
                    ))
                },
            )
            .map_err(storage)?;
        rows.collect::<std::result::Result<_, _>>().map_err(storage)
    }

    fn citation_chains(
        &self,
        start: &Locator,
        options: &ChainOptions,
    ) -> Result<Vec<QuotationChain>> {
        if options.max_depth == 0 || options.max_depth > 6 {
            return Err(RetrievalError::InvalidQuery(
                "max_depth must be 1..=6".into(),
            ));
        }
        let rel_names: Vec<String> = if options.rels.is_empty() {
            vec!["quotes".to_owned()]
        } else {
            options.rels.clone()
        };
        let Some(rels) = self.rel_ids(&rel_names)? else {
            return Ok(Vec::new());
        };
        let fan_out = options.fan_out as usize;
        let directions = directions(options.direction);
        let mut visited: HashSet<Locator> = HashSet::new();
        visited.insert(start.clone());
        let mut chains: Vec<QuotationChain> = Vec::new();
        // Index into `chains` per end node, to attach a stop later.
        let mut chain_of: HashMap<Locator, usize> = HashMap::new();
        let mut hops_to: HashMap<Locator, Vec<ChainHop>> = HashMap::new();
        hops_to.insert(start.clone(), Vec::new());
        let mut frontier: Vec<Locator> = vec![start.clone()];
        let mut node_limit_hit = false;
        for _depth in 1..=options.max_depth {
            let mut next: Vec<Locator> = Vec::new();
            let mut expanded: HashMap<Locator, usize> = HashMap::new();
            self.load_frontier(&frontier)?;
            'hop: for rel in &rels {
                for direction in directions {
                    for (from, edge) in self.expand_frontier(*direction, *rel, fan_out)? {
                        let used = expanded.entry(from.clone()).or_insert(0);
                        if fan_out != 0 && *used >= fan_out {
                            continue;
                        }
                        *used += 1;
                        if !edge.basis_documented() {
                            // The chain stops at `from`: the hop exists but states no basis.
                            let stop = ChainStop::BasisNotDocumented { edge };
                            match chain_of.get(&from) {
                                Some(i) if chains[*i].stop.is_none() => {
                                    chains[*i].stop = Some(stop);
                                }
                                Some(_) => {}
                                None => {
                                    // The start node itself: an empty chain that stops.
                                    chain_of.insert(from.clone(), chains.len());
                                    chains.push(QuotationChain {
                                        end: from.clone(),
                                        hops: Vec::new(),
                                        stop: Some(stop),
                                    });
                                }
                            }
                            continue;
                        }
                        let other = other_of(&edge, *direction);
                        if !visited.insert(other.clone()) {
                            continue; // cycle or second path: counted once
                        }
                        let mut hops = hops_to.get(&from).cloned().unwrap_or_default();
                        hops.push(ChainHop::from_edge(&edge, *direction));
                        hops_to.insert(other.clone(), hops.clone());
                        chain_of.insert(other.clone(), chains.len());
                        chains.push(QuotationChain {
                            end: other.clone(),
                            hops,
                            stop: None,
                        });
                        if chains.len() >= options.max_nodes {
                            node_limit_hit = true;
                            break 'hop;
                        }
                        next.push(other);
                    }
                }
            }
            if node_limit_hit {
                // Chains whose end was reached in this hop, and frontier nodes never
                // expanded, stop at the node limit.
                for node in frontier.iter().filter(|n| !expanded.contains_key(*n)) {
                    if let Some(i) = chain_of.get(node)
                        && chains[*i].stop.is_none()
                    {
                        chains[*i].stop = Some(ChainStop::NodeLimit);
                    }
                }
                for node in &next {
                    if let Some(i) = chain_of.get(node)
                        && chains[*i].stop.is_none()
                    {
                        chains[*i].stop = Some(ChainStop::NodeLimit);
                    }
                }
                break;
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }
        if !node_limit_hit && frontier.first().is_some_and(|f| f != start) {
            // The leaves at the depth limit: those with a further documented hop stop there,
            // and an undocumented next hop is still reported.
            self.load_frontier(&frontier)?;
            for rel in &rels {
                for direction in directions {
                    for (from, edge) in self.expand_frontier(*direction, *rel, 0)? {
                        let Some(i) = chain_of.get(&from).copied() else {
                            continue;
                        };
                        if chains[i].stop.is_some() {
                            continue;
                        }
                        if !edge.basis_documented() {
                            chains[i].stop = Some(ChainStop::BasisNotDocumented { edge });
                        } else if !visited.contains(&other_of(&edge, *direction)) {
                            chains[i].stop = Some(ChainStop::DepthLimit);
                        }
                    }
                }
            }
        }
        Ok(chains)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::fixture::temp_path;
    use crate::pack::{EdgeRow, PackBuilder, PackManifest};

    fn manifest(id: &str) -> PackManifest {
        PackManifest {
            edition: id.into(),
            scheme: "org".into(),
            source: "test".into(),
            license: "CC0-1.0".into(),
            attribution: String::new(),
            work: None,
            locator_scheme: None,
        }
    }

    /// Pack A: a → b, b → c, c → a (cycle), a → d; pack B: x → b, d → e.
    fn two_packs() -> (std::path::PathBuf, std::path::PathBuf) {
        let a = temp_path("links-a");
        let b = a.with_file_name("b.sqlite");
        let edges_a = [
            ("a", "b", 2.0),
            ("b", "c", 1.0),
            ("c", "a", 1.0),
            ("a", "d", 1.0),
        ];
        let edges_b = [("x", "b", 1.0), ("d", "e", 1.0)];
        for (path, id, edges) in [(&a, "pack-a", &edges_a[..]), (&b, "pack-b", &edges_b[..])] {
            let mut builder = PackBuilder::create(path, &manifest(id), &[]).unwrap();
            for (s, d, w) in edges {
                let context = format!("{s} cites {d}");
                builder
                    .add_edge(&EdgeRow {
                        src_work: "w",
                        src_locator: s,
                        rel: "cites",
                        dst_work: "w",
                        dst_locator: d,
                        weight: *w,
                        source_work: id,
                        method: "test",
                        provenance: ProvenanceKind::Curated,
                        review_state: ReviewState::SourceAttested,
                        context: Some(&context),
                    })
                    .unwrap();
            }
            builder.finish().unwrap();
        }
        (a, b)
    }

    fn node(l: &str) -> Locator {
        Locator {
            work: "w".into(),
            locator: l.into(),
        }
    }

    fn user_edge(source: &str, from: &str, to: &str) -> UserEdge {
        UserEdge {
            source: source.into(),
            edge: Edge {
                src: Locator {
                    work: "notes".into(),
                    locator: from.into(),
                },
                rel: "cites".into(),
                dst: node(to),
                weight: 0.0,
                provenance: ProvenanceKind::User,
                source_work: "user-db".into(),
                method: "note".into(),
                review_state: ReviewState::Published,
            },
            context: Some("my note".into()),
        }
    }

    #[test]
    fn install_remove_and_rebuild_keep_counts_consistent() {
        let (a, b) = two_packs();
        let mut index = LinkIndex::in_memory().unwrap();
        assert_eq!(index.install_pack("pack-a", &a).unwrap(), 4);
        assert_eq!(index.install_pack("pack-b", &b).unwrap(), 2);
        assert_eq!(index.backlink_count(&node("b"), None).unwrap(), 2);
        assert_eq!(index.backlink_count(&node("b"), Some("cites")).unwrap(), 2);
        assert_eq!(index.backlink_count(&node("b"), Some("quotes")).unwrap(), 0);
        // Reinstalling the same pack does not double its edges or its counts.
        assert_eq!(index.install_pack("pack-b", &b).unwrap(), 2);
        assert_eq!(index.edge_count().unwrap(), 6);
        assert_eq!(index.backlink_count(&node("b"), None).unwrap(), 2);
        assert_eq!(index.remove_pack("pack-b").unwrap(), 2);
        assert_eq!(index.backlink_count(&node("b"), None).unwrap(), 1);
        assert_eq!(index.backlink_count(&node("e"), None).unwrap(), 0);
        assert_eq!(
            index
                .rebuild(&[("pack-b".into(), b.clone()), ("pack-a".into(), a.clone())])
                .unwrap(),
            6
        );
        assert_eq!(
            index.installed().unwrap(),
            vec![("pack-a".to_owned(), 4), ("pack-b".to_owned(), 2)]
        );
        let most = index.most_cited("w", None, 2).unwrap();
        assert_eq!(most[0], (node("b"), 2));
        assert_eq!(most[1].1, 1);
        // The count table agrees with a direct count on every destination.
        let direct: Vec<(String, i64)> = {
            let mut stmt = index
                .conn
                .prepare("SELECT dst_locator, count(*) FROM link GROUP BY dst_locator ORDER BY 1")
                .unwrap();
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .collect::<std::result::Result<_, _>>()
                .unwrap()
        };
        for (locator, n) in direct {
            assert_eq!(
                index.backlink_count(&node(&locator), None).unwrap(),
                n as u64
            );
        }
        // cites + any per destination: 5 destinations × 2 rows.
        assert_eq!(index.count_rows().unwrap(), 10);
    }

    #[test]
    fn backlinks_carry_the_edge_attribution() {
        let (a, b) = two_packs();
        let mut index = LinkIndex::in_memory().unwrap();
        index.install_pack("pack-a", &a).unwrap();
        index.install_pack("pack-b", &b).unwrap();
        let links = index.backlinks(&node("b"), None, 10).unwrap();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].src, node("a")); // weight 2 first
        assert_eq!(links[0].source_work, "pack-a");
        assert_eq!(links[0].provenance, ProvenanceKind::Curated);
        assert_eq!(links[0].method, "test");
        assert_eq!(links[0].review_state, ReviewState::SourceAttested);
        assert_eq!(links[1].src, node("x"));
        assert_eq!(index.outlinks(&node("a"), None, 10).unwrap().len(), 2);
        let with_context = index.outlinks_with_context(&node("a"), 10).unwrap();
        assert_eq!(with_context[0].1.as_deref(), Some("a cites b"));
    }

    #[test]
    fn traversal_deduplicates_nodes_detects_cycles_and_caps_fan_out() {
        let (a, b) = two_packs();
        let mut index = LinkIndex::in_memory().unwrap();
        index.install_pack("pack-a", &a).unwrap();
        index.install_pack("pack-b", &b).unwrap();
        let options = TraversalOptions {
            rels: vec![],
            direction: Direction::Outbound,
            max_depth: 6,
            fan_out: 0,
            max_nodes: 100,
        };
        let reached = index.traverse(&node("a"), &options).unwrap();
        // a → b, d (depth 1); b → c, d → e (depth 2); c → a is a cycle and is not reported.
        let mut names: Vec<(String, u8)> = reached
            .iter()
            .map(|r| (r.node.locator.clone(), r.depth))
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                ("b".into(), 1),
                ("c".into(), 2),
                ("d".into(), 1),
                ("e".into(), 2)
            ]
        );
        let e = reached.iter().find(|r| r.node.locator == "e").unwrap();
        assert_eq!(e.path.len(), 2);
        assert_eq!(e.path[0].dst, node("d"));
        // The recursive CTE agrees on the reachable count.
        assert_eq!(index.reach_count_recursive(&node("a"), None, 6).unwrap(), 4);
        assert_eq!(index.reach_count_recursive(&node("a"), None, 1).unwrap(), 2);
        // A fan-out cap of 1 keeps only the heaviest edge of a: a → b.
        let capped = index
            .traverse(
                &node("a"),
                &TraversalOptions {
                    fan_out: 1,
                    max_depth: 1,
                    ..options.clone()
                },
            )
            .unwrap();
        assert_eq!(capped.len(), 1);
        assert_eq!(capped[0].node, node("b"));
        // Inbound traversal from b finds a and x, then c (c → a).
        let inbound = index
            .traverse(
                &node("b"),
                &TraversalOptions {
                    direction: Direction::Inbound,
                    max_depth: 2,
                    ..options.clone()
                },
            )
            .unwrap();
        let mut names: Vec<String> = inbound.iter().map(|r| r.node.locator.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["a", "c", "x"]);
        assert!(
            index
                .traverse(
                    &node("a"),
                    &TraversalOptions {
                        max_depth: 7,
                        ..options
                    }
                )
                .is_err()
        );
    }

    #[test]
    fn user_edges_show_in_backlinks_and_survive_a_rebuild() {
        let (a, b) = two_packs();
        let mut index = LinkIndex::in_memory().unwrap();
        index.install_pack("pack-a", &a).unwrap();
        index
            .add_user_edge(&user_edge("note-1", "n1", "b"))
            .unwrap();
        index
            .add_user_edge(&user_edge("note-2", "n2", "e"))
            .unwrap();
        // A non-user edge is refused on the user path.
        let mut not_user = user_edge("note-3", "n3", "b");
        not_user.edge.provenance = ProvenanceKind::Curated;
        assert!(index.add_user_edge(&not_user).is_err());
        let links = index.backlinks(&node("b"), None, 10).unwrap();
        assert_eq!(links.len(), 2);
        assert!(
            links
                .iter()
                .any(|e| e.provenance == ProvenanceKind::User && e.src.work == "notes")
        );
        assert_eq!(index.backlink_count(&node("b"), None).unwrap(), 2);
        assert_eq!(index.backlink_count(&node("e"), None).unwrap(), 1);
        // Rebuild from the packs: the user edges are still there, the counts still right.
        index
            .rebuild(&[("pack-a".into(), a.clone()), ("pack-b".into(), b.clone())])
            .unwrap();
        assert_eq!(index.edge_count().unwrap(), 8);
        assert_eq!(index.user_edges().unwrap().len(), 2);
        assert_eq!(index.backlink_count(&node("b"), None).unwrap(), 3);
        assert_eq!(index.backlink_count(&node("e"), None).unwrap(), 2);
        assert_eq!(index.most_cited("w", None, 1).unwrap()[0], (node("b"), 3));
        // Removing the note removes its edges and counts.
        assert_eq!(index.remove_user_edges("note-1").unwrap(), 1);
        assert_eq!(index.backlink_count(&node("b"), None).unwrap(), 2);
        assert_eq!(index.user_edges().unwrap()[0].source, "note-2");
    }

    #[test]
    fn a_fresh_build_is_byte_identical_for_the_same_installed_set() {
        let (a, b) = two_packs();
        let dir = a.parent().unwrap();
        let packs = vec![
            ("pack-b".to_owned(), b.clone()),
            ("pack-a".to_owned(), a.clone()),
        ];
        let user = vec![user_edge("note-1", "n1", "b")];
        let first = dir.join("links-1.sqlite");
        let second = dir.join("links-2.sqlite");
        assert_eq!(LinkIndex::build_fresh(&first, &packs, &user).unwrap(), 6);
        let reversed: Vec<_> = packs.iter().rev().cloned().collect();
        assert_eq!(
            LinkIndex::build_fresh(&second, &reversed, &user).unwrap(),
            6
        );
        let bytes_1 = std::fs::read(&first).unwrap();
        let bytes_2 = std::fs::read(&second).unwrap();
        assert!(!bytes_1.is_empty());
        assert_eq!(bytes_1, bytes_2, "two fresh builds differ");
        // A different installed set is a different file.
        let third = dir.join("links-3.sqlite");
        LinkIndex::build_fresh(&third, &packs[..1], &user).unwrap();
        assert_ne!(bytes_1, std::fs::read(&third).unwrap());
        // The built file opens and answers; the user edge is in it.
        let index = LinkIndex::open(&first).unwrap();
        assert_eq!(index.edge_count().unwrap(), 7);
        assert_eq!(index.backlink_count(&node("b"), None).unwrap(), 3);
    }

    /// Chains: w:1 quotes w:2 (curated), w:2 quotes w:3 (extracted, reviewed), w:3 quotes
    /// w:4 by model similarity only (relation `similar_to`, so not a quotation at all) and
    /// w:3 quotes w:5 with an empty method (basis not documented); w:5 quotes w:1 (cycle);
    /// w:2 quotes w:6 was rejected on review.
    fn chain_pack() -> std::path::PathBuf {
        let path = temp_path("links-chains");
        let mut b = PackBuilder::create(&path, &manifest("chains"), &[]).unwrap();
        let rows: &[(&str, &str, &str, &str, ProvenanceKind, ReviewState)] = &[
            (
                "1",
                "2",
                "quotes",
                "ccel-scripRef",
                ProvenanceKind::Curated,
                ReviewState::SourceAttested,
            ),
            (
                "2",
                "3",
                "quotes",
                "quote-parser@1",
                ProvenanceKind::Extracted,
                ReviewState::Reviewed,
            ),
            (
                "3",
                "4",
                "similar_to",
                "e5-small@1",
                ProvenanceKind::ModelSimilarity,
                ReviewState::Proposed,
            ),
            (
                "3",
                "5",
                "quotes",
                "",
                ProvenanceKind::Extracted,
                ReviewState::Proposed,
            ),
            (
                "5",
                "1",
                "quotes",
                "quote-parser@1",
                ProvenanceKind::Extracted,
                ReviewState::Reviewed,
            ),
            (
                "2",
                "6",
                "quotes",
                "quote-parser@1",
                ProvenanceKind::Extracted,
                ReviewState::Rejected,
            ),
        ];
        for (s, d, rel, method, provenance, state) in rows {
            b.add_edge(&EdgeRow {
                src_work: "w",
                src_locator: s,
                rel,
                dst_work: "w",
                dst_locator: d,
                weight: 1.0,
                source_work: "chains",
                method,
                provenance: *provenance,
                review_state: *state,
                context: None,
            })
            .unwrap();
        }
        b.finish().unwrap();
        path
    }

    #[test]
    fn chains_carry_provenance_and_stop_where_the_basis_is_not_documented() {
        let path = chain_pack();
        let mut index = LinkIndex::in_memory().unwrap();
        index.install_pack("chains", &path).unwrap();
        let chains = index
            .citation_chains(&node("1"), &ChainOptions::default())
            .unwrap();
        let mut ends: Vec<String> = chains.iter().map(|c| c.end.locator.clone()).collect();
        ends.sort();
        // 1 → 2 → 3; 3 → 5 has no documented basis, 2 → 6 is rejected, 3 → 4 is not a
        // quotation relation; so 4, 5 and 6 are never reached.
        assert_eq!(ends, vec!["2", "3"]);
        let to_3 = chains.iter().find(|c| c.end.locator == "3").unwrap();
        assert_eq!(to_3.depth(), 2);
        assert_eq!(to_3.hops[0].provenance, ProvenanceKind::Curated);
        assert_eq!(to_3.hops[0].method, "ccel-scripRef");
        assert_eq!(to_3.hops[0].review_state, ReviewState::SourceAttested);
        assert_eq!(to_3.hops[1].review_state, ReviewState::Reviewed);
        assert_eq!(to_3.hops[1].source_work, "chains");
        assert!(matches!(
            &to_3.stop,
            Some(ChainStop::BasisNotDocumented { edge }) if edge.dst == node("5")
        ));
        assert_eq!(
            to_3.stop.as_ref().unwrap().message(),
            "basis not documented"
        );
        let to_2 = chains.iter().find(|c| c.end.locator == "2").unwrap();
        assert!(matches!(
            &to_2.stop,
            Some(ChainStop::BasisNotDocumented { edge }) if edge.dst == node("6")
        ));
        // Depth limit: from 1 with one hop, the chain to 2 reports the limit since 2 → 3 is
        // a documented continuation (the rejected 2 → 6 ranks after it by rowid).
        let shallow = index
            .citation_chains(
                &node("1"),
                &ChainOptions {
                    max_depth: 1,
                    ..ChainOptions::default()
                },
            )
            .unwrap();
        assert_eq!(shallow.len(), 1);
        assert_eq!(shallow[0].stop, Some(ChainStop::DepthLimit));
        // Inbound from 1: 5 quotes 1 (documented); 3 → 5 is undocumented so it stops at 5.
        let inbound = index
            .citation_chains(
                &node("1"),
                &ChainOptions {
                    direction: Direction::Inbound,
                    ..ChainOptions::default()
                },
            )
            .unwrap();
        assert_eq!(inbound.len(), 1);
        assert_eq!(inbound[0].end, node("5"));
        assert_eq!(inbound[0].hops[0].from, node("1"));
        assert!(matches!(
            &inbound[0].stop,
            Some(ChainStop::BasisNotDocumented { .. })
        ));
        // Both ways from 2: 1 (inbound), 3 (outbound), then 5 through 5 → 1 once.
        let both = index
            .citation_chains(
                &node("2"),
                &ChainOptions {
                    direction: Direction::Both,
                    ..ChainOptions::default()
                },
            )
            .unwrap();
        let mut ends: Vec<String> = both.iter().map(|c| c.end.locator.clone()).collect();
        ends.sort();
        // The start itself appears as an empty chain because 2 → 6 states no basis.
        assert_eq!(ends, vec!["1", "2", "3", "5"]);
        let at_start = both.iter().find(|c| c.end.locator == "2").unwrap();
        assert!(at_start.hops.is_empty());
        assert!(
            matches!(&at_start.stop, Some(ChainStop::BasisNotDocumented { edge }) if edge.dst == node("6"))
        );
        assert!(
            index
                .citation_chains(
                    &node("1"),
                    &ChainOptions {
                        max_depth: 9,
                        ..ChainOptions::default()
                    }
                )
                .is_err()
        );
    }
}
