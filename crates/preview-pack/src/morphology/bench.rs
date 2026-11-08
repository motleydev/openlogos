//! Latency of the morphology layers natively, against the palette's 50 ms (specs/22 §7):
//! token lookups and lemma searches, each group at least 200 samples, chosen
//! deterministically across each layer (no randomness, so runs compare).

use std::path::Path;
use std::time::Instant;

use openlogos_retrieval_sqlite::MorphologyLayers;
use openlogos_retrieval_sqlite::tokenizer::normalize;
use serde::Serialize;

use crate::model::ImportError;

/// The budget.
pub const BUDGET_MS: f64 = 50.0;

/// What a lemma search returns at most, as a palette section would show.
pub const LIMIT: usize = 50;

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
}

/// The inputs the bench used, written beside the layers so the wasm bench replays the same
/// samples (`just preview-pack-morphology-bench-wasm`).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Samples {
    /// Token ids.
    pub tokens: Vec<String>,
    /// `(edition, lemma as spelled, its search key)`.
    pub lemmas: Vec<(String, String, String)>,
}

/// The file the samples are written to, beside `layers/`.
pub const SAMPLES_FILE: &str = "bench-samples.json";

/// The bench's answer.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Seconds to attach every layer and load its columns.
    pub load_seconds: f64,
    /// Per group.
    pub rows: Vec<Row>,
    /// The budget.
    pub budget_ms: f64,
    /// Spot checks: query → (matched_by, total, lemmas).
    pub spot: Vec<(String, String, u64, Vec<String>)>,
    /// The inputs.
    #[serde(skip)]
    pub samples: Samples,
}

fn row(group: &str, mut samples: Vec<(f64, String)>) -> Row {
    samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let at = |q: f64| samples[((samples.len() as f64 - 1.0) * q).round() as usize].0;
    Row {
        group: group.to_owned(),
        samples: samples.len(),
        p50_ms: at(0.5),
        p95_ms: at(0.95),
        max_ms: samples.last().map_or(0.0, |s| s.0),
        slowest: samples.last().map(|s| s.1.clone()).unwrap_or_default(),
    }
}

fn time<T>(f: impl FnOnce() -> T) -> (f64, T) {
    let start = Instant::now();
    let out = f();
    (start.elapsed().as_secs_f64() * 1000.0, out)
}

/// Runs the bench over `layers/` of a built morphology output.
pub fn run(layers_dir: &Path, rounds: usize) -> Result<Report, ImportError> {
    let err = |e: openlogos_retrieval::RetrievalError| ImportError::Pack(e.to_string());
    let (load_ms, layers) = time(|| MorphologyLayers::open_directory(layers_dir));
    let layers = layers.map_err(err)?;
    let mut lookups = Vec::new();
    let mut by_key = Vec::new();
    let mut exact = Vec::new();
    let mut by_token = Vec::new();
    let mut samples = Samples::default();
    for edition in layers.editions() {
        let (_, _, columns) = layers.columns(&edition).expect("listed");
        // Tokens: 250 evenly spaced positions' tokens.
        let n = columns.len();
        let lemma = columns.feature("lemma").expect("a layer has lemmas");
        let tokens: Vec<String> = (0..250)
            .map(|i| i * n / 250)
            .map(|pos| {
                let verse = columns.verse[pos];
                (verse, pos)
            })
            .filter_map(|(verse, pos)| {
                // The token of this position: its verse and word from the analysis.
                let book = verse / 1_000_000;
                let chapter = verse / 1000 % 1000;
                let v = verse % 1000;
                let word = layers.word_of(&edition, pos)?;
                (word != 0).then(|| format!("{edition}:{book:02}{chapter:03}{v:03}{word:03}"))
            })
            .collect();
        // Lemmas: 250 spread over the frequency ranks, most frequent included.
        let mut counts = vec![0u64; lemma.values.len()];
        for id in &lemma.ids {
            counts[*id as usize] += 1;
        }
        let mut ranked: Vec<usize> = (1..lemma.values.len()).collect();
        ranked.sort_by(|a, b| counts[*b].cmp(&counts[*a]).then(a.cmp(b)));
        let picks: Vec<String> = (0..250)
            .map(|i| ranked[i * ranked.len() / 250].min(ranked.len()))
            .map(|id| lemma.values[id].clone())
            .collect();
        let editions = vec![edition.clone()];
        samples.tokens.extend(tokens.iter().cloned());
        samples.lemmas.extend(
            picks
                .iter()
                .map(|l| (edition.clone(), l.clone(), normalize(l))),
        );
        for round in 0..rounds {
            // Round 0 is cold for every posting; later rounds hit the cache.
            let tag = if round == 0 { "cold" } else { "warm" };
            for t in &tokens {
                let (ms, r) = time(|| layers.analyze(t));
                r.map_err(err)?;
                lookups.push((ms, format!("{tag} {t}")));
            }
            for lemma in &picks {
                let typed = normalize(lemma);
                let (ms, r) = time(|| layers.lemma_search(&editions, &typed, LIMIT));
                r.map_err(err)?;
                by_key.push((ms, format!("{tag} {edition} {typed}")));
                let (ms, r) = time(|| layers.lemma_search(&editions, lemma, LIMIT));
                r.map_err(err)?;
                exact.push((ms, format!("{tag} {edition} {lemma}")));
            }
            for t in &tokens {
                let (ms, r) = time(|| layers.lemma_search_token(t, LIMIT));
                r.map_err(err)?;
                by_token.push((ms, format!("{tag} {t}")));
            }
        }
    }
    let mut spot = Vec::new();
    for q in [
        "ראשית",
        "בְּרֵאשִׁית",
        "אלהים",
        "λογος",
        "λόγος",
        "ἀγαπάω",
        "ו",
        "και",
        "אמר",
        "ἐν",
    ] {
        let r = layers.lemma_search(&[], q, LIMIT).map_err(err)?;
        spot.push((
            q.to_owned(),
            r.matched_by,
            r.total,
            r.lemmas
                .iter()
                .map(|l| format!("{} {} {} ({})", l.edition, l.lemma, l.strong, l.count))
                .collect(),
        ));
    }
    Ok(Report {
        load_seconds: load_ms / 1000.0,
        rows: vec![
            row("token lookup (analyze)", lookups),
            row("lemma search, typed key", by_key),
            row("lemma search, exact lemma", exact),
            row("lemma search from a token", by_token),
        ],
        budget_ms: BUDGET_MS,
        spot,
        samples,
    })
}
