//! B067: Clear's manual BSB alignments (S55) as alignment layers over the preview pack's
//! BSB, SBLGNT and WLC tokens (docs/evidence/B067.md, docs/alignments.md).
//!
//! `just preview-pack-alignment` runs [`build`]: S55 through the B071 rights gate (status,
//! permissions, every hash); of its files only the two BSB alignment files, the two BSB
//! target tables and the two source tables they reference are parsed (YLT, BGNT and the
//! catalog are hashed by the gate and never read); Clear's English words joined to the BSB
//! pack tokens ([`target`]); Clear's source ids joined to the SBLGNT and WLC tokens and
//! morphemes through the B026 morphology layers, which keep MACULA's ids ([`source`]); one
//! layer per testament written with the retrieval engine's
//! [`openlogos_retrieval_sqlite::alignment::AlignmentWriter`]. The output is outside the
//! repository, beside the preview pack and the morphology layers:
//!
//! ```text
//! ~/.cache/openlogos/alignments/
//!   alignments.json                              counts, classes, record shapes, sizes, timings
//!   joins/clear-sblgnt-bsb.json, clear-wlc-bsb.json   every place of every class but the plain ones
//!   layers/sblgnt-bsb.clear.sqlite, wlc-bsb.clear.sqlite
//! ```
//!
//! Every record keeps Clear's own metadata (`origin: manual`, `status: created`) and its id.

pub mod bench;
pub mod clear;
pub mod source;
pub mod target;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use openlogos_retrieval::VerseId;
use openlogos_retrieval_sqlite::alignment::{
    AlignmentInfo, AlignmentWriter, RecordRow, Side, SourceWordRow, TargetWordRow,
    tokenization_hash,
};
use openlogos_text::reading::pack::{PackManifest, sha256_hex};
use openlogos_text::reading::{DirSource, FilePack, ReadingLibrary};
use serde::Serialize;

use self::clear::{ClearAlignment, SourceToken, TargetToken};
use self::source::{LayerIndex, SourceReport};
use self::target::{PackText, PackVerse, TargetReport};
use crate::model::ImportError;
use crate::rights::{VerifiedSource, verify_source};

pub use openlogos_retrieval_sqlite::alignment::osis;

/// The CC BY 4.0 deed.
const CC_BY: &str = "https://creativecommons.org/licenses/by/4.0/";

/// The report at the top of the output.
pub const REPORT_FILE: &str = "alignments.json";

/// The source's directory under the corpora.
pub const SOURCE_DIR: &str = "S55-clear-alignments";

/// One testament's files and editions.
struct Pair {
    alignment_id: &'static str,
    title: &'static str,
    file: &'static str,
    alignment: &'static str,
    clear_source: &'static str,
    targets: &'static str,
    sources: &'static str,
    edition: &'static str,
    morphology: &'static str,
    nt: bool,
}

const PAIRS: [Pair; 2] = [
    Pair {
        alignment_id: "clear-sblgnt-bsb",
        title: "Clear SBLGNT–BSB alignment (manual)",
        file: "sblgnt-bsb.clear.sqlite",
        alignment: "data/eng/alignments/BSB/SBLGNT-BSB-manual.json",
        clear_source: "SBLGNT",
        targets: "data/eng/targets/BSB/nt_BSB.tsv",
        sources: "data/sources/SBLGNT+required.tsv",
        edition: "sblgnt",
        morphology: "sblgnt.macula-greek.sqlite",
        nt: true,
    },
    Pair {
        alignment_id: "clear-wlc-bsb",
        title: "Clear WLC–BSB alignment (manual)",
        file: "wlc-bsb.clear.sqlite",
        alignment: "data/eng/alignments/BSB/WLCM-BSB-manual.json",
        clear_source: "WLCM",
        targets: "data/eng/targets/BSB/ot_BSB.tsv",
        sources: "data/sources/WLCM+required.tsv",
        edition: "wlc",
        morphology: "wlc.macula-hebrew.sqlite",
        nt: false,
    },
];

