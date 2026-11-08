//! B057: the OpenBible cross references (S09) into a relation pack beside the preview pack's
//! text packs, `<out>/relations/openbible-xrefs.sqlite`, with a fidelity report
//! (`openbible-xrefs.json`), a spread to read by eye and the list latency.
//!
//! The source is one tab-separated file in a zip: a header `From Verse<TAB>To Verse<TAB>Votes
//! <TAB>#www.openbible.info CC-BY <date>`, then one row per link, `Gen.1.1<TAB>John.1.1-John.1.3
//! <TAB>384`. The importer keeps every row as published: the target range stays a range, the
//! votes stay the published integer (zero and negative included), the line stays the row's
//! locator. It invents no relevance model and no relation kind: the kind is `unspecified`.
//! Anything it does not understand stops the build.
//!
//! Its references are checked against the canon table rather than assumed `eng`: every one must
//! be a verse of `eng`, and the report counts those `org` lacks (Malachi 4, Joel 3, ...), which
//! is what tells the two apart. Mapping into each installed edition is counted over every row
//! with the same rules the core applies at read time.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use openlogos_core::canon;
use openlogos_core::reference::{BookId, Reference};
use openlogos_core::versification::{MappingStatus, SchemeId};
use openlogos_retrieval_sqlite::PackManifest;
use openlogos_retrieval_sqlite::cross_references::{
    CrossReferencePack, write_cross_reference_pack,
};
use openlogos_text::reading::pack::sha256_hex;
use openlogos_text::reading::{
    Availability, CrossReferenceInfo, CrossReferenceList, CrossReferenceOptions, CrossReferenceRow,
    DirSource, FilePack, ReadingLibrary,
};
use serde::Serialize;

use crate::bench::{Row, row};
use crate::build::{absolute, io, refuse_inside, unzip_entry};
use crate::model::{ImportError, parse_error};
use crate::rights::verify_source;

/// The rights record.
pub const SOURCE_ID: &str = "S09";
/// The source's directory under the corpora.
pub const CORPUS_DIR: &str = "S09-openbible-xrefs";
/// The zip the manifest lists.
pub const ZIP: &str = "cross-references.zip";
/// The data file inside it.
pub const FILE: &str = "cross_references.txt";
/// Relation packs, under the preview pack's output.
pub const RELATIONS_DIR: &str = "relations";
/// The pack.
pub const PACK_FILE: &str = "openbible-xrefs.sqlite";
/// The report.
pub const REPORT_FILE: &str = "openbible-xrefs.json";
/// The scheme the references are checked to be in.
pub const SCHEME: &str = "eng";

const HEADER: &str = "From Verse\tTo Verse\tVotes";
const CC_BY: &str = "https://creativecommons.org/licenses/by/4.0/";

/// What the votes are: the source's and the rights record's words, no more.
pub const VOTES_MEANING: &str = "The third column of OpenBible's file, as published. The labs page says the data's initial votes were seeded from OpenBible's Topical Bible and Twitter Bible Search; the rights record (S09) adds OpenBible.info users. A vote count is a popularity signal: not an accuracy score, a relevance model, or evidence of textual dependence. Zero and negative counts are published values and are kept; lists put them last because they sort by the number.";

/// The kind of every link: an imported traditional cross reference whose kind the source does
/// not identify (specs/17).
pub const KIND: &str = "unspecified";

/// One parsed file.
#[derive(Debug, Clone)]
pub struct Parsed {
    /// The header's trailing note (`#www.openbible.info CC-BY 2026-10-26`).
    pub note: String,
    /// The rows in file order; `line` is the file's line number.
    pub rows: Vec<CrossReferenceRow>,
}

fn reference(text: &str, line: u32) -> Result<Reference, ImportError> {
    let bad = |why: &str| parse_error(FILE, format!("line {line}: `{text}` {why}"));
    let mut parts = text.split('.');
    let (Some(book), Some(chapter), Some(verse), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(bad("is not Book.Chapter.Verse"));
    };
    let book = BookId::from_osis(book).ok_or_else(|| bad("names no book of the 66"))?;
    let number = |s: &str| {
        s.parse::<u32>()
            .ok()
            .filter(|n| *n > 0 && s.chars().all(|c| c.is_ascii_digit()))
    };
    let chapter = number(chapter).ok_or_else(|| bad("has no chapter number"))?;
    let verse = number(verse).ok_or_else(|| bad("has no verse number"))?;
    Ok(Reference::verse(book, chapter, verse))
}

