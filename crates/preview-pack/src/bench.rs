//! Keystroke to text results, against the 50 ms of specs/22 §7: the queries of
//! `bench-queries.json` replayed keystroke by keystroke through the real provider and the palette
//! over the conformance session. `just preview-pack-bench` runs it on a built preview pack.

use std::path::Path;
use std::time::Instant;

use openlogos_conformance::fixture_session;
use openlogos_retrieval_sqlite::EditionSearch;
use openlogos_workspaces::commands::Shell;
use openlogos_workspaces::palette::{
    DEFAULT_SECTION_LIMIT, EXPANDED_SECTION_LIMIT, PaletteContext, PaletteIndex, TextProvider,
    palette_query_indexed,
};
use serde::{Deserialize, Serialize};

use crate::model::ImportError;

/// The query list, also what the wasm benchmark replays.
pub const QUERIES: &str = include_str!("../bench-queries.json");

/// `bench-queries.json`.
#[derive(Debug, Deserialize)]
pub struct QueryList {
    /// English.
    pub en: Vec<String>,
    /// Greek.
    pub grc: Vec<String>,
    /// Hebrew.
    pub hbo: Vec<String>,
}

/// Every keystroke of every query, from its second character.
#[must_use]
pub fn keystrokes(queries: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for query in queries {
        let chars: Vec<char> = query.chars().collect();
        for n in 2..=chars.len() {
            out.push(chars[..n].iter().collect());
        }
        if chars.len() < 2 {
            out.push(query.clone());
        }
    }
    out
}

/// Latencies of one group of queries, in milliseconds.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// The group.
    pub group: String,
    /// Samples.
    pub samples: usize,
    /// Median.
    pub p50_ms: f64,
    /// 95th percentile, the budget's measure.
    pub p95_ms: f64,
    /// Largest.
    pub max_ms: f64,
    /// The query behind the largest.
    pub slowest: String,
}

/// Nearest-rank percentile of sorted values.
#[must_use]
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

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

/// What the benchmark measured.
#[derive(Debug, Clone, Serialize)]
pub struct BenchReport {
    /// The budget.
    pub budget_ms: f64,
    /// `provider`: the search alone, per script and over everything; `palette`: the whole palette
    /// answer with text results, over everything.
    pub rows: Vec<Row>,
    /// Hits for a few queries, as a check that the numbers come from real results.
    pub spot_hits: Vec<(String, usize)>,
}

fn timed_ms(run: impl FnOnce() -> usize) -> (f64, usize) {
    let started = Instant::now();
    let hits = run();
    (started.elapsed().as_secs_f64() * 1000.0, hits)
}

/// Runs `rounds` passes over the list; the first pass counts, so a cold index is in the numbers.
pub fn run(search_dir: &Path, rounds: usize) -> Result<BenchReport, ImportError> {
    let pack_error = |e: &dyn std::fmt::Display| ImportError::Pack(e.to_string());
    let list: QueryList = serde_json::from_str(QUERIES).map_err(|e| pack_error(&e))?;
    let search = EditionSearch::open_directory(search_dir).map_err(|e| pack_error(&e))?;
    let provider: &dyn TextProvider = &search;
    let groups = [("en", &list.en), ("grc", &list.grc), ("hbo", &list.hbo)];
    let mut rows = Vec::new();
    let mut all = Vec::new();
    for (name, queries) in groups {
        let strokes = keystrokes(queries);
        let mut samples = Vec::new();
        for _ in 0..rounds {
            for stroke in &strokes {
                let (ms, _) = timed_ms(|| provider.search(stroke, DEFAULT_SECTION_LIMIT).len());
                samples.push((ms, stroke.clone()));
            }
        }
        all.extend(samples.iter().cloned());
        rows.push(row(&format!("provider, {name}"), samples));
    }
    rows.push(row("provider, all three", all));
    let mut expanded = Vec::new();
    for _ in 0..rounds {
        for (_, queries) in groups {
            for stroke in keystrokes(queries) {
                let (ms, _) = timed_ms(|| provider.search(&stroke, EXPANDED_SECTION_LIMIT).len());
                expanded.push((ms, stroke));
            }
        }
    }
    rows.push(row("provider, all three, expanded section", expanded));

    let shell = Shell::new(fixture_session()).map_err(|e| pack_error(&e))?;
    let index = PaletteIndex::new(&shell.session);
    let context = PaletteContext::default();
    let mut samples = Vec::new();
    for _ in 0..rounds {
        for (_, queries) in groups {
            for stroke in keystrokes(queries) {
                let (ms, _) = timed_ms(|| {
                    palette_query_indexed(&shell, &index, &stroke, &context, Some(provider))
                        .groups
                        .len()
                });
                samples.push((ms, stroke));
            }
        }
    }
    rows.push(row("palette with text results, all three", samples));
    let spot_hits = ["for god so loved", "λογος", "בראשית", "melchizedek"]
        .iter()
        .map(|q| ((*q).to_owned(), provider.search(q, 50).len()))
        .collect();
    Ok(BenchReport {
        budget_ms: 50.0,
        rows,
        spot_hits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_query_list_gives_at_least_200_keystrokes_in_all() {
        let list: QueryList = serde_json::from_str(QUERIES).unwrap();
        let total =
            keystrokes(&list.en).len() + keystrokes(&list.grc).len() + keystrokes(&list.hbo).len();
        assert!(total >= 200, "{total}");
    }

    #[test]
    fn keystrokes_start_at_the_second_character() {
        assert_eq!(keystrokes(&["abc".to_owned()]), ["ab", "abc"]);
        assert_eq!(keystrokes(&["ο".to_owned()]), ["ο"]);
    }

    #[test]
    fn the_percentile_is_nearest_rank() {
        let v: Vec<f64> = (1..=200).map(f64::from).collect();
        assert_eq!(percentile(&v, 95.0), 190.0);
        assert_eq!(percentile(&v, 50.0), 100.0);
        assert_eq!(percentile(&[], 95.0), 0.0);
    }
}
