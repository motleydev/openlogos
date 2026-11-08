//! Coded edge storage (B057): an opt-in physical layout for a pack whose `edge` rows are many
//! and alike, such as a cross-reference pack. Readers see the same `edge` relation either way;
//! only the storage under it differs, and the manifest says which (`edge_storage = coded-1`).
//!
//! - `edge_term(kind, id, value)`: one dictionary per column kind (`work`, `rel`, `method`,
//!   `provenance`, `review`), ids from 0 in first-seen order, so a pack's usual values cost no
//!   payload bytes (SQLite stores the integers 0 and 1 in the record header alone).
//! - `edge_row`: the `edge` columns with those ids in place of the text, and each locator as an
//!   integer when it is a plain decimal number (`43003016`), else as text (`3.2.7`,
//!   `1001001-1001003`). `id` (an `INTEGER PRIMARY KEY`, so `VACUUM` keeps it) is the edge's
//!   id, which a writer may choose (B057: the line of the published file).
//! - `edge_posting(side, work, locator, rel, rowids)`: a roaring bitmap of `edge_row` ids per
//!   source (`side` 0) and per destination (`side` 1), in place of the `edge_src` and `edge_dst`
//!   B-tree indexes.
//! - `edge`: a view with exactly the columns, types and text values of the default `edge` table,
//!   so the link index (B068) installs a coded pack with the same SQL and gets the same rows.

use std::collections::{BTreeMap, HashMap};

use roaring::RoaringBitmap;
use rusqlite::types::Value;
use rusqlite::{Connection, params};

use super::{EdgeRow, PackResult, provenance_name};

/// The manifest key naming the edge storage; absent means the default `edge` table.
pub const EDGE_STORAGE_KEY: &str = "edge_storage";

/// The value of [`EDGE_STORAGE_KEY`] for this layout.
pub const CODED_EDGES: &str = "coded-1";

/// Replaces the default `edge` table and its indexes, inside the writer's open transaction.
pub(crate) const CODED_SCHEMA: &str = "
DROP INDEX edge_src;
DROP INDEX edge_dst;
DROP TABLE edge;
CREATE TABLE edge_term(kind TEXT NOT NULL, id INTEGER NOT NULL, value TEXT NOT NULL,
    PRIMARY KEY(kind, id)) WITHOUT ROWID;
CREATE TABLE edge_row(id INTEGER PRIMARY KEY, src_work INTEGER NOT NULL, src_locator NOT NULL, rel INTEGER NOT NULL,
    dst_work INTEGER NOT NULL, dst_locator NOT NULL, weight REAL NOT NULL DEFAULT 0,
    source_work INTEGER NOT NULL, method INTEGER NOT NULL, provenance_kind INTEGER NOT NULL,
    confidence REAL, review_state INTEGER NOT NULL, context TEXT);
CREATE TABLE edge_posting(side INTEGER NOT NULL, work INTEGER NOT NULL, locator NOT NULL,
    rel INTEGER NOT NULL, rowids BLOB NOT NULL, PRIMARY KEY(side, work, locator, rel)) WITHOUT ROWID;
CREATE VIEW edge(src_work, src_locator, rel, dst_work, dst_locator, weight, source_work, method,
    provenance_kind, confidence, review_state, context) AS
SELECT sw.value, CAST(e.src_locator AS TEXT), r.value, dw.value, CAST(e.dst_locator AS TEXT),
    e.weight, ow.value, m.value, p.value, e.confidence, v.value, e.context
FROM edge_row e
JOIN edge_term sw ON sw.kind = 'work' AND sw.id = e.src_work
JOIN edge_term r ON r.kind = 'rel' AND r.id = e.rel
JOIN edge_term dw ON dw.kind = 'work' AND dw.id = e.dst_work
JOIN edge_term ow ON ow.kind = 'work' AND ow.id = e.source_work
JOIN edge_term m ON m.kind = 'method' AND m.id = e.method
JOIN edge_term p ON p.kind = 'provenance' AND p.id = e.provenance_kind
JOIN edge_term v ON v.kind = 'review' AND v.id = e.review_state;
";

/// A locator as stored: an integer when the text is a plain decimal number that reads back the
/// same, else the text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StoredLocator {
    /// `43003016`.
    Number(i64),
    /// `3.2.7`, `1001001-1001003`, `007`.
    Text(String),
}