/// Reads the file; any line that is not three tab-separated fields of the expected shape stops
/// the import.
pub fn parse(text: &str) -> Result<Parsed, ImportError> {
    let mut lines = text.split('\n');
    let header = lines.next().unwrap_or_default().trim_end_matches('\r');
    let note = header
        .strip_prefix(HEADER)
        .ok_or_else(|| parse_error(FILE, format!("header `{header}`")))?
        .trim_start_matches('\t')
        .to_owned();
    let mut rows = Vec::new();
    for (i, raw) in lines.enumerate() {
        let line = i as u32 + 2;
        let raw = raw.trim_end_matches('\r');
        if raw.is_empty() {
            continue;
        }
        let fields: Vec<&str> = raw.split('\t').collect();
        let [from, to, votes] = fields.as_slice() else {
            return Err(parse_error(
                FILE,
                format!("line {line}: {} fields", fields.len()),
            ));
        };
        let (a, b) = to.split_once('-').unwrap_or((to, to));
        let row = CrossReferenceRow {
            line,
            from: reference(from, line)?,
            to_start: reference(a, line)?,
            to_end: reference(b, line)?,
            votes: votes
                .parse()
                .map_err(|_| parse_error(FILE, format!("line {line}: votes `{votes}`")))?,
        };
        rows.push(row);
    }
    Ok(Parsed { note, rows })
}

/// Counts of one kind of reference against the canon table.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SchemeCheck {
    /// The scheme the references were found to be in.
    pub scheme: String,
    /// References checked: each source, each single target, both ends of each range.
    pub references: u64,
    /// Of them, verses of `eng`.
    pub in_eng: u64,
    /// Of them, verses `org` does not have: what shows the file is not in `org`.
    pub not_in_org: u64,
    /// Those by chapter, most first.
    pub not_in_org_by_chapter: Vec<(String, u64)>,
    /// Verses `eng` has past what the KJV prints (3 John 1:15, Rev 12:18) that the file uses.
    pub eng_edge_verses_used: Vec<(String, u64)>,
}

/// Duplicates, overlaps and reversals.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Shape {
    /// Rows.
    pub rows: u64,
    /// Verses with at least one row.
    pub source_verses: u64,
    /// Distinct targets (verse or range).
    pub distinct_targets: u64,
    /// Range targets.
    pub range_targets: u64,
    /// Of them, across a chapter boundary.
    pub ranges_across_chapters: u64,
    /// Of them, across a book boundary.
    pub ranges_across_books: u64,
    /// The longest range, in `eng` verses, and where.
    pub longest_range: (u64, String),
    /// Rows repeating another row's source and target.
    pub duplicate_rows: u64,
    /// A single-verse target of a source that also lists a range holding it.
    pub single_inside_range_of_same_source: u64,
    /// Rows whose target is their own source.
    pub self_links: u64,
    /// Ranges that end before they start.
    pub backwards_ranges: u64,
    /// Unordered verse pairs published both ways (A→B and B→A, single verses).
    pub reversed_pairs: u64,
    /// Rows A→B whose source lies inside a range B→A' published the other way.
    pub reversed_into_ranges: u64,
    /// Targets not in the 66 books (the parser stops on them, so 0 in a built pack).
    pub targets_out_of_canon: u64,
    /// Target verses past a chapter's end in `eng` (stops the build, so 0 in a built pack).
    pub targets_out_of_range: u64,
}

/// Votes.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Votes {
    /// Rows with a negative count.
    pub negative: u64,
    /// Rows with zero.
    pub zero: u64,
    /// Rows with a positive count.
    pub positive: u64,
    /// Smallest.
    pub min: i32,
    /// Largest.
    pub max: i32,
    /// Rows by bucket.
    pub histogram: Vec<(String, u64)>,
    /// How they are treated.
    pub treatment: String,
}

/// Every row's target placed in one installed edition.
#[derive(Debug, Clone, Default, Serialize)]
pub struct EditionMapping {
    /// The edition.
    pub resource_id: String,
    /// Its scheme.
    pub scheme: String,
    /// Source verses by [`Availability`] in the edition (as the target rules count them).
    pub source_verses: BTreeMap<String, u64>,
    /// Row targets by [`Availability`].
    pub targets: BTreeMap<String, u64>,
    /// Row targets by [`MappingStatus`].
    pub mapping: BTreeMap<String, u64>,
    /// Row targets whose number in the edition differs from the published one.
    pub renumbered: u64,
    /// A few targets of each kind that is not `printed`, as published → in the edition.
    pub examples: BTreeMap<String, Vec<String>>,
}

