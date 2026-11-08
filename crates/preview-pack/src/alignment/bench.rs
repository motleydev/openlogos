//! Latency of the alignment layers natively: a token lookup (BSB word to Greek or Hebrew, and
//! back) against one frame (16 ms), a lemma's rendering table against 50 ms (specs/22 §7).
//! Each group has at least 200 samples, chosen deterministically (evenly spaced tokens;
//! lemmas spread over the frequency ranks, the most frequent included), so runs compare.

use std::path::Path;
use std::time::Instant;

use openlogos_retrieval_sqlite::{AlignmentLayers, MorphologyLayers};
use serde::{Deserialize, Serialize};

use crate::model::ImportError;

/// The token lookup budget: the next frame.
pub const LOOKUP_BUDGET_MS: f64 = 16.0;

/// The rendering table budget.
pub const RENDERING_BUDGET_MS: f64 = 50.0;

/// The file the samples are written to, beside `layers/`.
pub const SAMPLES_FILE: &str = "bench-samples.json";

/// One group's timings.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// What was measured.
    pub group: String,
    /// Samples.
    pub samples: usize,
    /// Median.
    pub p50_ms: f64,
    /// 95th percentile.
    pub p95_ms: f64,
    /// Slowest.
    pub max_ms: f64,
    /// The slowest sample's input.
    pub slowest: String,
    /// The group's budget.
    pub budget_ms: f64,
}

/// The inputs, replayed by the wasm bench.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Samples {
    /// BSB token ids.
    pub target_tokens: Vec<String>,
    /// SBLGNT and WLC token ids.
    pub source_tokens: Vec<String>,
    /// `(source edition, lemma as spelled)`.
    pub lemmas: Vec<(String, String)>,
}

/// The bench's answer.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Seconds to attach and load the morphology layers.
    pub morphology_load_seconds: f64,
    /// Seconds to attach and load the alignment layers.
    pub alignment_load_seconds: f64,
    /// Per group.
    pub rows: Vec<Row>,
    /// Spot rendering tables: query → table lines.
    pub spot: Vec<(String, Vec<String>)>,
    /// The inputs.
    #[serde(skip)]
    pub samples: Samples,
}

fn row(group: &str, budget_ms: f64, mut samples: Vec<(f64, String)>) -> Row {
    samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let at = |q: f64| samples[((samples.len() as f64 - 1.0) * q).round() as usize].0;
    Row {
        group: group.to_owned(),
        samples: samples.len(),
        p50_ms: at(0.5),
        p95_ms: at(0.95),
        max_ms: samples.last().map_or(0.0, |s| s.0),
        slowest: samples.last().map(|s| s.1.clone()).unwrap_or_default(),
        budget_ms,
    }
}

fn time<T>(f: impl FnOnce() -> T) -> (f64, T) {
    let start = Instant::now();
    let out = f();
    (start.elapsed().as_secs_f64() * 1000.0, out)
}

/// Lines of a rendering table, for reading by eye.
#[must_use]
pub fn table_lines(
    r: &openlogos_retrieval_sqlite::alignment::Renderings,
    rows: usize,
) -> Vec<String> {
    let mut lines = vec![format!(
        "matched {}; unavailable {:?}",
        r.matched_by, r.unavailable
    )];
    for t in &r.tables {
        lines.push(format!(
            "{} {} in {} → {}: {} occurrences, {} aligned, {} unaligned, {} not covered",
            t.lemma,
            t.strong,
            t.scope,
            t.target_edition,
            t.occurrences,
            t.aligned,
            t.unaligned,
            t.not_covered
        ));
        for row in t.rows.iter().take(rows) {
            lines.push(format!(
                "  {:>5}  {}{} {:?}",
                row.count,
                if row.rendering.is_empty() {
                    "(unaligned)"
                } else {
                    row.rendering.as_str()
                },
                if row.with_other_source_words > 0 {
                    format!(" [{} with other source words]", row.with_other_source_words)
                } else {
                    String::new()
                },
                row.examples
            ));
        }
    }
    lines
}