/// The only files of S55 this importer parses; everything else is hashed by the gate only.
#[must_use]
pub fn parsed_files() -> Vec<&'static str> {
    PAIRS
        .iter()
        .flat_map(|p| [p.alignment, p.targets, p.sources])
        .collect()
}

/// What a build is told.
#[derive(Debug, Clone)]
pub struct AlignmentOptions {
    /// The directory holding `S55-clear-alignments/`; only read.
    pub corpora: PathBuf,
    /// The repository's `rights/`.
    pub rights: PathBuf,
    /// The built preview pack; only read.
    pub preview_pack: PathBuf,
    /// The built morphology layers; only read.
    pub morphology: PathBuf,
    /// The output directory.
    pub out: PathBuf,
}

/// Record shapes and metadata.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RecordReport {
    /// Records in the file.
    pub records: u64,
    /// Records by `<source words>:<target words>`, capped at 3 (`3` means 3 or more).
    pub shapes: BTreeMap<String, u64>,
    /// Records whose target words are not one run of Clear's words.
    pub discontinuous_target: u64,
    /// Records whose source words are not one run of Clear's source tokens.
    pub discontinuous_source: u64,
    /// Records whose words come from more than one verse (either side).
    pub cross_verse: u64,
    /// Records with a word on either side that has no pack token.
    pub with_word_not_joined: u64,
    /// Records none of whose target words has a pack token.
    pub without_any_target_token: u64,
    /// Records none of whose source words has a pack token.
    pub without_any_source_token: u64,
    /// Source words in more than one record.
    pub source_words_in_several_records: u64,
    /// Target words in more than one record.
    pub target_words_in_several_records: u64,
    /// Clear source tokens in no record (Clear left them unaligned).
    pub source_words_unaligned: u64,
    /// Clear target words in no record.
    pub target_words_unaligned: u64,
    /// The file-level metadata.
    pub file_meta: BTreeMap<String, String>,
    /// Per-record metadata values by key (`origin` → `manual` → count).
    pub meta_values: BTreeMap<String, BTreeMap<String, u64>>,
}

/// One layer's line in the report.
#[derive(Debug, Clone, Serialize)]
pub struct LayerReport {
    /// The layer's manifest.
    pub layer: AlignmentInfo,
    /// The file, under `layers/`.
    pub file: String,
    /// Its size.
    pub bytes: u64,
    /// Seconds to parse, join and write it (after the gate).
    pub seconds: f64,
    /// The records.
    pub records: RecordReport,
    /// The English side's join.
    pub target: TargetReport,
    /// The original-language side's join.
    pub source: SourceReport,
}

/// `alignments.json`.
#[derive(Debug, Clone, Serialize)]
pub struct AlignmentReport {
    /// The command that makes this.
    pub built_by: String,
    /// The rights record and what the gate checked.
    pub source_id: String,
    /// SHA-256 of the source's `MANIFEST.sha256`.
    pub source_manifest_sha256: String,
    /// Files whose hash was checked.
    pub source_files_checked: usize,
    /// Files parsed.
    pub source_files_parsed: Vec<String>,
    /// Seconds in the rights gate.
    pub verify_seconds: f64,
    /// Seconds for the whole build.
    pub total_seconds: f64,
    /// Per layer.
    pub layers: Vec<LayerReport>,
}

fn io(path: &Path, error: std::io::Error) -> ImportError {
    ImportError::Io {
        path: path.display().to_string(),
        error,
    }
}

fn absolute(path: &Path) -> Result<PathBuf, ImportError> {
    std::path::absolute(path).map_err(|e| io(path, e))
}

