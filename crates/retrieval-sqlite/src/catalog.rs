//! The small shared device catalog (specs/19, Cross-resource links, item 3): installed packs
//! with their SHA-256, the work and edition each is, the registered locator schemes, and the
//! `section_map` rows copied out of every installed pack so a citation resolves to each
//! installed edition without attaching packs. The link index is rebuilt from this catalog's
//! installed set; this file is the source of truth for "what is installed".
//!
//! It also answers the two questions a display needs for a cited target: is it in the
//! library, and if not, why ("cited, not in your library": not installed, restricted,
//! unresolved or unknown work), and it produces the packager's per-scheme resolution report.

use std::io::Read;
use std::path::{Path, PathBuf};

use openlogos_core::locator::{
    Citation, CitationGrammar, GrammarKind, GrammarTable, WorkGrammar, parse_citation,
};
use openlogos_retrieval::{
    CitedTarget, Edge, Locator, LocatorScheme, LocatorSchemeKind, NotInLibraryReason,
    ResolutionBasis, ResolutionOutcome, ResolutionReport, Result, RetrievalError, SchemeMinimum,
    SectionMap, SectionMapKind, SectionMapRow, SectionOutcome, SectionResolution, SectionTarget,
    TargetStatus,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::pack::{PACK_SCHEMA_VERSION, open_read_only};
use crate::vectors::storage;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS scheme(id TEXT PRIMARY KEY, work TEXT NOT NULL, title TEXT NOT NULL,
    canonical INTEGER NOT NULL, kind TEXT NOT NULL, print_edition TEXT, levels TEXT NOT NULL,
    version TEXT NOT NULL, grammar_version TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS pack(pack_id TEXT PRIMARY KEY, path TEXT NOT NULL, sha256 TEXT NOT NULL,
    edition TEXT NOT NULL, work TEXT, locator_scheme TEXT, access TEXT NOT NULL,
    section_rows INTEGER NOT NULL) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS section_map(pack_id TEXT NOT NULL, scheme TEXT NOT NULL, locator TEXT NOT NULL,
    edition TEXT NOT NULL, segment TEXT, kind TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS section_map_pack ON section_map(pack_id);
CREATE INDEX IF NOT EXISTS section_map_edition ON section_map(edition);
";

/// What the user may read of an installed pack's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    /// Openly licensed text.
    Open,
    /// Restricted text the user has licensed.
    Licensed,
    /// Restricted text the user has not licensed: citations into it display locator and
    /// citing context only (specs/19 item 8).
    Restricted,
}

impl Access {
    fn name(self) -> &'static str {
        match self {
            Access::Open => "open",
            Access::Licensed => "licensed",
            Access::Restricted => "restricted",
        }
    }

    fn parse(text: &str) -> Access {
        match text {
            "licensed" => Access::Licensed,
            "restricted" => Access::Restricted,
            _ => Access::Open,
        }
    }

    /// Whether the text may be shown.
    #[must_use]
    pub fn readable(self) -> bool {
        self != Access::Restricted
    }
}

/// One installed pack as the catalog records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPack {
    /// Pack id (the link index's key).
    pub pack_id: String,
    /// Where the file is.
    pub path: PathBuf,
    /// Lower-case hex SHA-256 of the file as installed.
    pub sha256: String,
    /// Edition id from the manifest.
    pub edition: String,
    /// Work id from the manifest; `None` for a Bible edition.
    pub work: Option<String>,
    /// The work's canonical scheme from the manifest.
    pub locator_scheme: Option<String>,
    /// Rights status at install time.
    pub access: Access,
    /// `section_map` rows copied from the pack.
    pub section_rows: u64,
}

/// A citation the packager found in a work's text, to be counted in the resolution report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationCandidate {
    /// The citation as written.
    pub text: String,
    /// The scheme the extractor expected, so an unparsed citation is counted under it.
    pub scheme_hint: Option<String>,
}

/// Lower-case hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// A catalog entry built from a core grammar: the scheme's shape and levels come from the
/// grammar, the title and the page scheme's print edition from the caller.
#[must_use]
pub fn scheme_from_grammar(
    grammar: &WorkGrammar,
    title: &str,
    print_edition: Option<&str>,
) -> LocatorScheme {
    LocatorScheme {
        id: grammar.scheme.clone(),
        work: grammar.work.clone(),
        title: title.to_owned(),
        canonical: grammar.canonical,
        kind: match grammar.kind {
            GrammarKind::Hierarchical => LocatorSchemeKind::Hierarchical,
            GrammarKind::QuestionAnswer => LocatorSchemeKind::QuestionAnswer,
            GrammarKind::Page => LocatorSchemeKind::Page {
                print_edition: print_edition.unwrap_or_default().to_owned(),
            },
        },
        levels: grammar.levels.iter().map(|l| l.name.clone()).collect(),
        version: "1".to_owned(),
        grammar_version: grammar.version.clone(),
    }
}