impl StoredLocator {
    /// How `locator` is stored.
    #[must_use]
    pub fn of(locator: &str) -> StoredLocator {
        let plain = !locator.is_empty()
            && locator.len() <= 18
            && locator.bytes().all(|b| b.is_ascii_digit())
            && (locator == "0" || !locator.starts_with('0'));
        match locator.parse() {
            Ok(n) if plain => StoredLocator::Number(n),
            _ => StoredLocator::Text(locator.to_owned()),
        }
    }

    /// As an SQL value.
    #[must_use]
    pub fn value(&self) -> Value {
        match self {
            StoredLocator::Number(n) => Value::Integer(*n),
            StoredLocator::Text(t) => Value::Text(t.clone()),
        }
    }
}

/// The writer's dictionaries and postings.
#[derive(Debug, Default)]
pub(crate) struct CodedEdges {
    terms: HashMap<(&'static str, String), i64>,
    next: HashMap<&'static str, i64>,
    postings: BTreeMap<(u8, i64, StoredLocator, i64), RoaringBitmap>,
}

impl CodedEdges {
    fn term(&mut self, conn: &Connection, kind: &'static str, value: &str) -> PackResult<i64> {
        if let Some(id) = self.terms.get(&(kind, value.to_owned())) {
            return Ok(*id);
        }
        let next = self.next.entry(kind).or_insert(0);
        let id = *next;
        *next += 1;
        conn.prepare_cached("INSERT INTO edge_term(kind, id, value) VALUES (?1, ?2, ?3)")?
            .execute(params![kind, id, value])?;
        self.terms.insert((kind, value.to_owned()), id);
        Ok(id)
    }

    /// Inserts one edge row and records it in the postings.
    pub(crate) fn insert(
        &mut self,
        conn: &Connection,
        rowid: Option<i64>,
        edge: &EdgeRow<'_>,
    ) -> PackResult<()> {
        let src_work = self.term(conn, "work", edge.src_work)?;
        let rel = self.term(conn, "rel", edge.rel)?;
        let dst_work = self.term(conn, "work", edge.dst_work)?;
        let source_work = self.term(conn, "work", edge.source_work)?;
        let method = self.term(conn, "method", edge.method)?;
        let provenance = self.term(conn, "provenance", provenance_name(edge.provenance))?;
        let review = self.term(conn, "review", edge.review_state.name())?;
        let src = StoredLocator::of(edge.src_locator);
        let dst = StoredLocator::of(edge.dst_locator);
        conn.prepare_cached(
            "INSERT INTO edge_row(id, src_work, src_locator, rel, dst_work, dst_locator, weight,
                                  source_work, method, provenance_kind, review_state, context)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?
        .execute(params![
            rowid,
            src_work,
            src.value(),
            rel,
            dst_work,
            dst.value(),
            f64::from(edge.weight),
            source_work,
            method,
            provenance,
            review,
            edge.context
        ])?;
        let id = u32::try_from(conn.last_insert_rowid()).map_err(|_| {
            rusqlite::Error::InvalidParameterName(
                "a coded edge's rowid must fit in 32 bits".to_owned(),
            )
        })?;
        self.postings
            .entry((0, src_work, src, rel))
            .or_default()
            .insert(id);
        self.postings
            .entry((1, dst_work, dst, rel))
            .or_default()
            .insert(id);
        Ok(())
    }

    /// Writes the postings, in key order.
    pub(crate) fn finish(self, conn: &Connection) -> PackResult<()> {
        let mut insert = conn.prepare(
            "INSERT INTO edge_posting(side, work, locator, rel, rowids) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for ((side, work, locator, rel), mut bitmap) in self.postings {
            bitmap.optimize();
            let mut bytes = Vec::with_capacity(bitmap.serialized_size());
            bitmap
                .serialize_into(&mut bytes)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            insert.execute(params![side, work, locator.value(), rel, bytes])?;
        }
        Ok(())
    }
}

/// Reads the rowids posted for one end of an edge in a coded pack: `side` 0 for sources, 1 for
/// destinations. Empty when the pack has no such edge.
pub fn posted_rowids(
    conn: &Connection,
    side: u8,
    work: i64,
    locator: &str,
    rel: i64,
) -> rusqlite::Result<Vec<u32>> {
    let mut stmt = conn.prepare_cached(
        "SELECT rowids FROM edge_posting WHERE side = ?1 AND work = ?2 AND locator = ?3 AND rel = ?4",
    )?;
    let mut rows = stmt.query(params![side, work, StoredLocator::of(locator).value(), rel])?;
    match rows.next()? {
        None => Ok(Vec::new()),
        Some(row) => {
            let bytes: Vec<u8> = row.get(0)?;
            let bitmap = RoaringBitmap::deserialize_from(&bytes[..]).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Blob,
                    Box::new(e),
                )
            })?;
            Ok(bitmap.iter().collect())
        }
    }
}