fn refuse_inside(out: &Path, parent: &Path, what: &str) -> Result<(), ImportError> {
    if out.starts_with(parent) {
        return Err(ImportError::Usage(format!(
            "the output {} is inside {what}; an alignment layer never goes there",
            out.display()
        )));
    }
    Ok(())
}

/// A text pack's identity, verse texts and token ranges.
pub struct TextPack {
    /// Its manifest.
    pub manifest: PackManifest,
    /// SHA-256 of `manifest.json`.
    pub manifest_sha256: String,
    /// Verses with text and tokens.
    pub verses: PackText,
}

impl TextPack {
    /// `(id, surface)` of every token in id order, for the tokenization hash.
    #[must_use]
    pub fn token_lines(&self) -> Vec<(String, String)> {
        let edition = &self.manifest.resource_id;
        let mut lines = Vec::new();
        for (verse, pv) in &self.verses {
            let chars: Vec<char> = pv.text.chars().collect();
            for (word, start, end) in &pv.tokens {
                lines.push((
                    format!(
                        "{edition}:{:02}{:03}{:03}{word:03}",
                        verse.book(),
                        verse.chapter(),
                        verse.verse()
                    ),
                    chars[*start as usize..*end as usize].iter().collect(),
                ));
            }
        }
        lines
    }

    /// The tokenization hash of specs/03.
    #[must_use]
    pub fn tokenization_hash(&self) -> String {
        let lines = self.token_lines();
        tokenization_hash(lines.iter().map(|(a, b)| (a.as_str(), b.as_str())))
    }
}

/// Reads every chapter of a built text pack through the reader (which checks each chunk's
/// hash): verse text and token ranges.
pub fn read_text_pack(directory: &Path) -> Result<TextPack, ImportError> {
    let pack_error = |e: &dyn std::fmt::Display| ImportError::Pack(e.to_string());
    let path = directory.join("manifest.json");
    let json = std::fs::read_to_string(&path).map_err(|e| io(&path, e))?;
    let manifest: PackManifest = serde_json::from_str(&json).map_err(|e| pack_error(&e))?;
    let pack = FilePack::from_manifest(&json, Some(Box::new(DirSource::new(directory))))
        .map_err(|e| pack_error(&e))?;
    let mut library = ReadingLibrary::new();
    library.insert(Box::new(pack));
    let mut verses = PackText::new();
    for book in &manifest.books {
        for chapter in &book.chapters {
            let key = format!("{}.{}", book.book, chapter.chapter);
            let block = library
                .chapter_content(&manifest.resource_id, &key)
                .map_err(|e| pack_error(&e))?;
            for verse in &block.verses {
                let id = VerseId::new(
                    book.book.number(),
                    u16::try_from(chapter.chapter).unwrap_or(0),
                    u16::try_from(verse.number).unwrap_or(0),
                );
                verses.insert(
                    id,
                    PackVerse {
                        text: verse.text.clone(),
                        tokens: verse
                            .tokens
                            .iter()
                            .enumerate()
                            .map(|(i, t)| (i as u32 + 1, t.start, t.end))
                            .collect(),
                    },
                );
            }
        }
    }
    Ok(TextPack {
        manifest_sha256: sha256_hex(json.as_bytes()),
        manifest,
        verses,
    })
}

fn utf8<'a>(source: &'a VerifiedSource, path: &str) -> Result<&'a str, ImportError> {
    std::str::from_utf8(bytes(source, path)?)
        .map_err(|_| crate::model::parse_error(path, "not UTF-8"))
}

fn bytes<'a>(source: &'a VerifiedSource, path: &str) -> Result<&'a [u8], ImportError> {
    source
        .files
        .get(path)
        .map(Vec::as_slice)
        .ok_or_else(|| ImportError::Rights {
            source_id: source.source_id.clone(),
            detail: format!("{path} is not in the manifest"),
        })
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), ImportError> {
    let text = serde_json::to_string_pretty(value).map_err(|e| ImportError::Pack(e.to_string()))?;
    std::fs::write(path, text + "\n").map_err(|e| io(path, e))
}

