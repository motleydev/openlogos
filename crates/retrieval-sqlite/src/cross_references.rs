//! The cross-reference relation pack (B057): a B064 pack whose `edge` table holds one curated
//! `cross_reference` row per published link, read back as an
//! [`openlogos_text::reading::CrossReferenceSource`].
//!
//! Layout, all in the pack's own scheme (the manifest's `scheme`, `eng` for OpenBible), with
//! the coded edge storage of [`crate::pack::coded`] (`edge_storage = coded-1`): readers see the
//! `edge` relation below; it is stored as dictionary codes, integer locators and roaring
//! postings, which keeps 344,799 OpenBible rows under 20 MB.
//!
//! - `edge`: `src_work = dst_work = "bible"`; `src_locator` is the source verse as a decimal
//!   [`VerseId`]; `dst_locator` is the target verse, or `first-last` for a range (specs/19's
//!   `locator[-locator]`); `weight` is the votes exactly as published (negative and zero
//!   kept); `provenance_kind = curated`, `review_state = source_attested`; the `rowid` is
//!   the line of the published file, so every row names its source locator.
//! - `xref_range(first, last, edge)`: each range target cut at chapter ends, as verse ids, for
//!   the derived reverse lookup into ranges (`first` within 999 of the verse asked about); a
//!   single-verse target is found through the destination postings.
//! - `manifest`: the pack keys plus `xref_*` keys stating the relation kind, the direction
//!   rule, what the votes are, coverage and the source's file and hash.
//!
//! The link index (B068) installs the `edge` rows like any other pack's.

use std::io::Cursor;
use std::path::Path;
use std::sync::Mutex;

use openlogos_core::reference::{BookId, Reference};
use openlogos_retrieval::{ProvenanceKind, ReviewState, VerseId};
use openlogos_text::reading::{CrossReferenceInfo, CrossReferenceRow, CrossReferenceSource};
use rusqlite::{Connection, OpenFlags, params};

use crate::pack::coded::{CODED_EDGES, EDGE_STORAGE_KEY, posted_rowids, term_id};
use crate::pack::{EdgeRow, PACK_SCHEMA_VERSION, PackBuilder, PackManifest};

/// The work both ends of a Bible cross reference belong to; the locators are in the pack's
/// scheme.
pub const BIBLE_WORK: &str = "bible";

/// The relation every row carries.
pub const RELATION: &str = "cross_reference";

/// Manifest key: the pack holds cross references (`xref_format` = this value).
pub const XREF_FORMAT: &str = "openlogos-xref-1";

const RANGE_SCHEMA: &str = "
CREATE TABLE xref_range(first INTEGER NOT NULL, last INTEGER NOT NULL, edge INTEGER NOT NULL,
    PRIMARY KEY (first, edge)) WITHOUT ROWID;
";

/// A verse as its locator.
#[must_use]
pub fn verse_id(reference: &Reference) -> VerseId {
    VerseId::new(
        reference.book.number(),
        reference.chapter as u16,
        reference.verse.unwrap_or(0) as u16,
    )
}

fn reference_of(id: u32) -> Option<Reference> {
    let id = VerseId(id);
    Some(Reference::verse(
        BookId::from_number(id.book())?,
        u32::from(id.chapter()),
        u32::from(id.verse()),
    ))
}

/// A target's locator: `id` or `first-last`.
#[must_use]
pub fn target_locator(start: &Reference, end: &Reference) -> String {
    if start == end {
        verse_id(start).0.to_string()
    } else {
        format!("{}-{}", verse_id(start).0, verse_id(end).0)
    }
}

fn parse_target(locator: &str) -> Option<(Reference, Reference)> {
    let (a, b) = locator.split_once('-').unwrap_or((locator, locator));
    Some((
        reference_of(a.parse().ok()?)?,
        reference_of(b.parse().ok()?)?,
    ))
}

/// Every verse of `scheme` from `start` to `end` inclusive, crossing chapters and books by the
/// canon table; `None` when an end is not in the scheme or the range runs backwards.
#[must_use]
pub fn verses_between(scheme: &str, start: &Reference, end: &Reference) -> Option<Vec<Reference>> {
    openlogos_text::reading::scheme_verses(scheme, start, end)
}

