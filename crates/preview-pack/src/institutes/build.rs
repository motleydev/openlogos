//! The B016 build: rights gate, both volumes, citations, the work pack and its report, outside
//! the repository. Also the benchmark and the by-eye spread (`--show`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use openlogos_retrieval_sqlite::EditionSearch;
use openlogos_retrieval_sqlite::work_pack::{
    CitationRow, CitationStatus, NoteKind, WorkBibliography, WorkContent, WorkInfo, WorkPack,
    WorkVolume, write_work_pack,
};
use openlogos_workspaces::palette::{DEFAULT_SECTION_LIMIT, EXPANDED_SECTION_LIMIT, TextProvider};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::citations::{EXTRA_ALIASES, Outcome, alias_table, extract};
use super::{Finding, LeftOut, ParsedWork, TextCounters, parse_volumes, split_digitized};
use crate::bench::{Row, keystrokes, percentile};
use crate::model::{ImportError, parse_error};
use crate::rights::verify_source;

/// The rights record.
pub const SOURCE_ID: &str = "S75";

/// The corpus directory under the corpora root.
pub const CORPUS_DIR: &str = "S75-gutenberg-calvin-institutes";

/// The edition's resource id.
pub const RESOURCE_ID: &str = "institutes-allen";

/// What produced the citation links; stored on every edge.
pub const CITATION_METHOD: &str = "b016-allen-notes@1 (core reference parser, grammar b068-1)";

const VOLUMES: [(&str, &str); 2] = [
    ("pg45001.txt", "pg45001-record.html"),
    ("pg64392.txt", "pg64392-record.html"),
];

fn io(path: &Path, error: std::io::Error) -> ImportError {
    ImportError::Io {
        path: path.display().to_string(),
        error,
    }
}

/// Where to read and write.
#[derive(Debug, Clone)]
pub struct InstitutesOptions {
    /// The corpora root (`/Volumes/office-hub/openlogos/corpora`).
    pub corpora: PathBuf,
    /// `rights/`.
    pub rights: PathBuf,
    /// The output directory (`~/.cache/openlogos/works`).
    pub out: PathBuf,
}

/// Sections of one chapter-level unit.
#[derive(Debug, Clone, Serialize)]
pub struct ChapterCount {
    /// `3.2`.
    pub locator: String,
    /// Title as printed.
    pub title: Option<String>,
    /// Sections.
    pub sections: usize,
    /// First and last printed numbers, for a chapter.
    pub numbers: Option<(u32, u32)>,
    /// Notes in it.
    pub notes: usize,
    /// Citations in it, and how many resolved.
    pub citations: (usize, usize),
}

/// Citation counts.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CitationReport {
    /// Citations found.
    pub total: usize,
    /// Resolved.
    pub resolved: usize,
    /// `resolved / total`.
    pub rate: f64,
    /// `resolved / (total - unknown_name)`: the rate over what names a book at all.
    pub rate_named_books: f64,
    /// Unresolved per class.
    pub unresolved: BTreeMap<String, usize>,
    /// Resolved citations that are a whole chapter.
    pub chapter_only: usize,
    /// Resolved citations ending `&c.`.
    pub and_following: usize,
    /// Bible ranges linked (edges).
    pub ranges: usize,
    /// Notes per kind.
    pub notes_by_kind: BTreeMap<String, usize>,
    /// Up to ten unresolved per class: `(section, note, text, detail)`.
    pub samples: BTreeMap<String, Vec<(String, String, String, String)>>,
    /// The spellings added to the English alias table.
    pub extra_aliases: Vec<String>,
}

/// What the build did.
#[derive(Debug, Clone, Serialize)]
pub struct InstitutesReport {
    /// `just institutes-pack`.
    pub built_by: String,
    /// `S75`.
    pub source_id: String,
    /// The record's edition id.
    pub edition_id: String,
    /// SHA-256 of the source's `MANIFEST.sha256`.
    pub source_manifest_sha256: String,
    /// Files the gate hashed.
    pub source_files_checked: usize,
    /// The pack.
    pub pack: String,
    /// Its size.
    pub pack_bytes: u64,
    /// Its SHA-256.
    pub pack_sha256: String,
    /// The edition's content revision.
    pub content_revision: String,
    /// Sections by division.
    pub sections_by_division: BTreeMap<String, usize>,
    /// Every chapter-level unit.
    pub chapters: Vec<ChapterCount>,
    /// Anomalies of the source.
    pub findings: Vec<Finding>,
    /// Paragraph reconstruction.
    pub counters: TextCounters,
    /// What was left out.
    pub left_out: LeftOut,
    /// Notes per volume.
    pub notes_per_volume: [usize; 2],
    /// Citations.
    pub citations: CitationReport,
    /// The catalog's evidence.
    pub bibliography: WorkBibliography,
    /// Lines of the files that ended in CR LF.
    pub crlf_lines: usize,
    /// Seconds reading and checking the sources.
    pub verify_seconds: f64,
    /// Seconds in all.
    pub total_seconds: f64,
}