fn shape(n: usize) -> String {
    if n >= 3 {
        "3".to_owned()
    } else {
        n.to_string()
    }
}

/// Everything one testament's layer is made from.
pub struct PairInput<'a> {
    /// The layer's manifest.
    pub info: AlignmentInfo,
    /// The original-language pack.
    pub source_pack: &'a TextPack,
    /// The BSB verses of this testament.
    pub target_pack: &'a PackText,
    /// The morphology layer over the source pack.
    pub layer: &'a LayerIndex,
    /// Clear's target table.
    pub targets: &'a [TargetToken],
    /// Clear's source table.
    pub sources: &'a [SourceToken],
    /// Clear's alignment file.
    pub alignment: &'a ClearAlignment,
}

/// What [`join_pair`] makes.
pub struct PairOutput {
    /// The layer's counts.
    pub records: RecordReport,
    /// English join.
    pub target: TargetReport,
    /// Original-language join.
    pub source: SourceReport,
}

/// Joins one testament and writes its layer to `path`.
#[allow(clippy::too_many_lines)]
pub fn join_pair(input: &PairInput<'_>, path: &Path) -> Result<PairOutput, ImportError> {
    let pack = |e: openlogos_retrieval::RetrievalError| ImportError::Pack(e.to_string());
    let edition = input.info.source_edition.as_str();
    let target_join = target::join(input.target_pack, input.targets);
    let pack_tokens: Vec<(VerseId, u32)> = input
        .source_pack
        .verses
        .iter()
        .flat_map(|(v, pv)| pv.tokens.iter().map(move |t| (*v, t.0)))
        .collect();
    let source_join = source::join(edition, input.layer, input.sources, &pack_tokens);

    let mut writer = AlignmentWriter::create(path, &input.info).map_err(pack)?;
    // Target words: one per Clear word, joined or not.
    let mut target_handle: HashMap<&str, u32> = HashMap::new();
    let mut target_order: HashMap<&str, usize> = HashMap::new();
    for (order, w) in target_join.words.iter().enumerate() {
        let t = &input.targets[w.clear];
        let clear_verse = VerseId::new(t.book.number(), t.chapter, t.verse);
        let (verse, word, span) = match w.place {
            Some((v, word, s, e)) => (v, word, Some((s, e))),
            None => (clear_verse, 0, None),
        };
        let handle = writer.add_target(TargetWordRow {
            verse,
            word,
            span,
            external_id: t.id.clone(),
            text: t.text.clone(),
            join: w.class.to_owned(),
        });
        target_handle.insert(t.id.as_str(), handle);
        target_order.insert(t.id.as_str(), order);
    }
    let excluded: std::collections::HashSet<&str> = input
        .targets
        .iter()
        .filter(|t| t.exclude)
        .map(|t| t.id.as_str())
        .collect();
    // Source words: one per Clear source token that is in the layer.
    let mut source_handle: HashMap<&str, u32> = HashMap::new();
    let mut source_order: HashMap<&str, usize> = HashMap::new();
    let mut source_joined: HashMap<&str, bool> = HashMap::new();
    for (order, w) in source_join.words.iter().enumerate() {
        let s = &input.sources[w.clear];
        source_order.insert(s.id.as_str(), order);
        let Some(m) = w.morpheme else {
            continue;
        };
        let lm = &input.layer.morphemes[m];
        let handle = writer.add_source(SourceWordRow {
            pos: lm.pos,
            verse: lm.verse,
            word: lm.word,
            morpheme: lm.morpheme,
            morphemes: lm.morphemes,
            external_id: s.id.clone(),
            text: s.text.clone(),
            join: w.class.to_owned(),
        });
        source_handle.insert(s.id.as_str(), handle);
        source_joined.insert(s.id.as_str(), lm.word != 0);
    }

    let mut report = RecordReport {
        records: input.alignment.records.len() as u64,
        file_meta: input.alignment.meta.clone(),
        ..RecordReport::default()
    };
    let mut source_uses: HashMap<&str, u32> = HashMap::new();
    let mut target_uses: HashMap<&str, u32> = HashMap::new();
    let mut ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for record in &input.alignment.records {
        let mut meta: BTreeMap<String, String> = record
            .meta
            .iter()
            .map(|(k, v)| (k.clone(), clear::value_text(v)))
            .collect();
        let id = meta.remove("id").ok_or_else(|| {
            ImportError::Pack(format!(
                "{}: a record has no meta id",
                input.info.alignment_id
            ))
        })?;
        if !ids.insert(id.clone()) {
            return Err(ImportError::Pack(format!("record id {id} is not unique")));
        }
        for (k, v) in &meta {
            *report
                .meta_values
                .entry(k.clone())
                .or_default()
                .entry(v.clone())
                .or_default() += 1;
        }
        *report
            .shapes
            .entry(format!(
                "{}:{}",
                shape(record.source.len()),
                shape(record.target.len())
            ))
            .or_default() += 1;
        let mut sources = Vec::new();
        let mut not_joined = false;
        let mut any_source_token = false;
        let mut verses: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for s in &record.source {
            source_order.get(s.as_str()).ok_or_else(|| {
                ImportError::Pack(format!(
                    "record {id} names source {s}, not in {}",
                    input.info.source_edition
                ))
            })?;
            *source_uses.entry(s.as_str()).or_default() += 1;
            verses.insert(s.chars().skip(1).take(8).collect());
            match source_handle.get(s.as_str()) {
                Some(h) => {
                    sources.push(*h);
                    if source_joined[s.as_str()] {
                        any_source_token = true;
                    } else {
                        not_joined = true;
                    }
                }
                // Not in the layer or letters differ: listed by the source join; the record
                // keeps its other words.
                None => not_joined = true,
            }
        }
        let mut targets = Vec::new();
        let mut any_target_token = false;
        for t in &record.target {
            if excluded.contains(t.as_str()) {
                return Err(ImportError::Pack(format!(
                    "record {id} aligns {t}, a token Clear excludes (punctuation)"
                )));
            }
            let h = target_handle.get(t.as_str()).ok_or_else(|| {
                ImportError::Pack(format!(
                    "record {id} names target {t}, not in the BSB table"
                ))
            })?;
            *target_uses.entry(t.as_str()).or_default() += 1;
            verses.insert(t.chars().take(8).collect());
            targets.push(*h);
            let joined = target_join.words[target_order[t.as_str()]].place.is_some();
            any_target_token |= joined;
            not_joined |= !joined;
        }
        let run = |orders: Vec<usize>| {
            let mut orders = orders;
            orders.sort_unstable();
            orders.windows(2).any(|w| w[1] != w[0] + 1)
        };
        report.discontinuous_target += u64::from(run(record
            .target
            .iter()
            .map(|t| target_order[t.as_str()])
            .collect()));
        report.discontinuous_source += u64::from(run(record
            .source
            .iter()
            .filter_map(|s| source_order.get(s.as_str()).copied())
            .collect()));
        report.cross_verse += u64::from(verses.len() > 1);
        report.with_word_not_joined += u64::from(not_joined);
        report.without_any_target_token += u64::from(!any_target_token);
        report.without_any_source_token += u64::from(!any_source_token);
        if sources.is_empty() && targets.is_empty() {
            continue;
        }
        writer
            .add_record(RecordRow {
                external_id: id,
                sources,
                targets,
                meta,
            })
            .map_err(pack)?;
    }
    report.source_words_in_several_records =
        source_uses.values().filter(|n| **n > 1).count() as u64;
    report.target_words_in_several_records =
        target_uses.values().filter(|n| **n > 1).count() as u64;
    report.source_words_unaligned = input
        .sources
        .iter()
        .filter(|s| !source_uses.contains_key(s.id.as_str()))
        .count() as u64;
    report.target_words_unaligned = target_join
        .words
        .iter()
        .filter(|w| !target_uses.contains_key(input.targets[w.clear].id.as_str()))
        .count() as u64;

    for (verse, word, reason) in &target_join.absent {
        writer.absent(Side::Target, *verse, *word, None, reason);
    }
    for m in &source_join.not_in_clear {
        let lm = &input.layer.morphemes[*m];
        writer.absent(
            Side::Source,
            lm.verse,
            lm.word,
            Some(lm.pos),
            "not_in_alignment_source",
        );
    }
    for ((verse, word), reason) in &input.layer.absent {
        writer.absent(Side::Source, *verse, *word, None, reason);
    }
    writer.finish().map_err(pack)?;
    Ok(PairOutput {
        records: report,
        target: target_join.report,
        source: source_join.report,
    })
}