/// The built pack.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PackReport {
    /// Where it is.
    pub path: String,
    /// Bytes.
    pub bytes: u64,
    /// SHA-256.
    pub sha256: String,
    /// Edge rows.
    pub edges: u64,
    /// Range targets.
    pub ranges: u64,
    /// `xref_range` rows.
    pub range_rows: u64,
}

/// The whole report, `openbible-xrefs.json`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct XrefReport {
    /// What built it.
    pub built_by: String,
    /// `S09`.
    pub source_id: String,
    /// The record's edition id.
    pub edition_id: String,
    /// SHA-256 of the source's `MANIFEST.sha256`.
    pub source_manifest_sha256: String,
    /// The data file and its SHA-256 and size.
    pub file: (String, String, u64),
    /// The header's note.
    pub header_note: String,
    /// Scheme check.
    pub scheme: SchemeCheck,
    /// Shape.
    pub shape: Shape,
    /// Votes.
    pub votes: Votes,
    /// Per installed edition (none when the text packs are not built).
    pub editions: Vec<EditionMapping>,
    /// The pack.
    pub pack: PackReport,
    /// The attribution every item carries.
    pub attribution: String,
    /// Seconds in all.
    pub seconds: f64,
}

fn inc<K: Ord>(map: &mut BTreeMap<K, u64>, key: K) {
    *map.entry(key).or_default() += 1;
}

fn name<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn osis(start: &Reference, end: &Reference) -> String {
    if start == end {
        start.to_osis()
    } else {
        format!("{}-{}", start.to_osis(), end.to_osis())
    }
}

/// Checks every reference against the canon table: all must be `eng` verses.
pub fn check_scheme(rows: &[CrossReferenceRow]) -> Result<SchemeCheck, ImportError> {
    let eng = SchemeId::new(SCHEME);
    let org = SchemeId::org();
    let mut check = SchemeCheck {
        scheme: SCHEME.to_owned(),
        ..SchemeCheck::default()
    };
    let mut by_chapter: BTreeMap<String, u64> = BTreeMap::new();
    let mut edge: BTreeMap<String, u64> = BTreeMap::new();
    for row in rows {
        let ends: &[&Reference] = if row.to_start == row.to_end {
            &[&row.from, &row.to_start]
        } else {
            &[&row.from, &row.to_start, &row.to_end]
        };
        for r in ends.iter().copied() {
            check.references += 1;
            if !canon::contains(&eng, r) {
                return Err(parse_error(
                    FILE,
                    format!(
                        "line {}: {} is not a verse of {SCHEME}; the file is not in the scheme it was checked against",
                        row.line,
                        r.to_osis()
                    ),
                ));
            }
            check.in_eng += 1;
            if !canon::contains(&org, r) {
                check.not_in_org += 1;
                inc(
                    &mut by_chapter,
                    format!("{}.{}", r.book.as_osis(), r.chapter),
                );
            }
            if matches!(
                (r.book, r.chapter, r.verse),
                (BookId::John3, 1, Some(15)) | (BookId::Rev, 12, Some(18))
            ) {
                inc(&mut edge, r.to_osis());
            }
        }
    }
    let mut chapters: Vec<(String, u64)> = by_chapter.into_iter().collect();
    chapters.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    check.not_in_org_by_chapter = chapters;
    check.eng_edge_verses_used = edge.into_iter().collect();
    Ok(check)
}