/// The catalog entries for the built-in grammars of `openlogos_core::locator`: the Westminster
/// Confession and catechisms, Calvin's *Institutes* and Thayer's 1889 page milestones, plus
/// the canonical headword scheme of Thayer that the page scheme falls back to.
#[must_use]
pub fn builtin_schemes() -> Vec<LocatorScheme> {
    let table = GrammarTable::builtin();
    let mut schemes: Vec<LocatorScheme> = table
        .grammars
        .iter()
        .map(|g| {
            let (title, print) = match g.scheme.as_str() {
                "wcf" => ("Westminster Confession of Faith (1647), chapter and section", None),
                "wlc" => ("Westminster Larger Catechism (1648), question", None),
                "wsc" => ("Westminster Shorter Catechism (1647), question", None),
                "institutes" => (
                    "Calvin, Institutes of the Christian Religion (1559), book, chapter and section",
                    None,
                ),
                "thayer-1889-pages" => (
                    "Thayer, Greek-English Lexicon of the New Testament (1889 print edition), page",
                    Some("thayer-1889"),
                ),
                _ => (g.scheme.as_str(), None),
            };
            scheme_from_grammar(g, title, print)
        })
        .collect();
    schemes.push(LocatorScheme {
        id: "thayer".into(),
        work: "thayer".into(),
        title: "Thayer, Greek-English Lexicon of the New Testament, headword".into(),
        canonical: true,
        kind: LocatorSchemeKind::Hierarchical,
        levels: vec!["headword".into()],
        version: "1".into(),
        grammar_version: table.grammars[0].version.clone(),
    });
    schemes.sort_by(|a, b| a.id.cmp(&b.id));
    schemes
}

/// The edge locator string for a citation range start: bare in the work's canonical scheme,
/// `scheme:locator` otherwise.
#[must_use]
pub fn edge_locator(scheme: &str, canonical: bool, locator: &str) -> String {
    if canonical {
        locator.to_owned()
    } else {
        format!("{scheme}:{locator}")
    }
}

/// The catalog database.
pub struct Catalog {
    conn: Connection,
}

impl Catalog {
    /// Opens or creates the catalog at `path`.
    pub fn open(path: &Path) -> Result<Catalog> {
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
        Ok(Catalog { conn })
    }

    /// An in-memory catalog (tests).
    pub fn in_memory() -> Result<Catalog> {
        let conn = Connection::open_in_memory().map_err(storage)?;
        conn.execute_batch(SCHEMA).map_err(storage)?;
        Ok(Catalog { conn })
    }