/// The whole build. The repository, the corpora, the preview pack and the morphology
/// layers are refused as outputs.
#[allow(clippy::too_many_lines)]
pub fn build(
    options: &AlignmentOptions,
    repository: &Path,
) -> Result<AlignmentReport, ImportError> {
    let started = Instant::now();
    let out = absolute(&options.out)?;
    refuse_inside(&out, &absolute(repository)?, "the repository")?;
    refuse_inside(&out, &absolute(&options.corpora)?, "the corpora")?;
    refuse_inside(&out, &absolute(&options.preview_pack)?, "the preview pack")?;
    refuse_inside(
        &out,
        &absolute(&options.morphology)?,
        "the morphology layers",
    )?;
    let work = out.with_extension("building");
    if work.exists() {
        std::fs::remove_dir_all(&work).map_err(|e| io(&work, e))?;
    }
    for dir in ["layers", "joins"] {
        std::fs::create_dir_all(work.join(dir)).map_err(|e| io(&work, e))?;
    }

    let packs = options.preview_pack.join("packs");
    let bsb = read_text_pack(&packs.join("bsb"))?;
    let bsb_hash = bsb.tokenization_hash();

    let gate = Instant::now();
    let parsed = parsed_files();
    let s55 = verify_source(
        "S55",
        &options.rights,
        &options.corpora.join(SOURCE_DIR),
        |p| parsed.contains(&p),
    )?;
    let verify_seconds = gate.elapsed().as_secs_f64();
    let commit: String = s55
        .commit
        .as_deref()
        .map_or_else(String::new, |c| c.chars().take(7).collect());

    let mut layers = Vec::new();
    for pair in &PAIRS {
        let at = Instant::now();
        let source_pack = read_text_pack(&packs.join(pair.edition))?;
        let layer_path = options.morphology.join("layers").join(pair.morphology);
        let layer = source::read_layer(&layer_path)?;
        if layer.text_manifest_sha256 != source_pack.manifest_sha256 {
            return Err(ImportError::Pack(format!(
                "{} was joined to another {} text pack ({}, the pack's manifest is {}); rebuild the morphology layers",
                layer_path.display(),
                pair.edition,
                layer.text_manifest_sha256,
                source_pack.manifest_sha256
            )));
        }
        let target_pack: PackText = bsb
            .verses
            .iter()
            .filter(|(v, _)| (v.book() >= 41) == pair.nt)
            .map(|(v, pv)| (*v, pv.clone()))
            .collect();
        let targets = clear::read_targets(pair.targets, utf8(&s55, pair.targets)?)?;
        let sources = clear::read_sources(pair.sources, utf8(&s55, pair.sources)?)?;
        let alignment = clear::read_alignment(
            pair.alignment,
            bytes(&s55, pair.alignment)?,
            pair.clear_source,
            "BSB",
        )?;
        let info = AlignmentInfo {
            alignment_id: pair.alignment_id.to_owned(),
            title: pair.title.to_owned(),
            source_edition: source_pack.manifest.resource_id.clone(),
            target_edition: bsb.manifest.resource_id.clone(),
            revision: format!(
                "{}-{commit}-on-{}-{}",
                pair.alignment_id,
                source_pack.manifest.content_revision,
                bsb.manifest.content_revision
            ),
            license: format!("{}, {CC_BY}", s55.license),
            attribution: format!(
                "{} Licence: {CC_BY}. Source: {}, {} at commit {}. Changed for this layer: \
                 Clear's BSB words are joined to the tokens of the {} text pack ({}), and \
                 Clear's {} tokens, by their MACULA ids, to the tokens of the {} text pack ({}); \
                 words that could not be joined are kept without a token; each record keeps \
                 Clear's metadata. The licensor offers the work as-is and makes no warranties \
                 (CC BY 4.0 §5).",
                s55.attribution,
                s55.upstream_url,
                pair.alignment,
                s55.commit.as_deref().unwrap_or("unknown"),
                bsb.manifest.title,
                bsb.manifest.content_revision,
                pair.clear_source,
                source_pack.manifest.title,
                source_pack.manifest.content_revision,
            ),
            source: format!(
                "{} ({}), {}, MANIFEST.sha256 {}",
                s55.upstream_url, s55.edition_id, pair.alignment, s55.manifest_sha256
            ),
            origin: format!("curated:clear-alignments@{commit}"),
            source_text_content_revision: source_pack.manifest.content_revision.clone(),
            source_text_manifest_sha256: source_pack.manifest_sha256.clone(),
            source_tokenization_hash: source_pack.tokenization_hash(),
            source_morphology_layer: layer.layer_id.clone(),
            source_morphology_revision: layer.revision.clone(),
            target_text_content_revision: bsb.manifest.content_revision.clone(),
            target_text_manifest_sha256: bsb.manifest_sha256.clone(),
            target_tokenization_hash: bsb_hash.clone(),
        };
        let path = work.join("layers").join(pair.file);
        let output = join_pair(
            &PairInput {
                info: info.clone(),
                source_pack: &source_pack,
                target_pack: &target_pack,
                layer: &layer,
                targets: &targets,
                sources: &sources,
                alignment: &alignment,
            },
            &path,
        )?;
        let PairOutput {
            records,
            mut target,
            mut source,
        } = output;
        #[derive(Serialize)]
        struct Places<'a> {
            alignment_id: &'a str,
            target: &'a BTreeMap<String, Vec<target::TargetPlace>>,
            source: &'a BTreeMap<String, Vec<source::SourcePlace>>,
        }
        write_json(
            &work
                .join("joins")
                .join(format!("{}.json", pair.alignment_id)),
            &Places {
                alignment_id: pair.alignment_id,
                target: &target.places,
                source: &source.places,
            },
        )?;
        target.places.clear();
        source.places.clear();
        layers.push(LayerReport {
            layer: info,
            file: pair.file.to_owned(),
            bytes: std::fs::metadata(&path).map_err(|e| io(&path, e))?.len(),
            seconds: at.elapsed().as_secs_f64(),
            records,
            target,
            source,
        });
    }

    let report = AlignmentReport {
        built_by: "just preview-pack-alignment".to_owned(),
        source_id: s55.source_id.clone(),
        source_manifest_sha256: s55.manifest_sha256.clone(),
        source_files_checked: s55.files_checked,
        source_files_parsed: parsed.iter().map(|p| (*p).to_owned()).collect(),
        verify_seconds,
        total_seconds: started.elapsed().as_secs_f64(),
        layers,
    };
    write_json(&work.join(REPORT_FILE), &report)?;
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|e| io(&out, e))?;
    }
    std::fs::rename(&work, &out).map_err(|e| io(&out, e))?;
    Ok(report)
}