/// Duplicates, overlaps, reversals and range shapes.
#[must_use]
pub fn shape(rows: &[CrossReferenceRow]) -> Shape {
    let eng = SchemeId::new(SCHEME);
    let mut s = Shape {
        rows: rows.len() as u64,
        ..Shape::default()
    };
    let mut sources = BTreeSet::new();
    let mut targets = BTreeSet::new();
    let mut pairs: HashMap<(&Reference, &Reference, &Reference), u64> = HashMap::new();
    let mut singles: HashMap<(&Reference, &Reference), ()> = HashMap::new();
    let mut ranges_by_source: HashMap<&Reference, Vec<(&Reference, &Reference)>> = HashMap::new();
    for row in rows {
        sources.insert(&row.from);
        targets.insert((&row.to_start, &row.to_end));
        *pairs
            .entry((&row.from, &row.to_start, &row.to_end))
            .or_default() += 1;
        if row.to_start == row.to_end {
            singles.insert((&row.from, &row.to_start), ());
            if row.to_start == row.from {
                s.self_links += 1;
            }
        } else {
            s.range_targets += 1;
            if row.to_end < row.to_start {
                s.backwards_ranges += 1;
            }
            if row.to_start.book != row.to_end.book {
                s.ranges_across_books += 1;
            }
            if row.to_start.book != row.to_end.book || row.to_start.chapter != row.to_end.chapter {
                s.ranges_across_chapters += 1;
            }
            let length = openlogos_retrieval_sqlite::cross_references::verses_between(
                SCHEME,
                &row.to_start,
                &row.to_end,
            )
            .map_or(0, |v| v.len() as u64);
            if length > s.longest_range.0 {
                s.longest_range = (
                    length,
                    format!("line {}: {}", row.line, osis(&row.to_start, &row.to_end)),
                );
            }
            ranges_by_source
                .entry(&row.from)
                .or_default()
                .push((&row.to_start, &row.to_end));
        }
        for r in [&row.to_start, &row.to_end] {
            if !canon::contains(&eng, r) {
                if canon::chapter_count(&eng, r.book).is_none() {
                    s.targets_out_of_canon += 1;
                } else {
                    s.targets_out_of_range += 1;
                }
            }
        }
    }
    s.source_verses = sources.len() as u64;
    s.distinct_targets = targets.len() as u64;
    s.duplicate_rows = pairs.values().map(|n| n - 1).sum();
    for row in rows {
        if row.to_start != row.to_end {
            continue;
        }
        if let Some(ranges) = ranges_by_source.get(&row.from)
            && ranges
                .iter()
                .any(|(a, b)| *a <= &row.to_start && &row.to_start <= *b)
        {
            s.single_inside_range_of_same_source += 1;
        }
        if singles.contains_key(&(&row.to_start, &row.from)) && row.from < row.to_start {
            s.reversed_pairs += 1;
        }
        if let Some(ranges) = ranges_by_source.get(&row.to_start)
            && ranges
                .iter()
                .any(|(a, b)| *a <= &row.from && &row.from <= *b)
        {
            s.reversed_into_ranges += 1;
        }
    }
    s
}

/// Vote counts.
#[must_use]
pub fn votes(rows: &[CrossReferenceRow]) -> Votes {
    let mut v = Votes {
        min: i32::MAX,
        max: i32::MIN,
        treatment: "kept as published; never dropped, clamped or rescaled; lists order by the number, so zero and negative rows come last; the host may fold them but must say how many".to_owned(),
        ..Votes::default()
    };
    let buckets: [(&str, i32, i32); 7] = [
        ("< 0", i32::MIN, -1),
        ("0", 0, 0),
        ("1", 1, 1),
        ("2", 2, 2),
        ("3–9", 3, 9),
        ("10–99", 10, 99),
        ("≥ 100", 100, i32::MAX),
    ];
    let mut counts = [0u64; 7];
    for row in rows {
        let n = row.votes;
        v.min = v.min.min(n);
        v.max = v.max.max(n);
        match n.cmp(&0) {
            std::cmp::Ordering::Less => v.negative += 1,
            std::cmp::Ordering::Equal => v.zero += 1,
            std::cmp::Ordering::Greater => v.positive += 1,
        }
        if let Some(i) = buckets
            .iter()
            .position(|(_, lo, hi)| (*lo..=*hi).contains(&n))
        {
            counts[i] += 1;
        }
    }
    v.histogram = buckets
        .iter()
        .zip(counts)
        .map(|((label, _, _), n)| ((*label).to_owned(), n))
        .collect();
    v
}