    /// Registers or replaces a locator scheme.
    pub fn register_scheme(&mut self, scheme: &LocatorScheme) -> Result<()> {
        let (kind, print_edition) = match &scheme.kind {
            LocatorSchemeKind::Hierarchical => ("hierarchical", None),
            LocatorSchemeKind::QuestionAnswer => ("question_answer", None),
            LocatorSchemeKind::Page { print_edition } => ("page", Some(print_edition.as_str())),
        };
        self.conn
            .execute(
                "INSERT OR REPLACE INTO scheme(id, work, title, canonical, kind, print_edition, levels, version, grammar_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    scheme.id,
                    scheme.work,
                    scheme.title,
                    i64::from(scheme.canonical),
                    kind,
                    print_edition,
                    scheme.levels.join("\u{1f}"),
                    scheme.version,
                    scheme.grammar_version
                ],
            )
            .map_err(storage)?;
        Ok(())
    }

    /// Registers every built-in scheme.
    pub fn register_builtin_schemes(&mut self) -> Result<()> {
        for scheme in builtin_schemes() {
            self.register_scheme(&scheme)?;
        }
        Ok(())
    }

    /// Every registered scheme, by id.
    pub fn schemes(&self) -> Result<Vec<LocatorScheme>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, work, title, canonical, kind, print_edition, levels, version, grammar_version
                 FROM scheme ORDER BY id",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map([], |r| {
                let kind: String = r.get(4)?;
                let print_edition: Option<String> = r.get(5)?;
                let levels: String = r.get(6)?;
                Ok(LocatorScheme {
                    id: r.get(0)?,
                    work: r.get(1)?,
                    title: r.get(2)?,
                    canonical: r.get::<_, i64>(3)? != 0,
                    kind: match kind.as_str() {
                        "question_answer" => LocatorSchemeKind::QuestionAnswer,
                        "page" => LocatorSchemeKind::Page {
                            print_edition: print_edition.unwrap_or_default(),
                        },
                        _ => LocatorSchemeKind::Hierarchical,
                    },
                    levels: if levels.is_empty() {
                        Vec::new()
                    } else {
                        levels.split('\u{1f}').map(str::to_owned).collect()
                    },
                    version: r.get(7)?,
                    grammar_version: r.get(8)?,
                })
            })
            .map_err(storage)?;
        rows.collect::<std::result::Result<_, _>>().map_err(storage)
    }

    /// The canonical scheme id of a work, if one is registered.
    pub fn canonical_scheme(&self, work: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT id FROM scheme WHERE work = ?1 AND canonical = 1 ORDER BY id LIMIT 1",
                params![work],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)
    }

    /// Records a pack as installed: hashes the file, reads its manifest, copies its
    /// `section_map` rows. Replaces an earlier record of the same pack id.
    pub fn install_pack(
        &mut self,
        pack_id: &str,
        path: &Path,
        access: Access,
    ) -> Result<InstalledPack> {
        let sha256 = sha256_file(path).map_err(|e| RetrievalError::Storage(e.to_string()))?;
        let pack = open_read_only(path).map_err(storage)?;
        let manifest = |key: &str| -> Result<Option<String>> {
            pack.query_row(
                "SELECT value FROM manifest WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)
        };
        let version = manifest("schema_version")?.unwrap_or_default();
        if version != PACK_SCHEMA_VERSION {
            return Err(RetrievalError::Storage(format!(
                "pack schema {version} is not the supported {PACK_SCHEMA_VERSION}"
            )));
        }
        let edition = manifest("edition")?.unwrap_or_default();
        let work = manifest("work")?;
        let locator_scheme = manifest("locator_scheme")?;
        let rows: Vec<SectionMapRow> = {
            let mut stmt = pack
                .prepare(
                    "SELECT scheme, locator, edition, segment, kind FROM section_map
                     ORDER BY scheme, locator, edition, segment, kind",
                )
                .map_err(storage)?;
            let rows = stmt
                .query_map([], |r| {
                    let kind: String = r.get(4)?;
                    Ok(SectionMapRow {
                        scheme: r.get(0)?,
                        locator: r.get(1)?,
                        edition: r.get(2)?,
                        segment: r.get(3)?,
                        kind: SectionMapKind::parse(&kind).unwrap_or(SectionMapKind::Equal),
                    })
                })
                .map_err(storage)?;
            rows.collect::<std::result::Result<_, _>>()
                .map_err(storage)?
        };
        drop(pack);
        let tx = self.conn.unchecked_transaction().map_err(storage)?;
        tx.execute(
            "DELETE FROM section_map WHERE pack_id = ?1",
            params![pack_id],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT OR REPLACE INTO pack(pack_id, path, sha256, edition, work, locator_scheme, access, section_rows)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                pack_id,
                path.display().to_string(),
                sha256,
                edition,
                work,
                locator_scheme,
                access.name(),
                rows.len() as i64
            ],
        )
        .map_err(storage)?;
        {
            let mut insert = tx
                .prepare(
                    "INSERT INTO section_map(pack_id, scheme, locator, edition, segment, kind) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .map_err(storage)?;
            for row in &rows {
                insert
                    .execute(params![
                        pack_id,
                        row.scheme,
                        row.locator,
                        row.edition,
                        row.segment,
                        row.kind.name()
                    ])
                    .map_err(storage)?;
            }
        }
        tx.commit().map_err(storage)?;
        Ok(InstalledPack {
            pack_id: pack_id.to_owned(),
            path: path.to_path_buf(),
            sha256,
            edition,
            work,
            locator_scheme,
            access,
            section_rows: rows.len() as u64,
        })
    }

    /// Forgets a pack. Returns whether it was installed.
    pub fn remove_pack(&mut self, pack_id: &str) -> Result<bool> {
        let tx = self.conn.unchecked_transaction().map_err(storage)?;
        tx.execute(
            "DELETE FROM section_map WHERE pack_id = ?1",
            params![pack_id],
        )
        .map_err(storage)?;
        let n = tx
            .execute("DELETE FROM pack WHERE pack_id = ?1", params![pack_id])
            .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(n > 0)
    }

    /// Changes the rights status of an installed pack (a license was acquired or lapsed).
    pub fn set_access(&mut self, pack_id: &str, access: Access) -> Result<bool> {
        let n = self
            .conn
            .execute(
                "UPDATE pack SET access = ?2 WHERE pack_id = ?1",
                params![pack_id, access.name()],
            )
            .map_err(storage)?;
        Ok(n > 0)
    }

    /// Every installed pack, by pack id: the installed set the link index is rebuilt from.
    pub fn installed(&self) -> Result<Vec<InstalledPack>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT pack_id, path, sha256, edition, work, locator_scheme, access, section_rows
                 FROM pack ORDER BY pack_id",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map([], |r| {
                let access: String = r.get(6)?;
                Ok(InstalledPack {
                    pack_id: r.get(0)?,
                    path: PathBuf::from(r.get::<_, String>(1)?),
                    sha256: r.get(2)?,
                    edition: r.get(3)?,
                    work: r.get(4)?,
                    locator_scheme: r.get(5)?,
                    access: Access::parse(&access),
                    section_rows: r.get::<_, i64>(7)? as u64,
                })
            })
            .map_err(storage)?;
        rows.collect::<std::result::Result<_, _>>().map_err(storage)
    }

    /// `(pack id, path)` of every installed pack, the argument of `LinkIndex::rebuild`.
    pub fn pack_list(&self) -> Result<Vec<(String, PathBuf)>> {
        Ok(self
            .installed()?
            .into_iter()
            .map(|p| (p.pack_id, p.path))
            .collect())
    }

    /// Installed editions of a work with their access, by edition id. A Bible edition's
    /// work is its edition id.
    pub fn editions_of(&self, work: &str) -> Result<Vec<(String, Access)>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT edition, access FROM pack WHERE work = ?1 OR (work IS NULL AND edition = ?1)
                 ORDER BY edition",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![work], |r| {
                let access: String = r.get(1)?;
                Ok((r.get::<_, String>(0)?, Access::parse(&access)))
            })
            .map_err(storage)?;
        rows.collect::<std::result::Result<_, _>>().map_err(storage)
    }

    /// The merged section map of every installed edition of a work.
    pub fn section_map(&self, work: &str) -> Result<SectionMap> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT s.scheme, s.locator, s.edition, s.segment, s.kind FROM section_map s
                 JOIN pack p ON p.pack_id = s.pack_id WHERE p.work = ?1",
            )
            .map_err(storage)?;
        let rows = stmt
            .query_map(params![work], |r| {
                let kind: String = r.get(4)?;
                Ok(SectionMapRow {
                    scheme: r.get(0)?,
                    locator: r.get(1)?,
                    edition: r.get(2)?,
                    segment: r.get(3)?,
                    kind: SectionMapKind::parse(&kind).unwrap_or(SectionMapKind::Equal),
                })
            })
            .map_err(storage)?;
        let rows: Vec<SectionMapRow> = rows
            .collect::<std::result::Result<_, _>>()
            .map_err(storage)?;
        Ok(SectionMap::new(rows))
    }

    /// Resolves a target locator against the installed library (specs/19 items 2, 4 and 8).
    pub fn resolve(&self, target: &Locator) -> Result<TargetStatus> {
        let editions = self.editions_of(&target.work)?;
        let schemes = self.schemes()?;
        let known = schemes.iter().any(|s| s.work == target.work);
        if editions.is_empty() {
            return Ok(TargetStatus::NotInLibrary {
                reason: if known {
                    NotInLibraryReason::NotInstalled
                } else {
                    NotInLibraryReason::UnknownWork
                },
                resolution: None,
            });
        }
        let canonical = self.canonical_scheme(&target.work)?;
        let installed: Vec<String> = editions.iter().map(|(e, _)| e.clone()).collect();
        let resolution = match canonical {
            // A Bible edition (no locator scheme): verse locators are the edition's own
            // coordinates; which verses it prints is the verse map's business (B006).
            None => SectionResolution {
                target: SectionTarget {
                    scheme: target.work.clone(),
                    locator: target.locator.clone(),
                },
                editions: installed
                    .iter()
                    .map(|edition| openlogos_retrieval::EditionResolution {
                        edition: edition.clone(),
                        outcome: SectionOutcome::Mapped {
                            kind: SectionMapKind::Equal,
                            segments: vec![target.locator.clone()],
                            basis: ResolutionBasis::Direct,
                        },
                    })
                    .collect(),
            },
            Some(canonical) => {
                let section_target = SectionTarget::from_locator(&target.locator, &canonical);
                self.section_map(&target.work)?
                    .resolve(&schemes, &section_target, &installed)
            }
        };
        let readable = |edition: &str| {
            editions
                .iter()
                .any(|(e, access)| e == edition && access.readable())
        };
        let has_text = |want_readable: bool| {
            resolution.editions.iter().any(|e| {
                matches!(&e.outcome, SectionOutcome::Mapped { kind, .. } if *kind != SectionMapKind::Absent)
                    && readable(&e.edition) == want_readable
            })
        };
        if has_text(true) {
            Ok(TargetStatus::InLibrary { resolution })
        } else if has_text(false) {
            Ok(TargetStatus::NotInLibrary {
                reason: NotInLibraryReason::Restricted,
                resolution: Some(resolution),
            })
        } else {
            Ok(TargetStatus::NotInLibrary {
                reason: NotInLibraryReason::Unresolved,
                resolution: Some(resolution),
            })
        }
    }

    /// The display record of one outbound edge: its target's status with the citing locator
    /// and context kept whatever the status is.
    pub fn cited_target(&self, edge: &Edge, context: Option<&str>) -> Result<CitedTarget> {
        Ok(CitedTarget {
            target: edge.dst.clone(),
            cited_by: edge.src.clone(),
            rel: edge.rel.clone(),
            context: context.map(str::to_owned),
            status: self.resolve(&edge.dst)?,
        })
    }

    /// The packager's report for one pack: every candidate citation parsed with `grammar`
    /// and resolved against this catalog's section maps, counted per scheme, with the
    /// configured minimums applied. Rights do not count here: a locator that maps in any
    /// installed edition is resolved.
    pub fn resolution_report(
        &self,
        pack_id: &str,
        candidates: &[CitationCandidate],
        minimums: &[SchemeMinimum],
        grammar: &CitationGrammar,
    ) -> Result<ResolutionReport> {
        let mut report = ResolutionReport::new(
            pack_id,
            grammar
                .works
                .grammars
                .first()
                .map_or("", |g| g.version.as_str()),
        );
        let schemes = self.schemes()?;
        for candidate in candidates {
            match parse_citation(&candidate.text, grammar) {
                Err(_) => report.record(
                    candidate.scheme_hint.as_deref().unwrap_or("unknown"),
                    ResolutionOutcome::Unparsed,
                ),
                Ok(Citation::Scripture { .. }) => {
                    report.record("scripture", ResolutionOutcome::Resolved);
                }
                Ok(Citation::Work { citation }) => {
                    let map = self.section_map(&citation.work)?;
                    let installed: Vec<String> = self
                        .editions_of(&citation.work)?
                        .into_iter()
                        .map(|(e, _)| e)
                        .collect();
                    let outcome = if !installed.is_empty()
                        && citation.ranges.iter().all(|range| {
                            let target = SectionTarget {
                                scheme: citation.scheme.clone(),
                                locator: range.start.to_string(),
                            };
                            map.resolve(&schemes, &target, &installed).has_text()
                        }) {
                        ResolutionOutcome::Resolved
                    } else {
                        ResolutionOutcome::UnresolvedKnownWork
                    };
                    report.record(&citation.scheme, outcome);
                }
            }
        }
        report.apply_minimums(minimums);
        Ok(report)
    }
}