/// What a written pack holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WrittenCrossReferences {
    /// Edge rows.
    pub edges: u64,
    /// Of them, range targets.
    pub ranges: u64,
    /// `xref_range` rows (a range across chapters is one row per chapter).
    pub range_rows: u64,
}

/// Writes a cross-reference pack. `manifest.scheme` is the rows' scheme; `info` goes into the
/// `xref_*` manifest keys; `extra` adds more keys (the source file's name and hash). Rows are
/// written with their line as `rowid`. Fails on a target the scheme does not have.
pub fn write_cross_reference_pack(
    path: &Path,
    manifest: &PackManifest,
    info: &CrossReferenceInfo,
    extra: &[(&str, &str)],
    rows: &[CrossReferenceRow],
) -> rusqlite::Result<WrittenCrossReferences> {
    let mut builder = PackBuilder::create_coded_edges(path, manifest, &[])?;
    let conn = builder.connection();
    conn.execute_batch(RANGE_SCHEMA)?;
    let rows_text = rows.len().to_string();
    let entries: Vec<(&str, &str)> = [
        ("xref_format", XREF_FORMAT),
        ("xref_source_id", info.source_id.as_str()),
        ("xref_edition_id", info.edition_id.as_str()),
        ("xref_relation", info.relation.as_str()),
        ("xref_kind", info.kind.as_str()),
        ("xref_votes_meaning", info.votes_meaning.as_str()),
        ("xref_review_state", info.review_state.as_str()),
        ("xref_coverage", info.coverage.as_str()),
        ("xref_rows", rows_text.as_str()),
        (
            "xref_direction",
            "Each edge runs as published: from the verse the source lists it under to its target. A reverse lookup is derived for navigation and is not a claim the source makes.",
        ),
        (
            "xref_locators",
            "src_locator and dst_locator are BBCCCVVV verse ids in the manifest scheme; a range target is first-last; the edge rowid is the line of the published file.",
        ),
    ]
    .into_iter()
    .chain(extra.iter().copied())
    .collect();
    {
        let mut insert = conn.prepare("INSERT INTO manifest(key, value) VALUES (?1, ?2)")?;
        for (key, value) in entries {
            insert.execute(params![key, value])?;
        }
    }
    let mut written = WrittenCrossReferences::default();
    let method = format!("{}:votes", info.source_id);
    for row in rows {
        let src = verse_id(&row.from).0.to_string();
        let dst = target_locator(&row.to_start, &row.to_end);
        builder.add_edge_at(
            i64::from(row.line),
            &EdgeRow {
                src_work: BIBLE_WORK,
                src_locator: &src,
                rel: RELATION,
                dst_work: BIBLE_WORK,
                dst_locator: &dst,
                weight: row.votes as f32,
                source_work: &info.source_id,
                method: &method,
                provenance: ProvenanceKind::Curated,
                review_state: ReviewState::SourceAttested,
                context: None,
            },
        )?;
        written.edges += 1;
        if row.to_start != row.to_end {
            written.ranges += 1;
            let verses =
                verses_between(&manifest.scheme, &row.to_start, &row.to_end).ok_or_else(|| {
                    rusqlite::Error::InvalidParameterName(format!(
                        "line {}: {}-{} is not a range of {}",
                        row.line,
                        row.to_start.to_osis(),
                        row.to_end.to_osis(),
                        manifest.scheme
                    ))
                })?;
            let conn = builder.connection();
            let mut insert = conn
                .prepare_cached("INSERT INTO xref_range(first, last, edge) VALUES (?1, ?2, ?3)")?;
            let mut segment: Option<(u32, u32)> = None;
            for v in verses {
                let id = verse_id(&v).0;
                segment = match segment {
                    Some((first, _)) if first / 1000 == id / 1000 => Some((first, id)),
                    Some((first, last)) => {
                        insert.execute(params![first, last, row.line])?;
                        written.range_rows += 1;
                        Some((id, id))
                    }
                    None => Some((id, id)),
                };
            }
            if let Some((first, last)) = segment {
                insert.execute(params![first, last, row.line])?;
                written.range_rows += 1;
            }
        }
    }
    builder.finish()?;
    Ok(written)
}

/// A cross-reference pack opened for reading.
pub struct CrossReferencePack {
    conn: Mutex<Connection>,
    info: CrossReferenceInfo,
    /// The codes of `bible` and `cross_reference` in the pack's dictionaries.
    bible: i64,
    relation: i64,
}