/// The text packs of a built preview pack, if they are there.
pub fn load_editions(out: &Path) -> Result<ReadingLibrary, ImportError> {
    let pack_error = |e: &dyn std::fmt::Display| ImportError::Pack(e.to_string());
    let mut library = ReadingLibrary::new();
    for id in ["bsb", "sblgnt", "wlc"] {
        let directory = out.join("packs").join(id);
        let manifest_path = directory.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }
        let manifest =
            std::fs::read_to_string(&manifest_path).map_err(|e| io(&manifest_path, e))?;
        let pack = FilePack::from_manifest(&manifest, Some(Box::new(DirSource::new(&directory))))
            .map_err(|e| pack_error(&e))?;
        library.insert(Box::new(pack));
    }
    Ok(library)
}

/// Every row placed in every edition of `library`.
pub fn map_editions(
    library: &ReadingLibrary,
    rows: &[CrossReferenceRow],
) -> Result<Vec<EditionMapping>, ImportError> {
    let pack_error = |e: &dyn std::fmt::Display| ImportError::Pack(e.to_string());
    let mut out = Vec::new();
    for edition in library.editions() {
        let id = edition.resource_id.clone();
        let mut m = EditionMapping {
            resource_id: id.clone(),
            scheme: edition.scheme.clone(),
            ..EditionMapping::default()
        };
        let mut seen = BTreeSet::new();
        for row in rows {
            if seen.insert(&row.from) {
                let (_, _, availability) = library
                    .place_cross_reference(&id, SCHEME, &row.from, &row.from)
                    .map_err(|e| pack_error(&e))?;
                inc(&mut m.source_verses, name(&availability));
            }
            let (target, mapping, availability) = library
                .place_cross_reference(&id, SCHEME, &row.to_start, &row.to_end)
                .map_err(|e| pack_error(&e))?;
            let published = osis(&row.to_start, &row.to_end);
            let kind = name(&availability);
            inc(&mut m.targets, kind.clone());
            inc(&mut m.mapping, name(&mapping));
            if target.as_deref().is_some_and(|t| t != published) {
                m.renumbered += 1;
            }
            if availability != Availability::Printed
                && availability != Availability::BookNotInEdition
            {
                let examples = m.examples.entry(kind).or_default();
                let example = format!(
                    "{published} → {}",
                    target.unwrap_or_else(|| "unmapped".to_owned())
                );
                if examples.len() < 12 && !examples.contains(&example) {
                    examples.push(example);
                }
            }
            if mapping != MappingStatus::Exact {
                let examples = m
                    .examples
                    .entry(format!("mapping {}", name(&mapping)))
                    .or_default();
                let example = published.clone();
                if examples.len() < 12 && !examples.contains(&example) {
                    examples.push(example);
                }
            }
        }
        out.push(m);
    }
    Ok(out)
}

/// What a build is told.
#[derive(Debug, Clone)]
pub struct XrefOptions {
    /// The corpora directory; only read.
    pub corpora: PathBuf,
    /// The repository's `rights/`.
    pub rights: PathBuf,
    /// The preview pack's output; the pack goes to its `relations/`.
    pub out: PathBuf,
}

/// The attribution of the rights record, the licence, and what was changed.
fn attribution(record: &str) -> String {
    format!(
        "{record} Licence: {CC_BY}. Changes: rows converted into an OpenLogos relation pack; references, ranges and votes as published; shown in each edition's verse numbering. Provided as is, without warranties."
    )
}