/// The id of a term in a coded pack, if the pack has it.
pub fn term_id(conn: &Connection, kind: &str, value: &str) -> rusqlite::Result<Option<i64>> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT id FROM edge_term WHERE kind = ?1 AND value = ?2",
        params![kind, value],
        |r| r.get(0),
    )
    .optional()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links::LinkIndex;
    use crate::pack::fixture::temp_path;
    use crate::pack::{PackBuilder, PackManifest};
    use openlogos_retrieval::{ProvenanceKind, ReviewState};

    fn manifest() -> PackManifest {
        PackManifest {
            edition: "coded-test".to_owned(),
            scheme: "eng".to_owned(),
            source: "hand-written".to_owned(),
            license: "CC0-1.0".to_owned(),
            attribution: String::new(),
            work: None,
            locator_scheme: None,
        }
    }

    /// Edges of every shape a pack holds: verse ids, a range, a work locator, a context, other
    /// provenance and review states, a zero-padded locator that must stay text.
    fn edges() -> Vec<EdgeRow<'static>> {
        let e = |src: &'static str,
                 rel,
                 dst_work,
                 dst: &'static str,
                 weight,
                 method,
                 provenance,
                 review,
                 context| EdgeRow {
            src_work: "bible",
            src_locator: src,
            rel,
            dst_work,
            dst_locator: dst,
            weight,
            source_work: "S09",
            method,
            provenance,
            review_state: review,
            context,
        };
        vec![
            e(
                "1001001",
                "cross_reference",
                "bible",
                "43001001-43001003",
                384.0,
                "S09:votes",
                ProvenanceKind::Curated,
                ReviewState::SourceAttested,
                None,
            ),
            e(
                "1001001",
                "cross_reference",
                "bible",
                "58011003",
                -2.0,
                "S09:votes",
                ProvenanceKind::Curated,
                ReviewState::SourceAttested,
                None,
            ),
            e(
                "58011003",
                "cross_reference",
                "bible",
                "1001001",
                0.0,
                "S09:votes",
                ProvenanceKind::Curated,
                ReviewState::SourceAttested,
                None,
            ),
            e(
                "45003028",
                "cites",
                "institutes",
                "3.2.7",
                0.5,
                "citation-parser@b068-1",
                ProvenanceKind::Extracted,
                ReviewState::Proposed,
                Some("faith is a firm and certain knowledge"),
            ),
            e(
                "007",
                "cites",
                "wcf",
                "1.6",
                0.0,
                "citation-parser@b068-1",
                ProvenanceKind::Extracted,
                ReviewState::Reviewed,
                None,
            ),
        ]
    }

    fn write(path: &std::path::Path, coded: bool) {
        let mut b = if coded {
            PackBuilder::create_coded_edges(path, &manifest(), &[]).unwrap()
        } else {
            PackBuilder::create(path, &manifest(), &[]).unwrap()
        };
        for (i, e) in edges().iter().enumerate() {
            b.add_edge_at(i as i64 + 10, e).unwrap();
        }
        b.finish().unwrap();
    }

    fn rows(conn: &Connection, sql: &str) -> Vec<Vec<Value>> {
        let mut stmt = conn.prepare(sql).unwrap();
        let n = stmt.column_count();
        stmt.query_map([], |r| (0..n).map(|i| r.get::<_, Value>(i)).collect())
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn the_edge_view_reads_exactly_like_the_default_table() {
        let (plain, coded) = (temp_path("edges-plain"), temp_path("edges-coded"));
        write(&plain, false);
        write(&coded, true);
        let a = crate::pack::open_read_only(&plain).unwrap();
        let b = crate::pack::open_read_only(&coded).unwrap();
        let all = "SELECT * FROM edge ORDER BY src_work, src_locator, rel, dst_work, dst_locator";
        assert_eq!(rows(&a, all), rows(&b, all));
        let types = "SELECT typeof(src_locator), typeof(dst_locator), typeof(weight), typeof(context) FROM edge ORDER BY src_locator, dst_locator";
        assert_eq!(rows(&a, types), rows(&b, types));
        let storage = |c: &Connection| {
            c.query_row(
                "SELECT value FROM manifest WHERE key = ?1",
                [EDGE_STORAGE_KEY],
                |r| r.get::<_, String>(0),
            )
            .ok()
        };
        assert_eq!(storage(&a), None);
        assert_eq!(storage(&b).as_deref(), Some(CODED_EDGES));

        // Postings find rows by either end; rowids are the ones the writer chose.
        let bible = term_id(&b, "work", "bible").unwrap().unwrap();
        let xref = term_id(&b, "rel", "cross_reference").unwrap().unwrap();
        assert_eq!(
            posted_rowids(&b, 0, bible, "1001001", xref).unwrap(),
            [10, 11]
        );
        assert_eq!(posted_rowids(&b, 1, bible, "1001001", xref).unwrap(), [12]);
        assert_eq!(
            posted_rowids(&b, 1, bible, "43001001-43001003", xref).unwrap(),
            [10]
        );
        assert!(
            posted_rowids(&b, 0, bible, "2001001", xref)
                .unwrap()
                .is_empty()
        );
        let cites = term_id(&b, "rel", "cites").unwrap().unwrap();
        assert_eq!(posted_rowids(&b, 0, bible, "007", cites).unwrap(), [14]);
        std::fs::remove_file(&plain).unwrap();
        std::fs::remove_file(&coded).unwrap();
    }

    #[test]
    fn the_link_index_gets_the_same_rows_from_either_layout() {
        let (plain, coded) = (temp_path("links-plain"), temp_path("links-coded"));
        write(&plain, false);
        write(&coded, true);
        let dump = |path: &std::path::Path| {
            let mut index = LinkIndex::in_memory().unwrap();
            assert_eq!(index.install_pack("p", path).unwrap(), 5);
            let conn = index.connection();
            (
                rows(conn, "SELECT * FROM link ORDER BY rowid"),
                rows(conn, "SELECT * FROM work ORDER BY id"),
                rows(conn, "SELECT * FROM rel ORDER BY id"),
                rows(conn, "SELECT * FROM cited_count ORDER BY 1, 2, 3"),
            )
        };
        assert_eq!(dump(&plain), dump(&coded));
        std::fs::remove_file(&plain).unwrap();
        std::fs::remove_file(&coded).unwrap();
    }

    #[test]
    fn every_reader_refuses_an_edge_storage_it_does_not_read() {
        let path = temp_path("edges-coded-2");
        let mut b = PackBuilder::create_coded_edges(&path, &manifest(), &[]).unwrap();
        for e in edges() {
            b.add_edge(&e).unwrap();
        }
        b.connection()
            .execute(
                "UPDATE manifest SET value = 'coded-2' WHERE key = ?1",
                [EDGE_STORAGE_KEY],
            )
            .unwrap();
        b.finish().unwrap();
        let refused = |e: String| {
            assert!(
                e.contains("pack edge storage coded-2 is not one this engine reads"),
                "{e}"
            )
        };
        refused(
            LinkIndex::in_memory()
                .unwrap()
                .install_pack("p", &path)
                .unwrap_err()
                .to_string(),
        );
        refused(
            crate::catalog::Catalog::in_memory()
                .unwrap()
                .install_pack("p", &path, crate::catalog::Access::Open)
                .unwrap_err()
                .to_string(),
        );
        refused(
            crate::engine::Engine::new()
                .unwrap()
                .attach_pack(&path)
                .unwrap_err()
                .to_string(),
        );
        refused(
            crate::edition_search::EditionSearch::new()
                .unwrap()
                .attach_file(&path)
                .unwrap_err()
                .to_string(),
        );
        // A link index left nothing behind.
        let mut index = LinkIndex::in_memory().unwrap();
        assert!(index.install_pack("p", &path).is_err());
        assert!(index.installed().unwrap().is_empty());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn locators_are_numbers_only_when_they_read_back_the_same() {
        assert_eq!(
            StoredLocator::of("43003016"),
            StoredLocator::Number(43_003_016)
        );
        assert_eq!(StoredLocator::of("0"), StoredLocator::Number(0));
        for text in ["007", "3.2.7", "1-2", "", "p.6", "12345678901234567890"] {
            assert_eq!(
                StoredLocator::of(text),
                StoredLocator::Text(text.to_owned()),
                "{text}"
            );
        }
    }
}