#[cfg(test)]
pub(crate) mod fixture {
    //! Four tiny non-Bible packs for the catalog, link-index and conformance tests. The
    //! section-map rows are structural (locator → segment id); no work text is stored, so
    //! nothing here is a corpus. The works are public domain: the Westminster Confession of
    //! Faith (1647), Calvin's *Institutes* in the Beveridge (1845) and Allen (1813)
    //! translations, and Thayer's *Greek-English Lexicon* (1889).

    use super::*;
    use crate::pack::{EdgeRow, PackBuilder, PackManifest};
    use openlogos_retrieval::{ProvenanceKind, ReviewState};

    fn manifest(edition: &str, work: &str, scheme: &str) -> PackManifest {
        PackManifest {
            edition: edition.into(),
            scheme: "none".into(),
            source: "structural fixture, no text".into(),
            license: "CC0-1.0".into(),
            attribution: String::new(),
            work: Some(work.into()),
            locator_scheme: Some(scheme.into()),
        }
    }

    fn row(
        scheme: &str,
        locator: &str,
        edition: &str,
        segment: Option<&str>,
        kind: SectionMapKind,
    ) -> SectionMapRow {
        SectionMapRow {
            scheme: scheme.into(),
            locator: locator.into(),
            edition: edition.into(),
            segment: segment.map(str::to_owned),
            kind,
        }
    }

