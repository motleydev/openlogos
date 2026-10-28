//! The device-local link index (specs/19, Cross-resource links, item 3): one mutable SQLite
//! database that merges the outbound edges of every installed pack, keyed by destination, so
//! "who cites this" never attaches hundreds of packs (SQLite allows at most 125). It is
//! updated per pack on install and removal and rebuilt deterministically from the installed
//! set. Counts it answers are library-relative.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use openlogos_retrieval::{
    Direction, Edge, Locator, ProvenanceKind, Reached, RelationStore, Result, RetrievalError,
    TraversalOptions,
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
    source_work INTEGER NOT NULL, pack INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS link_dst ON link(dst_work, dst_locator, rel);
CREATE INDEX IF NOT EXISTS link_src ON link(src_work, src_locator, rel);
CREATE INDEX IF NOT EXISTS link_pack ON link(pack);
";

const SELECT_EDGE: &str =
    "SELECT sw.name, l.src_locator, r.name, dw.name, l.dst_locator, l.weight, l.provenance, ow.name
    FROM link l JOIN work sw ON sw.id = l.src_work JOIN work dw ON dw.id = l.dst_work
    JOIN rel r ON r.id = l.rel JOIN work ow ON ow.id = l.source_work";

fn provenance_from_code(code: i64) -> ProvenanceKind {
    match code {
        0 => ProvenanceKind::Curated,
        1 => ProvenanceKind::Extracted,
        2 => ProvenanceKind::ModelSimilarity,
        _ => ProvenanceKind::User,
    }
}

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
    })
}

/// The link index database.
pub struct LinkIndex {
    conn: Connection,
}

impl LinkIndex {
    /// Opens or creates the index at `path`.
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