/// The whole import: rights and hashes, parse, checks, pack, report.
pub fn build(options: &XrefOptions, repository: &Path) -> Result<XrefReport, ImportError> {
    let started = Instant::now();
    let out = absolute(&options.out)?;
    refuse_inside(&out, &absolute(repository)?, "the repository")?;
    refuse_inside(&out, &absolute(&options.corpora)?, "the corpora")?;
    let source = verify_source(
        SOURCE_ID,
        &options.rights,
        &options.corpora.join(CORPUS_DIR),
        |p| p == ZIP,
    )?;
    let relations = out.join(RELATIONS_DIR);
    std::fs::create_dir_all(&relations).map_err(|e| io(&relations, e))?;
    let zip = relations.join(format!(".{ZIP}.building"));
    let zip_bytes = source
        .files
        .get(ZIP)
        .ok_or_else(|| parse_error(ZIP, "not in the verified source"))?;
    std::fs::write(&zip, zip_bytes).map_err(|e| io(&zip, e))?;
    let text = unzip_entry(&zip, FILE);
    std::fs::remove_file(&zip).map_err(|e| io(&zip, e))?;
    let text = text?;
    let parsed = parse(&text)?;
    let scheme = check_scheme(&parsed.rows)?;
    let shape = shape(&parsed.rows);
    if shape.targets_out_of_canon + shape.targets_out_of_range + shape.backwards_ranges > 0 {
        return Err(ImportError::Oracle(format!(
            "{} targets out of canon, {} out of range, {} backwards ranges: not built",
            shape.targets_out_of_canon, shape.targets_out_of_range, shape.backwards_ranges
        )));
    }
    let votes = votes(&parsed.rows);
    let attribution = attribution(&source.attribution);
    let info = CrossReferenceInfo {
        source_id: SOURCE_ID.to_owned(),
        edition_id: source.edition_id.clone(),
        scheme: SCHEME.to_owned(),
        license: source.license.clone(),
        attribution: attribution.clone(),
        relation: "cross_reference".to_owned(),
        kind: KIND.to_owned(),
        votes_meaning: VOTES_MEANING.to_owned(),
        review_state: "source_attested".to_owned(),
        coverage: format!(
            "{} published links from {} verses of the 66-book Protestant canon, in {SCHEME} numbering; no verse text. Not reviewed by OpenLogos (specs/17: a human reviews representative links before release).",
            parsed.rows.len(),
            shape.source_verses
        ),
        rows: parsed.rows.len() as u64,
    };
    let manifest = PackManifest {
        edition: "openbible-xrefs".to_owned(),
        scheme: SCHEME.to_owned(),
        source: format!(
            "{} ({FILE} in {ZIP}, MANIFEST.sha256 {})",
            source.upstream_url, source.manifest_sha256
        ),
        license: source.license.clone(),
        attribution: attribution.clone(),
        work: None,
        locator_scheme: None,
    };
    let file_sha = sha256_hex(text.as_bytes());
    let building = relations.join(format!("{PACK_FILE}.building"));
    if building.exists() {
        std::fs::remove_file(&building).map_err(|e| io(&building, e))?;
    }
    let written = write_cross_reference_pack(
        &building,
        &manifest,
        &info,
        &[
            ("xref_file", FILE),
            ("xref_file_sha256", file_sha.as_str()),
            ("xref_header_note", parsed.note.as_str()),
        ],
        &parsed.rows,
    )
    .map_err(|e| ImportError::Pack(e.to_string()))?;
    let pack_path = relations.join(PACK_FILE);
    std::fs::rename(&building, &pack_path).map_err(|e| io(&pack_path, e))?;
    let pack_bytes = std::fs::read(&pack_path).map_err(|e| io(&pack_path, e))?;

    let library = load_editions(&out)?;
    let editions = map_editions(&library, &parsed.rows)?;
    let report = XrefReport {
        built_by: "just preview-pack-xrefs".to_owned(),
        source_id: SOURCE_ID.to_owned(),
        edition_id: source.edition_id.clone(),
        source_manifest_sha256: source.manifest_sha256.clone(),
        file: (FILE.to_owned(), file_sha, text.len() as u64),
        header_note: parsed.note,
        scheme,
        shape,
        votes,
        editions,
        pack: PackReport {
            path: pack_path.display().to_string(),
            bytes: pack_bytes.len() as u64,
            sha256: sha256_hex(&pack_bytes),
            edges: written.edges,
            ranges: written.ranges,
            range_rows: written.range_rows,
        },
        attribution,
        seconds: started.elapsed().as_secs_f64(),
    };
    let json =
        serde_json::to_string_pretty(&report).map_err(|e| ImportError::Pack(e.to_string()))?;
    let report_path = relations.join(REPORT_FILE);
    std::fs::write(&report_path, json + "\n").map_err(|e| io(&report_path, e))?;
    Ok(report)
}

/// The text packs and the cross-reference pack of a built preview pack.
pub fn open(out: &Path) -> Result<ReadingLibrary, ImportError> {
    let mut library = load_editions(out)?;
    let path = out.join(RELATIONS_DIR).join(PACK_FILE);
    let pack = CrossReferencePack::open(&path).map_err(ImportError::Pack)?;
    library.set_cross_references(Box::new(pack));
    Ok(library)
}