    /// Builds the packs under `dir`; returns `(pack id, path)` pairs.
    pub fn build_all(dir: &Path) -> Vec<(String, PathBuf)> {
        use SectionMapKind::*;
        let mut out = Vec::new();
        // WCF: chapter 1 sections 1..=10 as one segment each; 1.6 cites Scripture.
        {
            let path = dir.join("wcf-1647.sqlite");
            let mut b =
                PackBuilder::create(&path, &manifest("wcf-1647", "wcf", "wcf"), &[]).unwrap();
            for s in 1..=10 {
                b.add_section_map(&row(
                    "wcf",
                    &format!("1.{s}"),
                    "wcf-1647",
                    Some(&format!("ch1-s{s}")),
                    Equal,
                ))
                .unwrap();
            }
            b.add_edge(&EdgeRow {
                src_work: "wcf",
                src_locator: "1.6",
                rel: "explicitly_cites",
                dst_work: "bsb",
                dst_locator: "55003015",
                weight: 0.0,
                source_work: "wcf-1647",
                method: "proof-texts@1647",
                provenance: ProvenanceKind::Curated,
                review_state: ReviewState::SourceAttested,
                context: Some(
                    "the whole counsel of God ... is either expressly set down in Scripture",
                ),
            })
            .unwrap();
            b.finish().unwrap();
            out.push(("wcf-1647".to_owned(), path));
        }
        // Institutes, Beveridge 1845: 3.2.7 one segment; 3.2.8 and 3.2.9 share a segment
        // (merge); 4.20.32 absent (the fixture pretends the translation stops short).
        {
            let path = dir.join("institutes-beveridge.sqlite");
            let mut b = PackBuilder::create(
                &path,
                &manifest("institutes-beveridge", "institutes", "institutes"),
                &[],
            )
            .unwrap();
            b.add_section_map(&row(
                "institutes",
                "3.2.7",
                "institutes-beveridge",
                Some("b-3-2-7"),
                Equal,
            ))
            .unwrap();
            b.add_section_map(&row(
                "institutes",
                "3.2.8",
                "institutes-beveridge",
                Some("b-3-2-8-9"),
                Merge,
            ))
            .unwrap();
            b.add_section_map(&row(
                "institutes",
                "3.2.9",
                "institutes-beveridge",
                Some("b-3-2-8-9"),
                Merge,
            ))
            .unwrap();
            b.add_section_map(&row(
                "institutes",
                "4.20.32",
                "institutes-beveridge",
                None,
                Absent,
            ))
            .unwrap();
            // Beveridge's 3.2.7 quotes WCF 1.6 (fixture relation) and cites Thayer by page.
            b.add_edge(&EdgeRow {
                src_work: "institutes",
                src_locator: "3.2.7",
                rel: "quotes",
                dst_work: "wcf",
                dst_locator: "1.6",
                weight: 0.0,
                source_work: "institutes-beveridge",
                method: "quote-parser@1",
                provenance: ProvenanceKind::Extracted,
                review_state: ReviewState::Reviewed,
                context: Some("faith is a firm and certain knowledge"),
            })
            .unwrap();
            b.add_edge(&EdgeRow {
                src_work: "institutes",
                src_locator: "3.2.7",
                rel: "explicitly_cites",
                dst_work: "thayer",
                dst_locator: "thayer-1889-pages:p.6",
                weight: 0.0,
                source_work: "institutes-beveridge",
                method: "citation-parser@b068-1",
                provenance: ProvenanceKind::Extracted,
                review_state: ReviewState::SourceAttested,
                context: Some("(Thayer p. 6)"),
            })
            .unwrap();
            b.add_edge(&EdgeRow {
                src_work: "institutes",
                src_locator: "3.2.7",
                rel: "explicitly_cites",
                dst_work: "hodge-systematic",
                dst_locator: "1.1.1",
                weight: 0.0,
                source_work: "institutes-beveridge",
                method: "citation-parser@b068-1",
                provenance: ProvenanceKind::Extracted,
                review_state: ReviewState::SourceAttested,
                context: Some("(Hodge 1.1.1)"),
            })
            .unwrap();
            b.finish().unwrap();
            out.push(("institutes-beveridge".to_owned(), path));
        }
        // Institutes, Allen 1813: 3.2.7 split over two segments; 3.2.8 partial; no 3.2.9 row.
        {
            let path = dir.join("institutes-allen.sqlite");
            let mut b = PackBuilder::create(
                &path,
                &manifest("institutes-allen", "institutes", "institutes"),
                &[],
            )
            .unwrap();
            b.add_section_map(&row(
                "institutes",
                "3.2.7",
                "institutes-allen",
                Some("a-3-2-7a"),
                Split,
            ))
            .unwrap();
            b.add_section_map(&row(
                "institutes",
                "3.2.7",
                "institutes-allen",
                Some("a-3-2-7b"),
                Split,
            ))
            .unwrap();
            b.add_section_map(&row(
                "institutes",
                "3.2.8",
                "institutes-allen",
                Some("a-3-2-8"),
                Partial,
            ))
            .unwrap();
            b.finish().unwrap();
            out.push(("institutes-allen".to_owned(), path));
        }
        // Thayer 1889 print edition: three headwords and the pages they sit on.
        {
            let path = dir.join("thayer-1889.sqlite");
            let mut b =
                PackBuilder::create(&path, &manifest("thayer-1889", "thayer", "thayer"), &[])
                    .unwrap();
            for (head, seg) in [
                ("agapao", "e-0040"),
                ("agape", "e-0041"),
                ("agapetos", "e-0042"),
            ] {
                b.add_section_map(&row("thayer", head, "thayer-1889", Some(seg), Equal))
                    .unwrap();
            }
            b.add_section_map(&row(
                "thayer-1889-pages",
                "p.6",
                "thayer-1889",
                Some("e-0040"),
                Partial,
            ))
            .unwrap();
            b.add_section_map(&row(
                "thayer-1889-pages",
                "p.6",
                "thayer-1889",
                Some("e-0041"),
                Equal,
            ))
            .unwrap();
            b.add_section_map(&row(
                "thayer-1889-pages",
                "p.7",
                "thayer-1889",
                Some("e-0042"),
                Equal,
            ))
            .unwrap();
            b.finish().unwrap();
            out.push(("thayer-1889".to_owned(), path));
        }
        // Thayer, a later headword-keyed digitization without page milestones.
        {
            let path = dir.join("thayer-ccel.sqlite");
            let mut b =
                PackBuilder::create(&path, &manifest("thayer-ccel", "thayer", "thayer"), &[])
                    .unwrap();
            for (head, seg) in [("agapao", "c-agapao"), ("agape", "c-agape")] {
                b.add_section_map(&row("thayer", head, "thayer-ccel", Some(seg), Equal))
                    .unwrap();
            }
            b.finish().unwrap();
            out.push(("thayer-ccel".to_owned(), path));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::fixture::temp_path;
    use openlogos_retrieval::{EditionResolution, ProvenanceKind, ReviewState};

    fn catalog_with_fixture() -> (Catalog, Vec<(String, PathBuf)>) {
        let dir = temp_path("catalog").parent().unwrap().to_path_buf();
        let packs = fixture::build_all(&dir);
        let mut catalog = Catalog::in_memory().unwrap();
        catalog.register_builtin_schemes().unwrap();
        for (id, path) in &packs {
            let access = if id == "thayer-ccel" {
                Access::Restricted
            } else {
                Access::Open
            };
            catalog.install_pack(id, path, access).unwrap();
        }
        (catalog, packs)
    }

    fn target(work: &str, locator: &str) -> Locator {
        Locator {
            work: work.into(),
            locator: locator.into(),
        }
    }

    #[test]
    fn installs_record_hash_manifest_and_section_rows() {
        let (catalog, packs) = catalog_with_fixture();
        let installed = catalog.installed().unwrap();
        assert_eq!(installed.len(), 5);
        let beveridge = installed
            .iter()
            .find(|p| p.pack_id == "institutes-beveridge")
            .unwrap();
        assert_eq!(beveridge.work.as_deref(), Some("institutes"));
        assert_eq!(beveridge.locator_scheme.as_deref(), Some("institutes"));
        assert_eq!(beveridge.section_rows, 4);
        assert_eq!(beveridge.sha256.len(), 64);
        assert_eq!(beveridge.sha256, sha256_file(&packs[1].1).unwrap());
        assert_eq!(
            catalog.editions_of("institutes").unwrap(),
            vec![
                ("institutes-allen".to_owned(), Access::Open),
                ("institutes-beveridge".to_owned(), Access::Open)
            ]
        );
        assert_eq!(
            catalog.canonical_scheme("thayer").unwrap().as_deref(),
            Some("thayer")
        );
        assert_eq!(catalog.schemes().unwrap().len(), 6);
        assert_eq!(catalog.pack_list().unwrap().len(), 5);
    }

    #[test]
    fn resolves_to_each_installed_edition_with_kinds_and_fallbacks() {
        let (catalog, _) = catalog_with_fixture();
        let status = catalog.resolve(&target("institutes", "3.2.7")).unwrap();
        let TargetStatus::InLibrary { resolution } = status else {
            panic!("expected in library");
        };
        assert_eq!(resolution.editions.len(), 2);
        assert_eq!(
            resolution.editions[0],
            EditionResolution {
                edition: "institutes-allen".into(),
                outcome: SectionOutcome::Mapped {
                    kind: SectionMapKind::Split,
                    segments: vec!["a-3-2-7a".into(), "a-3-2-7b".into()],
                    basis: ResolutionBasis::Direct
                }
            }
        );
        assert!(matches!(
            &resolution.editions[1].outcome,
            SectionOutcome::Mapped {
                kind: SectionMapKind::Equal,
                ..
            }
        ));
        // 3.2.9: merged in Beveridge, unmapped in Allen.
        let TargetStatus::InLibrary { resolution } =
            catalog.resolve(&target("institutes", "3.2.9")).unwrap()
        else {
            panic!()
        };
        assert_eq!(resolution.editions[0].outcome, SectionOutcome::Unmapped);
        assert!(matches!(
            &resolution.editions[1].outcome,
            SectionOutcome::Mapped {
                kind: SectionMapKind::Merge,
                ..
            }
        ));
        // 4.20.32: absent in Beveridge, unmapped in Allen: unresolved, but the editions' answers stay visible.
        let status = catalog.resolve(&target("institutes", "4.20.32")).unwrap();
        assert!(matches!(
            &status,
            TargetStatus::NotInLibrary {
                reason: NotInLibraryReason::Unresolved,
                resolution: Some(_)
            }
        ));
        assert_eq!(status.label(), "cited, not in your library");
        // A page citation: the print edition answers directly; the restricted CCEL text
        // resolves through the nearest sections but may not be shown.
        let TargetStatus::InLibrary { resolution } = catalog
            .resolve(&target("thayer", "thayer-1889-pages:p.6"))
            .unwrap()
        else {
            panic!()
        };
        assert!(matches!(
            &resolution.editions[0].outcome,
            SectionOutcome::Mapped {
                kind: SectionMapKind::Partial,
                basis: ResolutionBasis::Direct,
                ..
            }
        ));
        assert!(matches!(
            &resolution.editions[1].outcome,
            SectionOutcome::Mapped {
                basis: ResolutionBasis::NearestSection { .. },
                ..
            }
        ));
        // Not installed, unknown, restricted.
        assert!(matches!(
            catalog.resolve(&target("wlc", "1")).unwrap(),
            TargetStatus::NotInLibrary {
                reason: NotInLibraryReason::NotInstalled,
                resolution: None
            }
        ));
        assert!(matches!(
            catalog
                .resolve(&target("hodge-systematic", "1.1.1"))
                .unwrap(),
            TargetStatus::NotInLibrary {
                reason: NotInLibraryReason::UnknownWork,
                ..
            }
        ));
        let mut catalog = catalog;
        catalog.remove_pack("thayer-1889").unwrap();
        assert!(matches!(
            catalog.resolve(&target("thayer", "agape")).unwrap(),
            TargetStatus::NotInLibrary {
                reason: NotInLibraryReason::Restricted,
                resolution: Some(_)
            }
        ));
        catalog.set_access("thayer-ccel", Access::Licensed).unwrap();
        assert!(matches!(
            catalog.resolve(&target("thayer", "agape")).unwrap(),
            TargetStatus::InLibrary { .. }
        ));
        // A Bible edition: in the library when installed, by its own coordinates.
        let bible = temp_path("catalog-bible");
        crate::pack::fixture::build(&bible).unwrap();
        catalog
            .install_pack("sblgnt-test", &bible, Access::Open)
            .unwrap();
        assert!(matches!(
            catalog.resolve(&target("sblgnt-test", "44001001")).unwrap(),
            TargetStatus::InLibrary { .. }
        ));
    }

    #[test]
    fn cited_targets_keep_locator_and_context_whatever_the_status() {
        let (catalog, packs) = catalog_with_fixture();
        let mut index = crate::LinkIndex::in_memory().unwrap();
        for (id, path) in &packs {
            index.install_pack(id, path).unwrap();
        }
        let outlinks = index
            .outlinks_with_context(&target("institutes", "3.2.7"), 10)
            .unwrap();
        assert_eq!(outlinks.len(), 3);
        let mut statuses: Vec<(String, &'static str, Option<String>)> = outlinks
            .iter()
            .map(|(edge, context)| {
                let cited = catalog.cited_target(edge, context.as_deref()).unwrap();
                assert_eq!(cited.cited_by, target("institutes", "3.2.7"));
                (
                    cited.target.work.clone(),
                    cited.status.label(),
                    cited.context.clone(),
                )
            })
            .collect();
        statuses.sort();
        assert_eq!(
            statuses,
            vec![
                (
                    "hodge-systematic".to_owned(),
                    "cited, not in your library",
                    Some("(Hodge 1.1.1)".to_owned())
                ),
                (
                    "thayer".to_owned(),
                    "in your library",
                    Some("(Thayer p. 6)".to_owned())
                ),
                (
                    "wcf".to_owned(),
                    "in your library",
                    Some("faith is a firm and certain knowledge".to_owned())
                ),
            ]
        );
    }

    #[test]
    fn the_packager_report_counts_resolved_unresolved_and_unparsed_per_scheme() {
        let (catalog, _) = catalog_with_fixture();
        let grammar = CitationGrammar::english();
        let candidate = |text: &str, hint: Option<&str>| CitationCandidate {
            text: text.into(),
            scheme_hint: hint.map(str::to_owned),
        };
        let candidates = vec![
            candidate("WCF 1.6", None),
            candidate("WCF I.vi", None),
            candidate("WCF 2.1", None), // chapter 2 is not in the fixture map
            candidate("Inst. 3.2.7", None),
            candidate("Institutes III.ii.9", None),
            candidate("Institutes 4.20.32", None), // absent in Beveridge, unmapped in Allen
            candidate("Inst. 3.2.7.1", Some("institutes")), // too many levels
            candidate("Thayer p. 6", None),
            candidate("Thayer 6", Some("thayer-1889-pages")), // no page marker
            candidate("WLC Q. 1", None),                      // known scheme, nothing installed
            candidate("John 3:16", None),
            candidate("Hodge 1.1.1", None), // unknown work, no hint
        ];
        let report = catalog
            .resolution_report(
                "commentary-fixture",
                &candidates,
                &[
                    SchemeMinimum {
                        scheme: "wcf".into(),
                        minimum_resolved_rate: 0.9,
                    },
                    SchemeMinimum {
                        scheme: "institutes".into(),
                        minimum_resolved_rate: 0.5,
                    },
                ],
                &grammar,
            )
            .unwrap();
        let rates = |s: &str| {
            let r = report.scheme(s).unwrap();
            (r.resolved, r.unresolved_known_work, r.unparsed)
        };
        assert_eq!(rates("wcf"), (2, 1, 0));
        assert_eq!(rates("institutes"), (2, 1, 1));
        assert_eq!(rates("thayer-1889-pages"), (1, 0, 1));
        assert_eq!(rates("wlc"), (0, 1, 0));
        assert_eq!(rates("scripture"), (1, 0, 0));
        assert_eq!(rates("unknown"), (0, 0, 1));
        assert_eq!(
            report.grammar_version,
            openlogos_core::locator::GRAMMAR_VERSION
        );
        assert!(!report.passed);
        assert_eq!(report.failures.len(), 1);
        assert!(report.failures[0].starts_with("wcf: resolved 2 of 3"));
        assert_eq!(builtin_schemes().iter().filter(|s| s.canonical).count(), 5);
        // The schema example under examples/ is the built-in catalog, byte for byte in
        // meaning (skipped when the tree is filtered to crates/, as the nix checks are).
        let example = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/locator-scheme.json"
        );
        if let Ok(text) = std::fs::read_to_string(example) {
            let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(doc["schema_version"], "1.0");
            assert_eq!(
                doc["schemes"],
                serde_json::to_value(builtin_schemes()).unwrap()
            );
        } else {
            eprintln!("skipping {example}: not present in this source tree");
        }
        assert_eq!(
            edge_locator("thayer-1889-pages", false, "p.6"),
            "thayer-1889-pages:p.6"
        );
        assert_eq!(edge_locator("wcf", true, "1.6"), "1.6");
        let _ = ProvenanceKind::User;
        let _ = ReviewState::Published;
    }
}