    /// The underlying connection (bench harness only).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Merges a pack's `edge` table into the index under `pack_id`, replacing any earlier
    /// edges of that pack. Returns the number of edges merged.
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
            tx.execute_batch(
                "INSERT OR IGNORE INTO work(name)
                    SELECT src_work FROM installing.edge UNION SELECT dst_work FROM installing.edge
                    UNION SELECT source_work FROM installing.edge;
                 INSERT OR IGNORE INTO rel(name) SELECT DISTINCT rel FROM installing.edge;",
            )
            .map_err(storage)?;
            let n = tx
                .execute(
                    "INSERT INTO link(src_work, src_locator, rel, dst_work, dst_locator, weight, provenance, source_work, pack)
                     SELECT sw.id, e.src_locator, r.id, dw.id, e.dst_locator, e.weight,
                            CASE e.provenance_kind WHEN 'curated' THEN 0 WHEN 'extracted' THEN 1
                                 WHEN 'model_similarity' THEN 2 ELSE 3 END,
                            ow.id, ?1
                     FROM installing.edge e
                     JOIN work sw ON sw.name = e.src_work JOIN work dw ON dw.name = e.dst_work
                     JOIN rel r ON r.name = e.rel JOIN work ow ON ow.name = e.source_work",
                    params![pack_row],
                )
                .map_err(storage)?;
            tx.execute(
                "UPDATE pack SET edges = ?1 WHERE id = ?2",
                params![n as i64, pack_row],
            )
            .map_err(storage)?;
            tx.commit().map_err(storage)?;
            Ok(n as u64)
        })();
        self.conn
            .execute_batch("DETACH DATABASE installing")
            .map_err(storage)?;
        result
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
        let n = tx
            .execute("DELETE FROM link WHERE pack = ?1", params![row])
            .map_err(storage)?;
        tx.execute("DELETE FROM pack WHERE id = ?1", params![row])
            .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(n as u64)
    }

    /// Drops everything and installs the given packs in order.
    pub fn rebuild(&mut self, packs: &[(String, std::path::PathBuf)]) -> Result<u64> {
        self.conn
            .execute_batch("DELETE FROM link; DELETE FROM pack; DELETE FROM work; DELETE FROM rel;")
            .map_err(storage)?;
        let mut total = 0;
        for (id, path) in packs {
            total += self.install_pack(id, path)?;
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

    /// Total edges in the index.
    pub fn edge_count(&self) -> Result<u64> {
        self.conn
            .query_row("SELECT count(*) FROM link", [], |r| r.get::<_, i64>(0))
            .map(|n| n as u64)
            .map_err(storage)
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
            "{SELECT_EDGE} WHERE l.{column}_work = ?1 AND l.{column}_locator = ?2 AND (?3 IS NULL OR l.rel = ?3)
             ORDER BY l.weight DESC LIMIT ?4"
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
                "SELECT count(*) FROM link WHERE dst_work = ?1 AND dst_locator = ?2 AND (?3 IS NULL OR rel = ?3)",
                params![work, target.locator, rel],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n as u64)
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
        let fan_out = if options.fan_out == 0 {
            usize::MAX
        } else {
            options.fan_out as usize
        };
        let rels: Vec<Option<&str>> = if options.rels.is_empty() {
            vec![None]
        } else {
            options.rels.iter().map(|r| Some(r.as_str())).collect()
        };
        let directions: &[Direction] = match options.direction {
            Direction::Outbound => &[Direction::Outbound],
            Direction::Inbound => &[Direction::Inbound],
            Direction::Both => &[Direction::Outbound, Direction::Inbound],
        };
        let mut visited: HashSet<Locator> = HashSet::new();
        visited.insert(start.clone());
        let mut reached: Vec<Reached> = Vec::new();
        let mut paths: HashMap<Locator, Vec<Edge>> = HashMap::new();
        paths.insert(start.clone(), Vec::new());
        let mut frontier: VecDeque<Locator> = VecDeque::new();
        frontier.push_back(start.clone());
        for depth in 1..=options.max_depth {
            let mut next: VecDeque<Locator> = VecDeque::new();
            while let Some(node) = frontier.pop_front() {
                let base = paths.get(&node).cloned().unwrap_or_default();
                let mut expanded = 0usize;
                'rels: for rel in &rels {
                    for direction in directions {
                        let edges =
                            self.edges(*direction, &node, *rel, fan_out.saturating_sub(expanded))?;
                        for edge in edges {
                            if expanded >= fan_out {
                                break 'rels;
                            }
                            expanded += 1;
                            let other = match direction {
                                Direction::Outbound => edge.dst.clone(),
                                _ => edge.src.clone(),
                            };
                            if !visited.insert(other.clone()) {
                                continue; // cycle or second path: counted once
                            }
                            let mut path = base.clone();
                            path.push(edge);
                            paths.insert(other.clone(), path.clone());
                            reached.push(Reached {
                                node: other.clone(),
                                depth,
                                path,
                            });
                            if reached.len() >= options.max_nodes {
                                return Ok(reached);
                            }
                            next.push_back(other);
                        }
                    }
                }
            }
            frontier = next;
            if frontier.is_empty() {
                break;
            }
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
                "SELECT dst_locator, count(*) AS n FROM link WHERE dst_work = ?1 AND (?2 IS NULL OR rel = ?2)
                 GROUP BY dst_locator ORDER BY n DESC, dst_locator LIMIT ?3",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![work_id, rel, limit as i64], |r| {
                Ok((
                    Locator {
                        work: work.to_owned(),
                        locator: r.get(0)?,
                    },
                    r.get::<_, i64>(1)? as u64,
                ))
            })
            .map_err(storage)?;
        rows.collect::<std::result::Result<_, _>>().map_err(storage)
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

    #[test]
    fn install_remove_and_rebuild_keep_counts_consistent() {
        let (a, b) = two_packs();
        let mut index = LinkIndex::in_memory().unwrap();
        assert_eq!(index.install_pack("pack-a", &a).unwrap(), 4);
        assert_eq!(index.install_pack("pack-b", &b).unwrap(), 2);
        assert_eq!(index.backlink_count(&node("b"), None).unwrap(), 2);
        assert_eq!(index.backlink_count(&node("b"), Some("cites")).unwrap(), 2);
        assert_eq!(index.backlink_count(&node("b"), Some("quotes")).unwrap(), 0);
        // Reinstalling the same pack does not double its edges.
        assert_eq!(index.install_pack("pack-b", &b).unwrap(), 2);
        assert_eq!(index.edge_count().unwrap(), 6);
        assert_eq!(index.remove_pack("pack-b").unwrap(), 2);
        assert_eq!(index.backlink_count(&node("b"), None).unwrap(), 1);
        assert_eq!(
            index
                .rebuild(&[("pack-a".into(), a.clone()), ("pack-b".into(), b.clone())])
                .unwrap(),
            6
        );
        assert_eq!(
            index.installed().unwrap(),
            vec![("pack-a".to_owned(), 4), ("pack-b".to_owned(), 2)]
        );
        let most = index.most_cited("w", None, 2).unwrap();
        assert_eq!(most[0], (node("b"), 2));
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
        assert_eq!(links[1].src, node("x"));
        assert_eq!(index.outlinks(&node("a"), None, 10).unwrap().len(), 2);
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
}