/// Text inside the first tag pair after `from`, with tags removed and entities decoded.
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    let decoded = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The rows of the record page's bibliographic table: `(heading, value)` in order.
pub fn record_rows(html: &str) -> Vec<(String, String)> {
    let Some(start) = html.find("class=\"bibrec\"") else {
        return Vec::new();
    };
    let table = &html[start..];
    let table = &table[..table.find("</table>").unwrap_or(table.len())];
    let mut rows = Vec::new();
    for row in table.split("<tr").skip(1) {
        let (Some(th), Some(td)) = (row.find("<th"), row.find("<td")) else {
            continue;
        };
        let th_end = row[th..].find("</th>").map_or(row.len(), |e| th + e);
        let td_end = row[td..].find("</td>").map_or(row.len(), |e| td + e);
        let head = strip_tags(&format!("{}>", &row[th..th_end]));
        let value = strip_tags(&format!("{}>", &row[td..td_end]));
        rows.push((head, value));
    }
    rows
}

fn first(rows: &[(String, String)], key: &str) -> String {
    rows.iter()
        .find(|(k, _)| k == key)
        .map_or_else(String::new, |(_, v)| v.clone())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn refuse_inside(out: &Path, parent: &Path, what: &str) -> Result<(), ImportError> {
    if out.starts_with(parent) {
        return Err(ImportError::Usage(format!(
            "the output {} is inside {what}; a work pack never goes there",
            out.display()
        )));
    }
    Ok(())
}

/// The whole build.
pub fn build(
    options: &InstitutesOptions,
    repository: &Path,
) -> Result<InstitutesReport, ImportError> {
    let started = Instant::now();
    let absolute = |p: &Path| std::path::absolute(p).map_err(|e| io(p, e));
    let out = absolute(&options.out)?;
    refuse_inside(&out, &absolute(repository)?, "the repository")?;
    refuse_inside(&out, &absolute(&options.corpora)?, "the corpora")?;
    let corpus = options.corpora.join(CORPUS_DIR);
    let source = verify_source(SOURCE_ID, &options.rights, &corpus, |_| true)?;
    let verify_seconds = started.elapsed().as_secs_f64();

    let text = |name: &str| -> Result<&str, ImportError> {
        let bytes = source
            .files
            .get(name)
            .ok_or_else(|| parse_error(name, "not in the manifest"))?;
        std::str::from_utf8(bytes).map_err(|e| parse_error(name, format!("not UTF-8: {e}")))
    };
    let v1 = split_digitized(VOLUMES[0].0, text(VOLUMES[0].0)?)?;
    let v2 = split_digitized(VOLUMES[1].0, text(VOLUMES[1].0)?)?;
    let parsed = parse_volumes(&v1, &v2)?;
    // The licence's condition for leaving it out: no reference to the digitizer remains.
    for section in &parsed.sections {
        let texts = section
            .paragraphs
            .iter()
            .map(|p| p.text.as_str())
            .chain(section.notes.iter().map(|n| n.text.as_str()));
        for t in texts {
            if t.to_lowercase().contains("gutenberg") {
                return Err(parse_error(
                    "",
                    format!("section {} still names the digitizer", section.locator),
                ));
            }
        }
    }

    let records: Vec<Vec<(String, String)>> = VOLUMES
        .iter()
        .map(|(_, record)| text(record).map(record_rows))
        .collect::<Result<_, _>>()?;
    let bibliography = bibliography(&records, &v1.header, &v2.header);
    let (content, citations) = assemble(&parsed, &source, bibliography.clone())?;

    std::fs::create_dir_all(&out).map_err(|e| io(&out, e))?;
    let building = out.join(format!("{RESOURCE_ID}.building.sqlite"));
    if building.exists() {
        std::fs::remove_file(&building).map_err(|e| io(&building, e))?;
    }
    write_work_pack(&building, &content).map_err(|e| ImportError::Pack(e.to_string()))?;
    // Read back what was written: every section, its paragraphs and notes.
    let check = WorkPack::open(&building).map_err(|e| ImportError::Pack(e.to_string()))?;
    for section in &content.sections {
        let read = check
            .section(&section.locator)
            .map_err(|e| ImportError::Pack(e.to_string()))?;
        if read != *section {
            return Err(ImportError::Pack(format!(
                "section {} reads back differently",
                section.locator
            )));
        }
    }
    drop(check);
    let pack_bytes = std::fs::read(&building).map_err(|e| io(&building, e))?;
    let final_path = out.join(format!("{RESOURCE_ID}.sqlite"));

    let mut sections_by_division = BTreeMap::new();
    let mut chapters = Vec::new();
    for division in &content.structure {
        for chapter in &division.chapters {
            *sections_by_division
                .entry(division.locator.clone())
                .or_insert(0) += chapter.sections.len();
            let sections: Vec<_> = content
                .sections
                .iter()
                .filter(|s| s.chapter == chapter.locator)
                .collect();
            let numbers: Vec<u32> = sections.iter().filter_map(|s| s.number).collect();
            let cites: Vec<_> = content
                .citations
                .iter()
                .filter(|c| sections.iter().any(|s| s.locator == c.link.section))
                .collect();
            chapters.push(ChapterCount {
                locator: chapter.locator.clone(),
                title: chapter.title.clone(),
                sections: sections.len(),
                numbers: numbers.first().zip(numbers.last()).map(|(a, b)| (*a, *b)),
                notes: sections.iter().map(|s| s.notes.len()).sum(),
                citations: (
                    cites.len(),
                    cites
                        .iter()
                        .filter(|c| c.link.status == CitationStatus::Resolved)
                        .count(),
                ),
            });
        }
    }
    let report = InstitutesReport {
        built_by: "just institutes-pack".to_owned(),
        source_id: source.source_id.clone(),
        edition_id: source.edition_id.clone(),
        source_manifest_sha256: source.manifest_sha256.clone(),
        source_files_checked: source.files_checked,
        pack: final_path.display().to_string(),
        pack_bytes: pack_bytes.len() as u64,
        pack_sha256: sha256_hex(&pack_bytes),
        content_revision: content.info.content_revision.clone(),
        sections_by_division,
        chapters,
        findings: parsed.findings.clone(),
        counters: parsed.counters.clone(),
        left_out: parsed.left_out.clone(),
        notes_per_volume: parsed.notes_per_volume,
        citations,
        bibliography,
        crlf_lines: v1.crlf_lines + v2.crlf_lines,
        verify_seconds,
        total_seconds: started.elapsed().as_secs_f64(),
    };
    let report_path = out.join(format!("{RESOURCE_ID}.json"));
    let json =
        serde_json::to_string_pretty(&report).map_err(|e| ImportError::Pack(e.to_string()))?;
    std::fs::write(&report_path, json + "\n").map_err(|e| io(&report_path, e))?;
    std::fs::rename(&building, &final_path).map_err(|e| io(&final_path, e))?;
    Ok(report)
}

fn bibliography(
    records: &[Vec<(String, String)>],
    h1: &BTreeMap<String, String>,
    h2: &BTreeMap<String, String>,
) -> WorkBibliography {
    let _ = (h1, h2);
    let volumes = records
        .iter()
        .enumerate()
        .map(|(i, rows)| WorkVolume {
            volume: i as u32 + 1,
            title: first(rows, "Title"),
            identifier: format!("eBook #{}", first(rows, "eBook-No.")),
            release_date: first(rows, "Release Date"),
            last_update: first(rows, "Last Update"),
            credits: first(rows, "Credits"),
        })
        .collect();
    let subjects = records[0]
        .iter()
        .filter(|(k, _)| k == "Subject")
        .map(|(_, v)| v.clone())
        .collect();
    WorkBibliography {
        author: first(&records[0], "Author"),
        translator: first(&records[0], "Translator"),
        edition: "Sixth American Edition, Revised and Corrected. In Two Volumes.".to_owned(),
        place: "Philadelphia".to_owned(),
        publisher: "Presbyterian Board of Publication".to_owned(),
        date_printed: "1813".to_owned(),
        subjects,
        volumes,
        note: "Edition, place, publisher and date are read from the volume I title page as the \
               digitized text prints it; the date is the one printed there, which the record does not \
               confirm or date further. Volume II's title page prints no date."
            .to_owned(),
    }
}

/// Citations and note kinds; the pack's content.
fn assemble(
    parsed: &ParsedWork,
    source: &crate::rights::VerifiedSource,
    bibliography: WorkBibliography,
) -> Result<(WorkContent, CitationReport), ImportError> {
    let table = alias_table();
    let mut sections = parsed.sections.clone();
    let mut rows = Vec::new();
    let mut report = CitationReport {
        extra_aliases: EXTRA_ALIASES
            .iter()
            .map(|(b, s)| format!("{s} → {}", b.as_osis()))
            .collect(),
        ..CitationReport::default()
    };
    for section in &mut sections {
        for note in &mut section.notes {
            let found = extract(&note.text, &table);
            let mut residue: Vec<char> = note.text.chars().collect();
            for f in &found {
                for c in residue.iter_mut().take(f.end).skip(f.start) {
                    *c = ' ';
                }
            }
            let residue_has_words = residue.iter().any(|c| c.is_alphabetic());
            note.kind = match (found.is_empty(), residue_has_words) {
                (true, _) => NoteKind::Comment,
                (false, false) => NoteKind::Citations,
                (false, true) => NoteKind::Mixed,
            };
            *report
                .notes_by_kind
                .entry(format!("{:?}", note.kind).to_lowercase())
                .or_insert(0) += 1;
            for f in found {
                report.total += 1;
                let (status, target, class, detail, spans) = match f.outcome {
                    Outcome::Resolved { set, spans, detail } => {
                        report.resolved += 1;
                        if set.ranges.iter().any(|r| r.start.verse.is_none()) {
                            report.chapter_only += 1;
                        }
                        if detail.as_deref().is_some_and(|d| d.contains("&c.")) {
                            report.and_following += 1;
                        }
                        report.ranges += spans.len();
                        (
                            CitationStatus::Resolved,
                            Some(set.to_osis()),
                            None,
                            detail,
                            spans,
                        )
                    }
                    Outcome::Unresolved { class, detail } => {
                        *report.unresolved.entry(class.to_owned()).or_insert(0) += 1;
                        let samples = report.samples.entry(class.to_owned()).or_default();
                        if samples.len() < 10 {
                            samples.push((
                                section.locator.clone(),
                                note.id.clone(),
                                f.text.clone(),
                                detail.clone(),
                            ));
                        }
                        (
                            CitationStatus::Unresolved,
                            None,
                            Some(class.to_owned()),
                            Some(detail),
                            Vec::new(),
                        )
                    }
                };
                rows.push(CitationRow {
                    link: openlogos_retrieval_sqlite::work_pack::WorkCitationLink {
                        section: section.locator.clone(),
                        note: note.id.clone(),
                        text: f.text,
                        start: f.start as u32,
                        end: f.end as u32,
                        status,
                        target,
                        class,
                        detail,
                    },
                    spans,
                });
            }
        }
    }
    let unknown = report.unresolved.get("unknown_name").copied().unwrap_or(0);
    report.rate = ratio(report.resolved, report.total);
    report.rate_named_books = ratio(report.resolved, report.total - unknown);

    let mut hasher = Sha256::new();
    for section in &sections {
        hasher.update(serde_json::to_vec(section).map_err(|e| ImportError::Pack(e.to_string()))?);
    }
    hasher.update(
        serde_json::to_vec(&parsed.structure).map_err(|e| ImportError::Pack(e.to_string()))?,
    );
    for row in &rows {
        hasher.update(format!("{:?}", row.link).as_bytes());
    }
    let digest: String = hasher
        .finalize()
        .iter()
        .take(6)
        .map(|b| format!("{b:02x}"))
        .collect();
    let info = WorkInfo {
        resource_id: RESOURCE_ID.to_owned(),
        work: "institutes".to_owned(),
        locator_scheme: "institutes".to_owned(),
        content_revision: format!("allen-1813-{digest}"),
        title: "Institutes of the Christian Religion".to_owned(),
        short_title: "Institutes (Allen)".to_owned(),
        language: "en".to_owned(),
        source_id: source.source_id.clone(),
        license: source.license.clone(),
        attribution: source.attribution.clone(),
        source: format!(
            "rights {}: pg45001.txt and pg64392.txt, MANIFEST.sha256 {}",
            source.source_id, source.manifest_sha256
        ),
        bibliography,
        sections: sections.len() as u32,
        notes: sections.iter().map(|s| s.notes.len() as u32).sum(),
        citations: report.total as u32,
        resolved: report.resolved as u32,
    };
    Ok((
        WorkContent {
            info,
            structure: parsed.structure.clone(),
            sections,
            citations: rows,
            citation_method: CITATION_METHOD.to_owned(),
        },
        report,
    ))
}

fn ratio(a: usize, b: usize) -> f64 {
    if b == 0 { 0.0 } else { a as f64 / b as f64 }
}

/// What the benchmark measured.
#[derive(Debug, Clone, Serialize)]
pub struct InstitutesBench {
    /// Budgets: chapter 100 ms, search 50 ms.
    pub budgets_ms: (f64, f64),
    /// One row per measure.
    pub rows: Vec<Row>,
    /// Hits for a few queries, so the numbers are seen to come from real results.
    pub spot_hits: Vec<(String, usize)>,
}

/// The queries replayed keystroke by keystroke: words and phrases of the work's subjects, the
/// commonest English words, and names.
pub const BENCH_QUERIES: [&str; 22] = [
    "faith",
    "justification by faith",
    "the knowledge of god",
    "predestination",
    "election",
    "sacraments",
    "the lord's supper",
    "baptism of infants",
    "the holy spirit",
    "trinity",
    "three persons",
    "repentance",
    "providence",
    "the church",
    "civil government",
    "augustine",
    "the",
    "and of the",
    "christ",
    "free will",
    "transubstantiation",
    "melancthon",
];

fn row(group: &str, mut samples: Vec<(f64, String)>) -> Row {
    samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let values: Vec<f64> = samples.iter().map(|s| s.0).collect();
    Row {
        group: group.to_owned(),
        samples: values.len(),
        p50_ms: percentile(&values, 50.0),
        p95_ms: percentile(&values, 95.0),
        max_ms: values.last().copied().unwrap_or(0.0),
        slowest: samples.last().map_or_else(String::new, |s| s.1.clone()),
    }
}

fn ms(run: impl FnOnce()) -> f64 {
    let started = Instant::now();
    run();
    started.elapsed().as_secs_f64() * 1000.0
}

/// Opens every chapter, every section's citations, the reverse index and search, `rounds` times.
pub fn bench(pack: &Path, rounds: usize) -> Result<InstitutesBench, ImportError> {
    let err = |e: &dyn std::fmt::Display| ImportError::Pack(e.to_string());
    let opened = Instant::now();
    let work = WorkPack::open(pack).map_err(|e| err(&e))?;
    let open_ms = opened.elapsed().as_secs_f64() * 1000.0;
    let chapters: Vec<String> = work
        .structure()
        .iter()
        .flat_map(|d| d.chapters.iter().map(|c| c.locator.clone()))
        .collect();
    let mut rows = vec![row(
        "open the pack (cold)",
        vec![(open_ms, pack.display().to_string())],
    )];
    let mut chapter_samples = Vec::new();
    for _ in 0..rounds {
        for chapter in &chapters {
            let t = ms(|| {
                std::hint::black_box(work.chapter(chapter).ok());
            });
            chapter_samples.push((t, chapter.clone()));
        }
    }
    rows.push(row("chapter: every chapter-level unit", chapter_samples));
    let sections: Vec<String> = work
        .structure()
        .iter()
        .flat_map(|d| d.chapters.iter().flat_map(|c| c.sections.clone()))
        .collect();
    let mut section_samples = Vec::new();
    let mut citation_samples = Vec::new();
    for _ in 0..rounds {
        for section in &sections {
            section_samples.push((
                ms(|| {
                    std::hint::black_box(work.section(section).ok());
                }),
                section.clone(),
            ));
            citation_samples.push((
                ms(|| {
                    std::hint::black_box(work.citations(section).ok());
                }),
                section.clone(),
            ));
        }
    }
    rows.push(row("section: every section", section_samples));
    rows.push(row(
        "citations of a section: every section",
        citation_samples,
    ));
    // The reverse index: every chapter of a few books often cited, verse by verse.
    let mut reverse = Vec::new();
    let passages: Vec<String> = ["Rom", "Ps", "John", "Gen", "Heb"]
        .iter()
        .flat_map(|book| (1..=8).map(move |c| format!("{book}.{c}")))
        .collect();
    for _ in 0..rounds {
        for chapter in &passages {
            for verse in 1..=10 {
                let passage = format!("{chapter}.{verse}");
                reverse.push((
                    ms(|| {
                        std::hint::black_box(work.cited_by(&passage, None).ok());
                    }),
                    passage,
                ));
            }
        }
    }
    rows.push(row("sections citing a verse (reverse index)", reverse));
    let mut search = EditionSearch::new().map_err(|e| err(&e))?;
    search.attach_file(pack).map_err(|e| err(&e))?;
    let provider: &dyn TextProvider = &search;
    let queries: Vec<String> = BENCH_QUERIES.iter().map(|q| (*q).to_owned()).collect();
    let strokes = keystrokes(&queries);
    for (name, limit) in [
        ("search, palette section (5)", DEFAULT_SECTION_LIMIT),
        ("search, expanded section (50)", EXPANDED_SECTION_LIMIT),
    ] {
        let mut samples = Vec::new();
        for _ in 0..rounds {
            for stroke in &strokes {
                samples.push((
                    ms(|| {
                        std::hint::black_box(provider.search(stroke, limit).len());
                    }),
                    stroke.clone(),
                ));
            }
        }
        rows.push(row(name, samples));
    }
    let spot_hits = [
        "predestination",
        "transubstantiation",
        "faith",
        "melancthon",
    ]
    .iter()
    .map(|q| ((*q).to_owned(), provider.search(q, 50).len()))
    .collect();
    Ok(InstitutesBench {
        budgets_ms: (100.0, 50.0),
        rows,
        spot_hits,
    })
}

/// The by-eye spread: each place's text and notes, with its citations, for reading against the
/// source. Printed, never written into the repository.
pub fn show(pack: &Path, places: &[String]) -> Result<String, ImportError> {
    let err = |e: &dyn std::fmt::Display| ImportError::Pack(e.to_string());
    let work = WorkPack::open(pack).map_err(|e| err(&e))?;
    let mut out = String::new();
    for place in places {
        let section = work.section(place).map_err(|e| err(&e))?;
        out.push_str(&format!(
            "=== {} (#{}, chapter {}, label {:?}, {:?})\n",
            section.locator, section.ordinal, section.chapter, section.label, section.citation
        ));
        for p in &section.paragraphs {
            out.push_str(&format!("[{:?}] {}\n", p.style, p.text));
        }
        for n in &section.notes {
            out.push_str(&format!(
                "  note {} {} at p{}:{} ({:?}): {}\n",
                n.id, n.marker, n.paragraph, n.scalar, n.kind, n.text
            ));
        }
        for c in work.citations(&section.locator).map_err(|e| err(&e))? {
            out.push_str(&format!(
                "  cite {} {:?} -> {:?} {:?} {:?}\n",
                c.text, c.status, c.target, c.class, c.detail
            ));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_record_table_is_read_row_by_row() {
        let html = r#"<html><table class="bibrec"><tr><th>Author</th><td><a href="x">Calvin, Jean, 1509-1564</a></td></tr>
<tr><th>Release Date</th><td>Feb 24, 2014</td></tr><tr><th>Subject</th><td>A &amp; B</td></tr></table></html>"#;
        let rows = record_rows(html);
        assert_eq!(
            rows[0],
            ("Author".to_owned(), "Calvin, Jean, 1509-1564".to_owned())
        );
        assert_eq!(first(&rows, "Release Date"), "Feb 24, 2014");
        assert_eq!(first(&rows, "Subject"), "A & B");
        assert!(record_rows("<html></html>").is_empty());
    }

    #[test]
    fn the_output_may_not_be_in_the_repository() {
        assert!(refuse_inside(Path::new("/a/b/c"), Path::new("/a/b"), "x").is_err());
        assert!(refuse_inside(Path::new("/a/c"), Path::new("/a/b"), "x").is_ok());
    }
}