/// The spread read by eye (docs/evidence/B057.md): passage, edition and whether to include
/// reverse links.
pub const SPREAD: &[(&str, &str, bool)] = &[
    ("Gen 1:1", "bsb", false),
    ("Ps 23", "bsb", false),
    ("Isa 53", "bsb", false),
    ("John 3:16", "bsb", false),
    ("John 3:16", "sblgnt", true),
    ("Rom 8:28", "bsb", false),
    ("Rom 8:28", "sblgnt", false),
    ("Mal 4:5", "bsb", false),
    ("Mal 3:23", "wlc", false),
    ("Ps 51:1", "wlc", false),
    ("Ps 51:3", "wlc", false),
    ("Eph 3:20", "sblgnt", false),
    ("Rom 16:25", "bsb", true),
];

/// One passage's list as lines: `votes  label  [mapping, availability, direction] (from …)`.
#[must_use]
pub fn render(list: &CrossReferenceList, limit: usize, with_preview: bool) -> String {
    let mut out = format!(
        "{} in {}: {} items ({} published), passage in {} = {} [{}]\n",
        list.reference,
        list.resource_id,
        list.total,
        list.published,
        list.source.scheme,
        list.source_verses.join(" "),
        name(&list.mapping)
    );
    for item in list.items.iter().take(limit) {
        let renumbered = if item.label == item.source_label {
            String::new()
        } else {
            format!(" (published {})", item.source_label)
        };
        let froms: Vec<String> = item
            .assertions
            .iter()
            .map(|a| {
                format!(
                    "{}→{} {} line {} {}",
                    a.from,
                    a.to,
                    a.votes,
                    a.line,
                    name(&a.direction)
                )
            })
            .collect();
        out.push_str(&format!(
            "  {:>5}  {}{}  [{}, {}, {}] in: {:?}\n         {}\n",
            item.votes,
            item.label,
            renumbered,
            name(&item.mapping),
            name(&item.availability),
            name(&item.direction),
            item.available_in,
            froms.join("; ")
        ));
        if with_preview && let Some(p) = &item.preview {
            out.push_str(&format!(
                "         “{}”\n",
                p.chars().take(90).collect::<String>()
            ));
        }
    }
    out
}

/// The spread, rendered.
pub fn show(out: &Path, limit: usize, with_preview: bool) -> Result<String, ImportError> {
    let library = open(out)?;
    let mut text = String::new();
    for (passage, edition, reverse) in SPREAD {
        let list = library
            .cross_references(
                edition,
                passage,
                &CrossReferenceOptions {
                    include_reverse: *reverse,
                    ..CrossReferenceOptions::default()
                },
            )
            .map_err(|e| ImportError::Pack(format!("{passage} in {edition}: {e}")))?;
        text.push_str(&render(&list, limit, with_preview));
    }
    Ok(text)
}

/// The page a host asks for first.
pub const PAGE: usize = 25;