impl std::fmt::Debug for CrossReferencePack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrossReferencePack")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

fn read_error(error: rusqlite::Error) -> String {
    error.to_string()
}

impl CrossReferencePack {
    /// Opens a pack file read-only.
    pub fn open(path: &Path) -> Result<CrossReferencePack, String> {
        let uri = format!(
            "file:{}?immutable=1",
            path.display()
                .to_string()
                .replace('%', "%25")
                .replace('?', "%3F")
                .replace('#', "%23")
        );
        let conn = Connection::open_with_flags(
            uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
        CrossReferencePack::adopt(conn)
    }

    /// Reads a pack from its bytes (a browser's fetched copy); they are copied into SQLite's
    /// memory.
    pub fn from_bytes(bytes: &[u8]) -> Result<CrossReferencePack, String> {
        if !bytes.starts_with(b"SQLite format 3\0") {
            return Err("these bytes are not a cross-reference pack".to_owned());
        }
        let mut conn = Connection::open_in_memory_with_flags(
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(read_error)?;
        conn.deserialize_read_exact("main", Cursor::new(bytes), bytes.len(), true)
            .map_err(read_error)?;
        CrossReferencePack::adopt(conn)
    }

    fn adopt(conn: Connection) -> Result<CrossReferencePack, String> {
        let value = |key: &str| -> Result<String, String> {
            conn.query_row(
                "SELECT value FROM manifest WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .map_err(|e| format!("manifest {key}: {e}"))
        };
        let version = value("schema_version")?;
        if version != PACK_SCHEMA_VERSION {
            return Err(format!(
                "pack schema {version} is not the supported {PACK_SCHEMA_VERSION}"
            ));
        }
        let storage = value(EDGE_STORAGE_KEY)?;
        if storage != CODED_EDGES {
            return Err(format!(
                "edge storage {storage} is not the supported {CODED_EDGES}; rebuild with just preview-pack-xrefs"
            ));
        }
        let code = |kind: &str, term: &str| -> Result<i64, String> {
            term_id(&conn, kind, term)
                .map_err(read_error)?
                .ok_or_else(|| format!("the pack has no {kind} {term}"))
        };
        let bible = code("work", BIBLE_WORK)?;
        let relation = code("rel", RELATION)?;
        let format = value("xref_format")?;
        if format != XREF_FORMAT {
            return Err(format!(
                "cross-reference format {format} is not the supported {XREF_FORMAT}"
            ));
        }
        let info = CrossReferenceInfo {
            source_id: value("xref_source_id")?,
            edition_id: value("xref_edition_id")?,
            scheme: value("scheme")?,
            license: value("license")?,
            attribution: value("attribution")?,
            relation: value("xref_relation")?,
            kind: value("xref_kind")?,
            votes_meaning: value("xref_votes_meaning")?,
            review_state: value("xref_review_state")?,
            coverage: value("xref_coverage")?,
            rows: value("xref_rows")?
                .parse()
                .map_err(|_| "manifest xref_rows is not a number".to_owned())?,
        };
        Ok(CrossReferencePack {
            conn: Mutex::new(conn),
            info,
            bible,
            relation,
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The rows with these rowids, in rowid (line) order.
    fn rows(
        &self,
        conn: &Connection,
        mut rowids: Vec<u32>,
    ) -> Result<Vec<CrossReferenceRow>, String> {
        rowids.sort_unstable();
        rowids.dedup();
        let mut stmt = conn
            .prepare_cached(
                "SELECT CAST(src_locator AS TEXT), CAST(dst_locator AS TEXT), weight
                 FROM edge_row WHERE id = ?1",
            )
            .map_err(read_error)?;
        let mut out = Vec::with_capacity(rowids.len());
        for line in rowids {
            let (src, dst, votes): (String, String, f64) = stmt
                .query_row([line], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(read_error)?;
            let from = src
                .parse()
                .ok()
                .and_then(reference_of)
                .ok_or_else(|| format!("line {line}: source locator {src}"))?;
            let (to_start, to_end) =
                parse_target(&dst).ok_or_else(|| format!("line {line}: target locator {dst}"))?;
            out.push(CrossReferenceRow {
                line,
                from,
                to_start,
                to_end,
                votes: votes as i32,
            });
        }
        Ok(out)
    }
}

impl CrossReferenceSource for CrossReferencePack {
    fn info(&self) -> &CrossReferenceInfo {
        &self.info
    }

    fn outgoing(&self, verse: &Reference) -> Result<Vec<CrossReferenceRow>, String> {
        let conn = self.lock();
        let id = verse_id(verse).0.to_string();
        let rowids = posted_rowids(&conn, 0, self.bible, &id, self.relation).map_err(read_error)?;
        self.rows(&conn, rowids)
    }

    fn incoming(&self, verse: &Reference) -> Result<Vec<CrossReferenceRow>, String> {
        let conn = self.lock();
        let id = verse_id(verse).0;
        let mut rowids = posted_rowids(&conn, 1, self.bible, &id.to_string(), self.relation)
            .map_err(read_error)?;
        let mut stmt = conn
            .prepare_cached(
                "SELECT edge FROM xref_range WHERE first BETWEEN ?1 - 999 AND ?1 AND last >= ?1",
            )
            .map_err(read_error)?;
        let ranges = stmt
            .query_map([i64::from(id)], |r| r.get::<_, u32>(0))
            .map_err(read_error)?;
        for edge in ranges {
            rowids.push(edge.map_err(read_error)?);
        }
        self.rows(&conn, rowids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links::LinkIndex;
    use openlogos_core::canon;

    fn r(osis: &str) -> Reference {
        canon::segment_reference(osis).unwrap()
    }

    fn info() -> CrossReferenceInfo {
        CrossReferenceInfo {
            source_id: "S09".to_owned(),
            edition_id: "test-edition".to_owned(),
            scheme: "eng".to_owned(),
            license: "CC-BY-4.0".to_owned(),
            attribution: "Cross references from a test".to_owned(),
            relation: RELATION.to_owned(),
            kind: "unspecified".to_owned(),
            votes_meaning: "votes".to_owned(),
            review_state: "source_attested".to_owned(),
            coverage: "three rows".to_owned(),
            rows: 4,
        }
    }

    fn row(line: u32, from: &str, a: &str, b: &str, votes: i32) -> CrossReferenceRow {
        CrossReferenceRow {
            line,
            from: r(from),
            to_start: r(a),
            to_end: r(b),
            votes,
        }
    }

    fn manifest() -> PackManifest {
        PackManifest {
            edition: "xref-test".to_owned(),
            scheme: "eng".to_owned(),
            source: "hand-written rows".to_owned(),
            license: "CC-BY-4.0".to_owned(),
            attribution: "Cross references from a test".to_owned(),
            work: None,
            locator_scheme: None,
        }
    }

    fn rows() -> Vec<CrossReferenceRow> {
        vec![
            row(2, "Gen.1.1", "John.1.1", "John.1.3", 384),
            row(3, "Gen.1.1", "Heb.11.3", "Heb.11.3", -2),
            row(4, "Heb.11.3", "Gen.1.1", "Gen.1.1", 0),
            // Across a chapter and a book, as the published file has them.
            row(5, "Ezra.1.1", "2Chr.36.22", "Ezra.1.3", 13),
        ]
    }

    fn write(name: &str) -> std::path::PathBuf {
        let path = crate::pack::fixture::temp_path(name);
        let written = write_cross_reference_pack(
            &path,
            &manifest(),
            &info(),
            &[("xref_file", "x.txt")],
            &rows(),
        )
        .unwrap();
        assert_eq!(written.edges, 4);
        assert_eq!(written.ranges, 2);
        // John 1:1–3, and 2 Chr 36:22–23 plus Ezra 1:1–3.
        assert_eq!(written.range_rows, 1 + 2);
        path
    }

    #[test]
    fn a_written_pack_reads_back_rows_lines_votes_and_ranges() {
        let path = write("xref-roundtrip");
        let pack = CrossReferencePack::open(&path).unwrap();
        assert_eq!(pack.info(), &info());
        let out = pack.outgoing(&r("Gen.1.1")).unwrap();
        assert_eq!(out, rows()[..2].to_vec());
        // Reverse: single-verse targets through the postings, ranges through xref_range.
        assert_eq!(
            pack.incoming(&r("Gen.1.1")).unwrap(),
            vec![rows()[2].clone()]
        );
        assert_eq!(
            pack.incoming(&r("John.1.2")).unwrap(),
            vec![rows()[0].clone()]
        );
        assert_eq!(
            pack.incoming(&r("2Chr.36.23")).unwrap(),
            vec![rows()[3].clone()]
        );
        assert!(pack.incoming(&r("John.1.4")).unwrap().is_empty());
        assert_eq!(
            pack.incoming(&r("Ezra.1.2")).unwrap(),
            vec![rows()[3].clone()]
        );
        assert!(pack.incoming(&r("Ezra.1.4")).unwrap().is_empty());
        assert!(pack.incoming(&r("2Chr.36.21")).unwrap().is_empty());

        let bytes = std::fs::read(&path).unwrap();
        let in_memory = CrossReferencePack::from_bytes(&bytes).unwrap();
        assert_eq!(
            in_memory.outgoing(&r("Ezra.1.1")).unwrap(),
            vec![rows()[3].clone()]
        );
        assert!(CrossReferencePack::from_bytes(b"not a pack").is_err());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn the_link_index_installs_the_edges_like_any_pack() {
        let path = write("xref-link-index");
        let mut index = LinkIndex::in_memory().unwrap();
        assert_eq!(index.install_pack("xref-test", &path).unwrap(), 4);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_range_the_scheme_does_not_have_stops_the_write() {
        let path = crate::pack::fixture::temp_path("xref-bad-range");
        let bad = vec![row(2, "Gen.1.1", "John.1.1", "John.1.60", 1)];
        assert!(write_cross_reference_pack(&path, &manifest(), &info(), &[], &bad).is_err());
        let _ = std::fs::remove_file(&path);
    }

    /// The coded pack answers exactly as the rows themselves do (the in-memory source over the
    /// same rows), for lists holding partial, reverse-derived, unmapped and not-in-edition items;
    /// and a default-layout pack of the same edges gives the link index identical rows.
    #[test]
    fn the_coded_pack_is_equivalent_to_its_rows_and_to_the_default_layout() {
        use openlogos_core::canon::BookOrder;
        use openlogos_core::versification::MappingStatus;
        use openlogos_text::reading::{
            Availability, CrossReferenceDirection, CrossReferenceOptions, Edition,
            MemoryCrossReferences, PlaceholderPack, ReadingLibrary,
        };
        let published = vec![
            row(2, "Gen.1.1", "John.1.1", "John.1.3", 384),
            row(3, "Gen.1.1", "Heb.11.3", "Heb.11.3", -2),
            row(4, "Heb.11.3", "Gen.1.1", "Gen.1.1", 0),
            row(5, "Ps.33.6", "Gen.1.1", "Gen.1.3", 12),
            row(6, "Eph.3.20", "Acts.19.23", "Acts.19.41", 2),
            row(7, "Eph.3.20", "2Cor.13.14", "2Cor.13.14", 1),
            row(8, "Eph.3.20", "Rom.16.25", "Rom.16.27", 4),
            row(9, "Eph.3.20", "Jer.32.17", "Jer.32.17", 134),
            row(10, "Acts.19.30", "Eph.3.20", "Eph.3.20", 3),
        ];
        let mut info = info();
        info.rows = published.len() as u64;
        let coded = crate::pack::fixture::temp_path("xref-equivalence-coded");
        write_cross_reference_pack(&coded, &manifest(), &info, &[], &published).unwrap();

        // The same edges in the default layout.
        let plain = crate::pack::fixture::temp_path("xref-equivalence-plain");
        let mut b = PackBuilder::create(&plain, &manifest(), &[]).unwrap();
        for r in &published {
            let src = verse_id(&r.from).0.to_string();
            let dst = target_locator(&r.to_start, &r.to_end);
            b.add_edge_at(
                i64::from(r.line),
                &EdgeRow {
                    src_work: BIBLE_WORK,
                    src_locator: &src,
                    rel: RELATION,
                    dst_work: BIBLE_WORK,
                    dst_locator: &dst,
                    weight: r.votes as f32,
                    source_work: "S09",
                    method: "S09:votes",
                    provenance: ProvenanceKind::Curated,
                    review_state: ReviewState::SourceAttested,
                    context: None,
                },
            )
            .unwrap();
        }
        b.finish().unwrap();
        let dump = |path: &std::path::Path| {
            let mut index = LinkIndex::in_memory().unwrap();
            index.install_pack("xref", path).unwrap();
            let conn = index.connection();
            let mut out = Vec::new();
            for sql in [
                "SELECT * FROM link ORDER BY rowid",
                "SELECT * FROM work ORDER BY id",
                "SELECT * FROM rel ORDER BY id",
                "SELECT * FROM cited_count ORDER BY 1, 2, 3",
            ] {
                let mut stmt = conn.prepare(sql).unwrap();
                let n = stmt.column_count();
                let rows: Vec<Vec<rusqlite::types::Value>> = stmt
                    .query_map([], |r| (0..n).map(|i| r.get(i)).collect())
                    .unwrap()
                    .map(Result::unwrap)
                    .collect();
                out.push(rows);
            }
            out
        };
        assert_eq!(dump(&plain), dump(&coded));

        let library = |source: Box<dyn CrossReferenceSource>| {
            let mut lib = ReadingLibrary::new();
            let edition = |id: &str, scheme: &str| Edition {
                resource_id: id.to_owned(),
                content_revision: format!("{id}-test"),
                title: id.to_uppercase(),
                name: id.to_owned(),
                language: "en".to_owned(),
                direction: "ltr".to_owned(),
                scheme: scheme.to_owned(),
                book_order: BookOrder::Protestant,
                placeholder: true,
                attribution: None,
            };
            let nt: Vec<BookId> = BookId::all()
                .into_iter()
                .filter(|b| *b >= BookId::Matt)
                .collect();
            lib.insert(Box::new(
                PlaceholderPack::new(edition("bsb", "eng"), &BookId::all(), Vec::new()).unwrap(),
            ));
            lib.insert(Box::new(
                PlaceholderPack::new(edition("sblgnt", "org"), &nt, Vec::new()).unwrap(),
            ));
            lib.set_cross_references(source);
            lib
        };
        let from_pack = library(Box::new(CrossReferencePack::open(&coded).unwrap()));
        let from_rows = library(Box::new(MemoryCrossReferences::new(
            CrossReferencePack::open(&coded).unwrap().info().clone(),
            published.clone(),
        )));
        let mut seen = (false, false, false, false);
        for (edition, passage) in [
            ("bsb", "Gen 1:1"),
            ("bsb", "John 1:2"),
            ("bsb", "Gen 1:2"),
            ("sblgnt", "Heb 11:3"),
            ("sblgnt", "Eph 3:20"),
            ("sblgnt", "Acts 19:30"),
            ("bsb", "Acts 19:41"),
        ] {
            for include_reverse in [false, true] {
                let options = CrossReferenceOptions {
                    include_reverse,
                    ..CrossReferenceOptions::default()
                };
                let a = from_pack
                    .cross_references(edition, passage, &options)
                    .unwrap();
                let b = from_rows
                    .cross_references(edition, passage, &options)
                    .unwrap();
                assert_eq!(a, b, "{edition} {passage} reverse {include_reverse}");
                for item in &a.items {
                    seen.0 |= item.mapping == MappingStatus::Partial;
                    seen.1 |= item.direction == CrossReferenceDirection::ReverseDerived;
                    seen.2 |= item.availability == Availability::Unmapped;
                    seen.3 |= item.availability == Availability::BookNotInEdition;
                }
            }
        }
        assert_eq!(seen, (true, true, true, true));
        std::fs::remove_file(&plain).unwrap();
        std::fs::remove_file(&coded).unwrap();
    }

    #[test]
    fn verses_between_crosses_chapters_and_books() {
        let v = verses_between("eng", &r("2Chr.36.22"), &r("Ezra.1.2")).unwrap();
        let osis: Vec<String> = v.iter().map(Reference::to_osis).collect();
        assert_eq!(osis, ["2Chr.36.22", "2Chr.36.23", "Ezra.1.1", "Ezra.1.2"]);
        assert!(verses_between("eng", &r("Mal.4.1"), &r("Mal.4.2")).is_some());
        assert!(verses_between("org", &r("Mal.4.1"), &r("Mal.4.2")).is_none());
        assert!(verses_between("eng", &r("John.1.3"), &r("John.1.1")).is_none());
    }
}