/// Runs the bench over `layers/` of the morphology and alignment outputs.
pub fn run(
    morphology_dir: &Path,
    alignment_dir: &Path,
    rounds: usize,
) -> Result<Report, ImportError> {
    let err = |e: openlogos_retrieval::RetrievalError| ImportError::Pack(e.to_string());
    let (morph_ms, morphology) = time(|| MorphologyLayers::open_directory(morphology_dir));
    let morphology = morphology.map_err(err)?;
    let (align_ms, alignments) = time(|| AlignmentLayers::open_directory(alignment_dir));
    let alignments = alignments.map_err(err)?;
    let mismatches = alignments.morphology_mismatches(&morphology);
    if !mismatches.is_empty() {
        return Err(ImportError::Pack(format!("{mismatches:?}")));
    }
    let mut samples = Samples::default();
    for info in alignments.layers() {
        let edition = info.source_edition.clone();
        let (_, _, columns) = morphology
            .columns(&edition)
            .ok_or_else(|| ImportError::Pack(format!("no morphology layer over {edition}")))?;
        let n = columns.len();
        // Source tokens: 250 evenly spaced positions' tokens.
        for pos in (0..250).map(|i| i * n / 250) {
            let Some((verse, word)) = morphology.place_of(&edition, pos as u32) else {
                continue;
            };
            if word != 0 {
                samples.source_tokens.push(format!(
                    "{edition}:{:02}{:03}{:03}{word:03}",
                    verse.book(),
                    verse.chapter(),
                    verse.verse()
                ));
            }
        }
        // Target tokens: the BSB tokens of 250 evenly spaced alignment records' verses: the
        // records' first target word, read from a lookup of the sampled source token.
        // Lemmas: 250 spread over the frequency ranks.
        let lemma = columns.feature("lemma").expect("a layer has lemmas");
        let mut counts = vec![0u64; lemma.values.len()];
        for id in &lemma.ids {
            counts[*id as usize] += 1;
        }
        let mut ranked: Vec<usize> = (1..lemma.values.len()).filter(|i| counts[*i] > 0).collect();
        ranked.sort_by(|a, b| counts[*b].cmp(&counts[*a]).then(a.cmp(b)));
        for i in 0..250 {
            let id = ranked[i * ranked.len() / 250];
            samples
                .lemmas
                .push((edition.clone(), lemma.values[id].clone()));
        }
    }
    // BSB tokens: those the sampled source tokens align to (first of each), then evenly
    // spaced tokens of a fixed set of verses, until there are at least 250.
    for token in samples.source_tokens.clone() {
        let answer = alignments
            .align(std::slice::from_ref(&token), Some(&morphology))
            .map_err(err)?;
        if let Some(t) = answer
            .groups
            .iter()
            .flat_map(|g| g.targets.iter())
            .find_map(|t| t.token_id.clone())
        {
            samples.target_tokens.push(t);
        }
    }
    let mut lookups_target = Vec::new();
    let mut lookups_source = Vec::new();
    let mut tables = Vec::new();
    for round in 0..rounds {
        let tag = if round == 0 { "cold" } else { "warm" };
        for t in &samples.target_tokens {
            let (ms, r) = time(|| alignments.align(std::slice::from_ref(t), Some(&morphology)));
            r.map_err(err)?;
            lookups_target.push((ms, format!("{tag} {t}")));
        }
        for t in &samples.source_tokens {
            let (ms, r) = time(|| alignments.align(std::slice::from_ref(t), Some(&morphology)));
            r.map_err(err)?;
            lookups_source.push((ms, format!("{tag} {t}")));
        }
        for (edition, lemma) in &samples.lemmas {
            let editions = vec![edition.clone()];
            let (ms, r) = time(|| alignments.renderings(&morphology, &editions, lemma, None));
            r.map_err(err)?;
            tables.push((ms, format!("{tag} {edition} {lemma}")));
        }
    }
    let mut spot = Vec::new();
    for q in ["λόγος", "ἀγάπη", "ἀγαπάω", "φιλέω", "חֶסֶד", "אֱלֹהִים", "καί"]
    {
        let r = alignments
            .renderings(&morphology, &[], q, None)
            .map_err(err)?;
        spot.push((q.to_owned(), table_lines(&r, 12)));
    }
    Ok(Report {
        morphology_load_seconds: morph_ms / 1000.0,
        alignment_load_seconds: align_ms / 1000.0,
        rows: vec![
            row(
                "BSB token → original words (with morphology)",
                LOOKUP_BUDGET_MS,
                lookups_target,
            ),
            row(
                "original token → BSB words (with morphology)",
                LOOKUP_BUDGET_MS,
                lookups_source,
            ),
            row("lemma rendering table", RENDERING_BUDGET_MS, tables),
        ],
        spot,
        samples,
    })
}