/// The list latency: one call per verse for a deterministic spread of verses in each edition,
/// default options (published only, three verses of preview), then with reverse links, then
/// whole chapters.
pub fn bench(out: &Path, stride: usize) -> Result<Vec<Row>, ImportError> {
    let library = open(out)?;
    let stride = stride.max(1);
    let mut rows = Vec::new();
    for edition in library.editions() {
        let id = edition.resource_id.clone();
        let mut verses = Vec::new();
        let mut chapters = Vec::new();
        for book in library
            .books(&id)
            .map_err(|e| ImportError::Pack(e.to_string()))?
        {
            for chapter in &book.chapters {
                chapters.push(format!("{}.{}", book.book.as_osis(), chapter.chapter));
                for v in chapter.first_verse.max(1)..=chapter.last_verse {
                    verses.push(format!("{}.{}.{v}", book.book.as_osis(), chapter.chapter));
                }
            }
        }
        // The heaviest verse of the source, and the spread's, are always in.
        let mut sample: Vec<String> = verses.iter().step_by(stride).cloned().collect();
        for extra in [
            "Gen.1.1",
            "John.3.16",
            "Rom.8.28",
            "Ps.23.1",
            "Isa.53.5",
            "Prov.3.5",
        ] {
            sample.push(extra.to_owned());
        }
        for (label, reverse, preview_verses, limit) in [
            ("verse, first page of 25", false, 3, Some(PAGE)),
            ("verse, whole list", false, 3, None),
            ("verse, whole list, no preview text", false, 0, None),
            (
                "verse with reverse links, first page of 25",
                true,
                3,
                Some(PAGE),
            ),
            ("verse with reverse links, whole list", true, 3, None),
        ] {
            let mut samples = Vec::new();
            for verse in &sample {
                let options = CrossReferenceOptions {
                    include_reverse: reverse,
                    preview_verses,
                    limit,
                    ..CrossReferenceOptions::default()
                };
                let started = Instant::now();
                let result = library.cross_references(&id, verse, &options);
                let ms = started.elapsed().as_secs_f64() * 1000.0;
                match result {
                    Ok(l) => samples.push((ms, format!("{verse} ({} items)", l.total))),
                    // A verse of another testament in a one-testament pack.
                    Err(_) => continue,
                }
            }
            rows.push(row(&format!("{id}: {label}"), samples));
        }
        let mut samples = Vec::new();
        for chapter in chapters.iter().step_by(stride.div_ceil(25).max(1)) {
            let started = Instant::now();
            let options = CrossReferenceOptions {
                limit: Some(PAGE),
                ..CrossReferenceOptions::default()
            };
            let result = library.cross_references(&id, chapter, &options);
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            if let Ok(l) = result {
                samples.push((ms, format!("{chapter} ({} items)", l.total)));
            }
        }
        rows.push(row(
            &format!("{id}: whole chapter, first page of 25"),
            samples,
        ));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The published format, hand-written: header, a single target, a range, a range across
    /// books, zero and negative votes, a reversed pair.
    const SNIPPET: &str = "From Verse\tTo Verse\tVotes\t#www.openbible.info CC-BY 2026-10-26\nGen.1.1\tJohn.1.1-John.1.3\t384\nGen.1.1\tHeb.11.3\t-2\nHeb.11.3\tGen.1.1\t0\nIsa.44.28\t2Chr.36.22-Ezra.1.3\t13\nMal.4.5\tLuke.1.17\t29\n";

    #[test]
    fn the_snippet_parses_with_its_lines() {
        let parsed = parse(SNIPPET).unwrap();
        assert_eq!(parsed.note, "#www.openbible.info CC-BY 2026-10-26");
        assert_eq!(parsed.rows.len(), 5);
        assert_eq!(parsed.rows[0].line, 2);
        assert_eq!(parsed.rows[0].to_end.to_osis(), "John.1.3");
        assert_eq!(parsed.rows[1].votes, -2);
        assert_eq!(parsed.rows[3].to_start.book, BookId::Chr2);
    }

    #[test]
    fn anything_unexpected_stops_the_parse() {
        for bad in [
            "From\tTo\nGen.1.1\tGen.1.2\t1\n",
            "From Verse\tTo Verse\tVotes\nGen.1.1\tGen.1.2\n",
            "From Verse\tTo Verse\tVotes\nGen.1\tGen.1.2\t1\n",
            "From Verse\tTo Verse\tVotes\nTob.1.1\tGen.1.2\t1\n",
            "From Verse\tTo Verse\tVotes\nGen.1.1\tGen.1.2\tmany\n",
            "From Verse\tTo Verse\tVotes\nGen.1.0\tGen.1.2\t1\n",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_scheme_check_counts_what_org_lacks_and_refuses_what_eng_lacks() {
        let rows = parse(SNIPPET).unwrap().rows;
        let check = check_scheme(&rows).unwrap();
        assert_eq!(check.references, 12);
        assert_eq!(check.in_eng, 12);
        assert_eq!(check.not_in_org, 1);
        assert_eq!(check.not_in_org_by_chapter, [("Mal.4".to_owned(), 1)]);
        let past = parse("From Verse\tTo Verse\tVotes\nJohn.1.52\tGen.1.1\t1\n").unwrap();
        assert!(check_scheme(&past.rows).is_err());
    }

    #[test]
    fn shape_and_votes_count_pairs_ranges_and_signs() {
        let rows = parse(SNIPPET).unwrap().rows;
        let s = shape(&rows);
        assert_eq!(s.rows, 5);
        assert_eq!(s.source_verses, 4);
        assert_eq!(s.range_targets, 2);
        assert_eq!(s.ranges_across_books, 1);
        assert_eq!(s.ranges_across_chapters, 1);
        assert_eq!(s.reversed_pairs, 1);
        assert_eq!(s.duplicate_rows, 0);
        assert_eq!(s.longest_range.0, 5);
        let v = votes(&rows);
        assert_eq!((v.negative, v.zero, v.positive), (1, 1, 3));
        assert_eq!((v.min, v.max), (-2, 384));
    }
}
