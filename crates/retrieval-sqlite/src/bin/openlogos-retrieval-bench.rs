//! The B064 benchmark harness (`just bench-retrieval`). Plain `std` timing, no criterion:
//! every subcommand prints a Markdown table the evidence file quotes verbatim and, for R6,
//! writes the engine's counts as JSON for the Text-Fabric oracle to compare.
//!
//! ```text
//! openlogos-retrieval-bench ingest-tf <tf-dir> <pack.sqlite> [--source S] [--attribution A]
//! openlogos-retrieval-bench r6 <pack.sqlite> <queries.json> [--runs N] [--json out.json]
//! openlogos-retrieval-bench r5 <work-dir> [--sizes 50000,200000,500000] [--dims 256,768] [--runs N]
//! openlogos-retrieval-bench r9 <work-dir> [--packs 200] [--edges 5000000] [--runs N] [--reuse]
//! openlogos-retrieval-bench b068 <work-dir> [--packs 200] [--edges 1000000] [--runs N] [--reuse]
//! ```
//!
//! `b068` (docs/evidence/B068.md) builds the same synthetic pack set as `r9`, records it in a
//! catalog, builds the link index twice from the catalog's installed set and compares the
//! two files' SHA-256, then times incremental install and removal, the in-place rebuild that
//! keeps user edges, and the count-table `most_cited`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use openlogos_retrieval::{
    Direction, Locator, ProvenanceKind, Quantization, QueryVector, RelationStore, ReviewState,
    TextStore, TokenEngine, TokenQuery, VectorIndex, VerseId,
};
use openlogos_retrieval_sqlite::catalog::{Access, Catalog, sha256_file};
use openlogos_retrieval_sqlite::ingest::build_n1904_pack;
use openlogos_retrieval_sqlite::pack::{EdgeRow, PackBuilder, PackManifest};
use openlogos_retrieval_sqlite::{Engine, LinkIndex, UserEdge};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("ingest-tf") => ingest_tf(&args[2..]),
        Some("r6") => r6(&args[2..]),
        Some("r5") => r5(&args[2..]),
        Some("r9") => r9(&args[2..]),
        Some("b068") => b068(&args[2..]),
        _ => {
            eprintln!(
                "usage: openlogos-retrieval-bench ingest-tf|r6|r5|r9|b068 ... (see the crate's bin source)"
            );
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn list(args: &[String], name: &str, default: &[u64]) -> Vec<u64> {
    flag(args, name)
        .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| default.to_vec())
}

fn ms(d: Duration) -> String {
    format!("{:.3}", d.as_secs_f64() * 1000.0)
}

/// `(p50, p95, max)` of a sample set.
fn percentiles(samples: &mut [Duration]) -> (Duration, Duration, Duration) {
    samples.sort();
    let at = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
    (at(0.5), at(0.95), samples[samples.len() - 1])
}

fn time<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let v = f();
    (v, start.elapsed())
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn banner() {
    let conn = rusqlite::Connection::open_in_memory().expect("memory db");
    openlogos_retrieval_sqlite::vectors::install_vec_extension();
    let conn2 = rusqlite::Connection::open_in_memory().expect("memory db");
    let vec: String = conn2
        .query_row("SELECT vec_version()", [], |r| r.get(0))
        .unwrap_or_else(|_| "unavailable".into());
    drop(conn);
    println!(
        "engine: SQLite {} (bundled), sqlite-vec {vec}, roaring 0.11.5, target {}, {} build",
        rusqlite::version(),
        std::env::consts::ARCH,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
}

/// A small deterministic generator (xorshift64*) so runs are reproducible without a crate.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }
    fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let bytes = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
    }
    /// Skewed index in `0..n`: a power law with exponent 0.5 (index k drawn with probability
    /// about 1 / sqrt(k + 1)), so the most popular of 30,000 verses receives about 0.3 % of
    /// all citations (about 15,000 of 5M). A steeper Zipf (exponent 1) put 6 % of every
    /// edge on one verse, which no real library does (OpenBible's busiest verse has a few
    /// thousand inbound references).
    fn zipf(&mut self, n: u64) -> u64 {
        let u = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        ((n as f64 * u * u) as u64).min(n - 1)
    }
}

// ---------------------------------------------------------------------------------------------
// ingest-tf

fn ingest_tf(args: &[String]) -> Res<()> {
    let tf_dir = PathBuf::from(
        args.first()
            .ok_or("ingest-tf needs <tf-dir> <pack.sqlite>")?,
    );
    let out = PathBuf::from(
        args.get(1)
            .ok_or("ingest-tf needs <tf-dir> <pack.sqlite>")?,
    );
    if out.exists() {
        fs::remove_file(&out)?;
    }
    let manifest = PackManifest {
        edition: flag(args, "--edition").unwrap_or("n1904").to_owned(),
        scheme: "org".into(),
        source: flag(args, "--source").unwrap_or("CenterBLC/N1904 tf/1.0.0").to_owned(),
        license: flag(args, "--license").unwrap_or("MIT (repository); data from MACULA Greek, CC-BY-4.0").to_owned(),
        attribution: flag(args, "--attribution")
            .unwrap_or("Nestle 1904 Greek New Testament in Text-Fabric, CBLC and ETCBC, from MACULA Greek (Clear Bible)")
            .to_owned(),
        work: None,
        locator_scheme: None,
    };
    banner();
    let (report, took) = time(|| build_n1904_pack(&tf_dir, &out, &manifest));
    let report = report?;
    println!("| Measure | Value |\n|---|---|");
    println!("| tokens | {} |", report.tokens);
    println!("| verses | {} |", report.verses);
    for (kind, n) in &report.containers {
        println!("| containers `{kind}` | {n} |");
    }
    println!(
        "| discontinuous containers (stored as hull spans) | {} |",
        report.discontinuous
    );
    println!("| pack size | {} bytes |", file_size(&out));
    println!("| ingest time | {} s |", took.as_secs_f64());
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// R6

#[derive(serde::Deserialize)]
struct NamedQuery {
    id: String,
    family: String,
    description: String,
    query: TokenQuery,
}

#[derive(serde::Serialize)]
struct QueryResult {
    id: String,
    family: String,
    description: String,
    count: u64,
    cold_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    error: Option<String>,
}

fn r6(args: &[String]) -> Res<()> {
    let pack = PathBuf::from(
        args.first()
            .ok_or("r6 needs <pack.sqlite> <queries.json>")?,
    );
    let queries_path = PathBuf::from(args.get(1).ok_or("r6 needs <pack.sqlite> <queries.json>")?);
    let runs: usize = flag(args, "--runs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let queries: Vec<NamedQuery> = serde_json::from_str(&fs::read_to_string(&queries_path)?)?;
    banner();
    let mut engine = Engine::new()?;
    let (info, attach) = time(|| engine.attach_pack(&pack));
    let info = info?;
    let (_, load) = time(|| engine.columns(&info.edition));
    println!(
        "pack `{}`: {} verses, {} tokens, features {:?}, containers {:?}; attach {} ms, column load {} ms, {} runs per query",
        info.edition,
        info.verses,
        info.tokens,
        info.features,
        info.containers,
        ms(attach),
        ms(load),
        runs
    );
    println!(
        "\n| id | family | description | count | cold ms | p50 ms | p95 ms |\n|---|---|---|---:|---:|---:|---:|"
    );
    let mut results = Vec::new();
    let mut by_family: std::collections::BTreeMap<String, Vec<Duration>> =
        std::collections::BTreeMap::new();
    for q in &queries {
        let mut query = q.query.clone();
        query.edition = info.edition.clone();
        let (first, cold) = time(|| engine.token_search(&query));
        let (count, error) = match first {
            Ok(h) => (h.count, None),
            Err(e) => (0, Some(e.to_string())),
        };
        let mut samples = Vec::with_capacity(runs);
        for _ in 0..runs {
            let (_, d) = time(|| engine.token_search(&query));
            samples.push(d);
        }
        let (p50, p95, _) = percentiles(&mut samples);
        by_family
            .entry(q.family.clone())
            .or_default()
            .extend(samples.iter().copied());
        println!(
            "| {} | {} | {} | {} | {} | {} | {} |",
            q.id,
            q.family,
            q.description,
            error
                .as_ref()
                .map_or_else(|| count.to_string(), |e| format!("error: {e}")),
            ms(cold),
            ms(p50),
            ms(p95)
        );
        results.push(QueryResult {
            id: q.id.clone(),
            family: q.family.clone(),
            description: q.description.clone(),
            count,
            cold_ms: cold.as_secs_f64() * 1000.0,
            p50_ms: p50.as_secs_f64() * 1000.0,
            p95_ms: p95.as_secs_f64() * 1000.0,
            error,
        });
    }
    println!("\n| family | queries | p50 ms | p95 ms | max ms |\n|---|---:|---:|---:|---:|");
    for (family, samples) in by_family.iter_mut() {
        let n = samples.len() / runs;
        let (p50, p95, max) = percentiles(samples);
        println!(
            "| {family} | {n} | {} | {} | {} |",
            ms(p50),
            ms(p95),
            ms(max)
        );
    }
    // Facets over the first lemma query, as specs/19's worked example asks.
    if let Some(q) = queries.iter().find(|q| q.family == "lemma") {
        let mut query = q.query.clone();
        query.edition = info.edition.clone();
        let hits = engine.token_search(&query)?;
        let positions: Vec<u32> = hits.matches.iter().map(|m| m.positions[0]).collect();
        let (by_book, d1) = time(|| engine.facets(&info.edition, &positions, "book"));
        let (by_form, d2) = time(|| engine.facets(&info.edition, &positions, "normalized"));
        println!(
            "\nfacets for {} ({} hits): {} books in {} ms, {} forms in {} ms; top book {:?}",
            q.id,
            hits.count,
            by_book.as_ref().map_or(0, Vec::len),
            ms(d1),
            by_form.as_ref().map_or(0, Vec::len),
            ms(d2),
            by_book.ok().and_then(|f| f.first().cloned())
        );
    }
    if let Some(out) = flag(args, "--json") {
        fs::write(out, serde_json::to_string_pretty(&results)?)?;
        println!("\nwrote {out}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// R5

fn r5(args: &[String]) -> Res<()> {
    let dir = PathBuf::from(args.first().ok_or("r5 needs <work-dir>")?);
    fs::create_dir_all(&dir)?;
    let sizes = list(args, "--sizes", &[50_000, 200_000, 500_000]);
    let dims = list(args, "--dims", &[256, 768]);
    let runs: usize = flag(args, "--runs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let k: usize = flag(args, "--k").and_then(|s| s.parse().ok()).unwrap_or(20);
    banner();
    println!(
        "random vectors (xorshift64*, seed per table), k = {k}, {runs} timed queries after 3 warm-ups\n"
    );
    println!(
        "| vectors | dims | quantization | db bytes | build s | p50 ms | p95 ms | max ms |\n|---:|---:|---|---:|---:|---:|---:|---:|"
    );
    for &d in &dims {
        for quant in [Quantization::Int8, Quantization::Binary] {
            for &n in &sizes {
                let name = format!(
                    "v{n}_{d}_{}",
                    if quant == Quantization::Int8 {
                        "i8"
                    } else {
                        "bit"
                    }
                );
                let path = dir.join(format!("{name}.sqlite"));
                let bytes_per = match quant {
                    Quantization::Int8 => d as usize,
                    Quantization::Binary => d as usize / 8,
                };
                let build = if path.exists() && !has_flag(args, "--rebuild") {
                    None
                } else {
                    let _ = fs::remove_file(&path);
                    let manifest = PackManifest {
                        edition: name.clone(),
                        scheme: "none".into(),
                        source: "synthetic".into(),
                        license: "CC0-1.0".into(),
                        attribution: String::new(),
                        work: None,
                        locator_scheme: None,
                    };
                    let (_, took) = time(|| -> Res<()> {
                        let mut b = PackBuilder::create(&path, &manifest, &[])?;
                        b.add_vector_set("v", "random", "seed", d as u32, quant)?;
                        let mut rng = Rng::new(n * 1000 + d);
                        let mut buf = vec![0u8; bytes_per];
                        let mut rows: Vec<(i64, Vec<u8>)> = Vec::with_capacity(10_000);
                        for id in 0..n as i64 {
                            rng.fill(&mut buf);
                            rows.push((id, buf.clone()));
                            if rows.len() == 10_000 || id == n as i64 - 1 {
                                b.add_vectors("v", rows.iter().map(|(i, v)| (*i, v.as_slice())))?;
                                rows.clear();
                            }
                        }
                        b.finish()?;
                        Ok(())
                    });
                    Some(took)
                };
                let mut engine = Engine::new()?;
                engine.attach_pack(&path)?;
                let mut rng = Rng::new(7);
                let mut query_bytes = vec![0u8; bytes_per];
                let mut samples = Vec::with_capacity(runs);
                for i in 0..runs + 3 {
                    rng.fill(&mut query_bytes);
                    let query = match quant {
                        Quantization::Int8 => {
                            QueryVector::Int8(query_bytes.iter().map(|b| *b as i8).collect())
                        }
                        Quantization::Binary => QueryVector::Binary(query_bytes.clone()),
                    };
                    let (hits, took) = time(|| engine.nearest("v", &query, k));
                    let hits = hits?;
                    assert_eq!(hits.len(), k.min(n as usize));
                    if i >= 3 {
                        samples.push(took);
                    }
                }
                let (p50, p95, max) = percentiles(&mut samples);
                println!(
                    "| {n} | {d} | {} | {} | {} | {} | {} | {} |",
                    if quant == Quantization::Int8 {
                        "int8"
                    } else {
                        "binary"
                    },
                    file_size(&path),
                    build.map_or_else(
                        || "reused".to_owned(),
                        |b| format!("{:.1}", b.as_secs_f64())
                    ),
                    ms(p50),
                    ms(p95),
                    ms(max)
                );
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// R9

/// Shape of the synthetic library: 200 pack-shaped sources, about 5M edges.
struct SyntheticPack {
    id: String,
    kind: &'static str,
    edges: u64,
}

fn plan_packs(packs: u64, edges: u64, rng: &mut Rng) -> Vec<SyntheticPack> {
    // One cross-reference pack (OpenBible scale), 20 lexicons, 19 other works, the rest
    // commentaries; sizes vary so that the total meets `edges`.
    let mut plan = Vec::new();
    plan.push(SyntheticPack {
        id: "xref-openbible".into(),
        kind: "xref",
        edges: 340_000.min(edges / 10),
    });
    let lexicons = 20.min(packs.saturating_sub(2));
    let others = 19.min(packs.saturating_sub(2 + lexicons));
    let commentaries = packs.saturating_sub(1 + lexicons + others);
    let remaining = edges.saturating_sub(plan[0].edges);
    let lex_share = remaining / 10;
    let other_share = remaining / 12;
    let comm_share = remaining - lex_share - other_share;
    for i in 0..lexicons {
        plan.push(SyntheticPack {
            id: format!("lexicon-{i:02}"),
            kind: "lexicon",
            edges: lex_share / lexicons.max(1),
        });
    }
    for i in 0..others {
        plan.push(SyntheticPack {
            id: format!("work-{i:02}"),
            kind: "work",
            edges: other_share / others.max(1),
        });
    }
    // Commentary sizes: random weights so a few are large.
    let weights: Vec<u64> = (0..commentaries).map(|_| 1 + rng.zipf(8)).collect();
    let total_w: u64 = weights.iter().sum::<u64>().max(1);
    for (i, w) in weights.iter().enumerate() {
        plan.push(SyntheticPack {
            id: format!("commentary-{i:03}"),
            kind: "commentary",
            edges: comm_share * w / total_w,
        });
    }
    plan
}

/// 31,102 verse ids of a Protestant canon, approximated as 66 books × chapters × verses from a
/// fixed table of chapter counts with 25 verses each; exact counts do not matter for timing.
fn verse_universe() -> Vec<VerseId> {
    const CHAPTERS: [u16; 66] = [
        50, 40, 27, 36, 34, 24, 21, 4, 31, 24, 22, 25, 29, 36, 10, 13, 10, 42, 150, 31, 12, 8, 66,
        52, 5, 48, 12, 14, 3, 9, 1, 4, 7, 3, 3, 3, 2, 14, 4, 28, 16, 24, 21, 28, 16, 16, 13, 6, 6,
        4, 4, 5, 3, 6, 4, 3, 1, 13, 5, 5, 3, 5, 1, 1, 1, 22,
    ];
    let mut verses = Vec::with_capacity(31_102);
    for (i, chapters) in CHAPTERS.iter().enumerate() {
        let book = if i < 39 { i as u8 + 1 } else { i as u8 + 2 };
        for c in 1..=*chapters {
            for v in 1..=25u16 {
                verses.push(VerseId::new(book, c, v));
            }
        }
    }
    verses
}

fn build_synthetic_pack(
    dir: &Path,
    plan: &SyntheticPack,
    verses: &[VerseId],
    rng: &mut Rng,
) -> Res<PathBuf> {
    let path = dir.join(format!("{}.sqlite", plan.id));
    let _ = fs::remove_file(&path);
    let manifest = PackManifest {
        edition: plan.id.clone(),
        scheme: "org".into(),
        source: "synthetic (R9)".into(),
        license: "CC0-1.0".into(),
        attribution: String::new(),
        work: None,
        locator_scheme: None,
    };
    let mut b = PackBuilder::create(&path, &manifest, &[])?;
    let nv = verses.len() as u64;
    let verse_text = |v: VerseId| v.0.to_string();
    for i in 0..plan.edges {
        let dst_verse = verses[rng.zipf(nv) as usize];
        match plan.kind {
            "xref" => {
                let src_verse = verses[rng.below(nv) as usize];
                b.add_edge(&EdgeRow {
                    src_work: "bible",
                    src_locator: &verse_text(src_verse),
                    rel: "cross_reference",
                    dst_work: "bible",
                    dst_locator: &verse_text(dst_verse),
                    weight: rng.below(100) as f32,
                    source_work: &plan.id,
                    method: "openbible-votes",
                    provenance: ProvenanceKind::Curated,
                    review_state: ReviewState::SourceAttested,
                    context: None,
                })?;
            }
            "lexicon" => {
                let lexeme = format!("lex{}", rng.zipf(6000));
                if i % 4 == 0 {
                    let other = format!("lex{}", rng.below(6000));
                    b.add_edge(&EdgeRow {
                        src_work: &plan.id,
                        src_locator: &lexeme,
                        rel: if i % 8 == 0 {
                            "synonym_of"
                        } else {
                            "cognate_of"
                        },
                        dst_work: &plan.id,
                        dst_locator: &other,
                        weight: 0.0,
                        source_work: &plan.id,
                        method: "editorial",
                        provenance: ProvenanceKind::Curated,
                        review_state: ReviewState::SourceAttested,
                        context: None,
                    })?;
                } else {
                    b.add_edge(&EdgeRow {
                        src_work: &plan.id,
                        src_locator: &lexeme,
                        rel: "attested_in",
                        dst_work: "bible",
                        dst_locator: &verse_text(dst_verse),
                        weight: 0.0,
                        source_work: &plan.id,
                        method: "editorial",
                        provenance: ProvenanceKind::Curated,
                        review_state: ReviewState::SourceAttested,
                        context: None,
                    })?;
                }
            }
            "work" => {
                let section = format!(
                    "{}.{}.{}",
                    1 + rng.below(4),
                    1 + rng.below(25),
                    1 + rng.below(30)
                );
                if i % 5 == 0 {
                    // A quotation chain hop into another work.
                    let other = format!("work-{:02}", rng.below(19));
                    let other_section = format!(
                        "{}.{}.{}",
                        1 + rng.below(4),
                        1 + rng.below(25),
                        1 + rng.below(30)
                    );
                    b.add_edge(&EdgeRow {
                        src_work: &plan.id,
                        src_locator: &section,
                        rel: "quotes",
                        dst_work: &other,
                        dst_locator: &other_section,
                        weight: 0.0,
                        source_work: &plan.id,
                        method: "scripRef-parsed",
                        provenance: ProvenanceKind::Extracted,
                        review_state: ReviewState::SourceAttested,
                        context: None,
                    })?;
                } else {
                    b.add_edge(&EdgeRow {
                        src_work: &plan.id,
                        src_locator: &section,
                        rel: "cites",
                        dst_work: "bible",
                        dst_locator: &verse_text(dst_verse),
                        weight: 0.0,
                        source_work: &plan.id,
                        method: "bcv-parser@2",
                        provenance: ProvenanceKind::Extracted,
                        review_state: ReviewState::SourceAttested,
                        context: None,
                    })?;
                }
            }
            _ => {
                let paragraph = format!("{}.{}", 1 + rng.below(66), 1 + rng.below(400));
                b.add_edge(&EdgeRow {
                    src_work: &plan.id,
                    src_locator: &paragraph,
                    rel: "cites",
                    dst_work: "bible",
                    dst_locator: &verse_text(dst_verse),
                    weight: 0.0,
                    source_work: &plan.id,
                    method: "bcv-parser@2",
                    provenance: ProvenanceKind::Extracted,
                    review_state: ReviewState::SourceAttested,
                    context: None,
                })?;
            }
        }
    }
    b.finish()?;
    Ok(path)
}

fn r9(args: &[String]) -> Res<()> {
    let dir = PathBuf::from(args.first().ok_or("r9 needs <work-dir>")?);
    fs::create_dir_all(&dir)?;
    let packs: u64 = flag(args, "--packs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);
    let edges: u64 = flag(args, "--edges")
        .and_then(|s| s.parse().ok())
        .unwrap_or(5_000_000);
    let runs: usize = flag(args, "--runs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    let reuse = has_flag(args, "--reuse");
    banner();
    let verses = verse_universe();
    let mut rng = Rng::new(2026);
    let plan = plan_packs(packs, edges, &mut rng);
    let planned: u64 = plan.iter().map(|p| p.edges).sum();
    println!(
        "{} packs planned, {planned} edges (xref {}, lexicons {}, works {}, commentaries {}); destinations power-law (exponent 0.5) over {} verses",
        plan.len(),
        plan.iter().filter(|p| p.kind == "xref").count(),
        plan.iter().filter(|p| p.kind == "lexicon").count(),
        plan.iter().filter(|p| p.kind == "work").count(),
        plan.iter().filter(|p| p.kind == "commentary").count(),
        verses.len()
    );

    // Build (or reuse) the pack files.
    let pack_dir = dir.join("packs");
    fs::create_dir_all(&pack_dir)?;
    let mut paths: Vec<(String, PathBuf)> = Vec::new();
    let mut pack_bytes = 0u64;
    let (_, build_time) = time(|| -> Res<()> {
        for p in &plan {
            let path = pack_dir.join(format!("{}.sqlite", p.id));
            if !(reuse && path.exists()) {
                build_synthetic_pack(&pack_dir, p, &verses, &mut rng)?;
            }
            pack_bytes += file_size(&path);
            paths.push((p.id.clone(), path));
        }
        Ok(())
    });
    println!(
        "packs: {} bytes on disk, build {:.1} s{}",
        pack_bytes,
        build_time.as_secs_f64(),
        if reuse { " (reused where present)" } else { "" }
    );

    // Link index: full rebuild from all packs but the last commentary, then incremental.
    let index_path = dir.join("links.sqlite");
    let (last_id, last_path) = paths.last().cloned().ok_or("no packs")?;
    let mut index = LinkIndex::open(&index_path)?;
    let rebuild = if reuse && index.edge_count()? > 0 {
        None
    } else {
        let _ = fs::remove_file(&index_path);
        let _ = fs::remove_file(dir.join("links.sqlite-wal"));
        index = LinkIndex::open(&index_path)?;
        let (n, took) = time(|| index.rebuild(&paths[..paths.len() - 1]));
        n?;
        Some(took)
    };
    let (n_incr, incremental) = time(|| index.install_pack(&last_id, &last_path));
    let n_incr = n_incr?;
    let (_, removal) = time(|| index.remove_pack(&last_id));
    let (_, reinstall) = time(|| index.install_pack(&last_id, &last_path));
    let total = index.edge_count()?;
    index
        .connection()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    println!("\n| Measure | Value |\n|---|---|");
    println!("| edges in link index | {total} |");
    println!("| link index bytes | {} |", file_size(&index_path));
    println!(
        "| full rebuild ({} packs) | {} |",
        paths.len() - 1,
        rebuild.map_or_else(
            || "reused".to_owned(),
            |d| format!("{:.1} s", d.as_secs_f64())
        )
    );
    println!(
        "| incremental install of `{last_id}` ({n_incr} edges) | {} ms |",
        ms(incremental)
    );
    println!("| removal of `{last_id}` | {} ms |", ms(removal));
    println!("| reinstall of `{last_id}` | {} ms |", ms(reinstall));

    // Query workload.
    let engine = Engine::new()?.with_link_index(&index_path)?;
    let mut qrng = Rng::new(99);
    let popular: Vec<Locator> = (0..runs)
        .map(|_| Locator::verse("bible", verses[qrng.zipf(verses.len() as u64) as usize]))
        .collect();
    let uniform: Vec<Locator> = (0..runs)
        .map(|_| Locator::verse("bible", verses[qrng.below(verses.len() as u64) as usize]))
        .collect();

    println!(
        "\n| Query | p50 ms | p95 ms | max ms | median result size |\n|---|---:|---:|---:|---:|"
    );
    let row = |name: &str, samples: &mut Vec<Duration>, sizes: &mut Vec<u64>| {
        let (p50, p95, max) = percentiles(samples);
        sizes.sort();
        println!(
            "| {name} | {} | {} | {} | {} |",
            ms(p50),
            ms(p95),
            ms(max),
            sizes[sizes.len() / 2]
        );
    };

    for (label, targets) in [
        ("skew-popular verses", &popular),
        ("uniform verses", &uniform),
    ] {
        let mut s = Vec::new();
        let mut sizes = Vec::new();
        for t in targets {
            let (r, d) = time(|| engine.backlinks(t, None, 100));
            sizes.push(r?.len() as u64);
            s.push(d);
        }
        row(
            &format!("backlinks (limit 100), {label}"),
            &mut s,
            &mut sizes,
        );
        let mut s = Vec::new();
        let mut sizes = Vec::new();
        for t in targets {
            let (r, d) = time(|| engine.backlink_count(t, None));
            sizes.push(r?);
            s.push(d);
        }
        row(&format!("backlink_count, {label}"), &mut s, &mut sizes);
        let mut s = Vec::new();
        let mut sizes = Vec::new();
        for t in targets {
            let (r, d) = time(|| engine.backlinks(t, Some("cross_reference"), 100));
            sizes.push(r?.len() as u64);
            s.push(d);
        }
        row(
            &format!("backlinks rel=cross_reference, {label}"),
            &mut s,
            &mut sizes,
        );
    }

    for depth in 1..=6u8 {
        let options = openlogos_retrieval::TraversalOptions {
            rels: vec!["cross_reference".into()],
            direction: Direction::Both,
            max_depth: depth,
            fan_out: 20,
            max_nodes: 5000,
        };
        let mut s = Vec::new();
        let mut sizes = Vec::new();
        for t in popular.iter().take(20) {
            let (r, d) = time(|| engine.traverse(t, &options));
            sizes.push(r?.len() as u64);
            s.push(d);
        }
        row(
            &format!(
                "traverse cross_reference both ways, depth {depth}, fan-out 20, max 5000 nodes"
            ),
            &mut s,
            &mut sizes,
        );
    }
    for depth in 1..=3u8 {
        let mut s = Vec::new();
        let mut sizes = Vec::new();
        let index = engine.link_index().ok_or("no index")?;
        for t in popular.iter().take(10) {
            let (r, d) = time(|| index.reach_count_recursive(t, Some("cross_reference"), depth));
            sizes.push(r?);
            s.push(d);
        }
        row(
            &format!(
                "WITH RECURSIVE reach count (no caps), outbound cross_reference, depth {depth}"
            ),
            &mut s,
            &mut sizes,
        );
    }
    // Quotation chains between works, with path deduplication.
    let chain_starts: Vec<Locator> = (0..20)
        .map(|_| Locator {
            work: format!("work-{:02}", qrng.below(19)),
            locator: format!(
                "{}.{}.{}",
                1 + qrng.below(4),
                1 + qrng.below(25),
                1 + qrng.below(30)
            ),
        })
        .collect();
    let options = openlogos_retrieval::TraversalOptions {
        rels: vec!["quotes".into()],
        direction: Direction::Both,
        max_depth: 6,
        fan_out: 50,
        max_nodes: 10_000,
    };
    let mut s = Vec::new();
    let mut sizes = Vec::new();
    for t in &chain_starts {
        let (r, d) = time(|| engine.traverse(t, &options));
        sizes.push(r?.len() as u64);
        s.push(d);
    }
    row(
        "quotation chains (quotes, both ways, depth 6, fan-out 50)",
        &mut s,
        &mut sizes,
    );

    let mut s = Vec::new();
    let mut sizes = Vec::new();
    for _ in 0..5 {
        let (r, d) = time(|| engine.most_cited("bible", None, 20));
        sizes.push(r?.len() as u64);
        s.push(d);
    }
    row(
        "most_cited(bible, any rel, top 20) — library-relative aggregate",
        &mut s,
        &mut sizes,
    );
    let mut s = Vec::new();
    let mut sizes = Vec::new();
    for _ in 0..5 {
        let (r, d) = time(|| engine.most_cited("bible", Some("cross_reference"), 20));
        sizes.push(r?.len() as u64);
        s.push(d);
    }
    row(
        "most_cited(bible, cross_reference, top 20)",
        &mut s,
        &mut sizes,
    );
    let top = engine.most_cited("bible", None, 3)?;
    println!(
        "\nmost cited in this library: {:?}",
        top.iter()
            .map(|(l, n)| (l.locator.clone(), *n))
            .collect::<Vec<_>>()
    );
    let _ = engine.editions();
    Ok(())
}

/// B068: deterministic rebuild, incremental install and removal, user edges, count table.
fn b068(args: &[String]) -> Res<()> {
    let dir = PathBuf::from(args.first().ok_or("b068 needs <work-dir>")?);
    fs::create_dir_all(&dir)?;
    let packs: u64 = flag(args, "--packs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);
    let edges: u64 = flag(args, "--edges")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);
    let runs: usize = flag(args, "--runs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let reuse = has_flag(args, "--reuse");
    banner();
    let verses = verse_universe();
    let mut rng = Rng::new(2026);
    let plan = plan_packs(packs, edges, &mut rng);
    let planned: u64 = plan.iter().map(|p| p.edges).sum();
    println!("{} packs planned, {planned} edges", plan.len());

    let pack_dir = dir.join("packs");
    fs::create_dir_all(&pack_dir)?;
    let mut paths: Vec<(String, PathBuf)> = Vec::new();
    let mut pack_bytes = 0u64;
    let (_, build_time) = time(|| -> Res<()> {
        for p in &plan {
            let path = pack_dir.join(format!("{}.sqlite", p.id));
            if !(reuse && path.exists()) {
                build_synthetic_pack(&pack_dir, p, &verses, &mut rng)?;
            }
            pack_bytes += file_size(&path);
            paths.push((p.id.clone(), path));
        }
        Ok(())
    });
    println!(
        "packs: {pack_bytes} bytes on disk, build {:.1} s{}",
        build_time.as_secs_f64(),
        if reuse { " (reused where present)" } else { "" }
    );

    // The catalog: hash and register every pack (the installed set).
    let catalog_path = dir.join("catalog.sqlite");
    let _ = fs::remove_file(&catalog_path);
    let mut catalog = Catalog::open(&catalog_path)?;
    catalog.register_builtin_schemes()?;
    let (_, catalog_time) = time(|| -> Res<()> {
        for (id, path) in &paths {
            catalog.install_pack(id, path, Access::Open)?;
        }
        Ok(())
    });
    let installed = catalog.pack_list()?;

    // Two fresh builds from the same installed set, the second from the reversed list.
    let user_edges: Vec<UserEdge> = (0..3)
        .map(|i| UserEdge {
            source: format!("note-{i}"),
            edge: openlogos_retrieval::Edge {
                src: Locator {
                    work: "notes".into(),
                    locator: format!("n{i}"),
                },
                rel: "cites".into(),
                dst: Locator::verse("bible", verses[i * 1000]),
                weight: 0.0,
                provenance: ProvenanceKind::User,
                source_work: "user-db".into(),
                method: "note".into(),
                review_state: ReviewState::Published,
            },
            context: Some("a note".into()),
        })
        .collect();
    let a = dir.join("links-a.sqlite");
    let b = dir.join("links-b.sqlite");
    let (n_a, build_a) = time(|| LinkIndex::build_fresh(&a, &installed, &user_edges));
    let n_a = n_a?;
    let reversed: Vec<(String, PathBuf)> = installed.iter().rev().cloned().collect();
    let (n_b, build_b) = time(|| LinkIndex::build_fresh(&b, &reversed, &user_edges));
    let n_b = n_b?;
    let (hash_a, hash_time) = time(|| sha256_file(&a));
    let hash_a = hash_a?;
    let hash_b = sha256_file(&b)?;
    let identical = hash_a == hash_b && file_size(&a) == file_size(&b);

    println!("\n| Measure | Value |\n|---|---|");
    println!("| packs | {} |", installed.len());
    println!(
        "| edges per build | {n_a} / {n_b} (+ {} user edges) |",
        user_edges.len()
    );
    println!(
        "| catalog install (hash + manifest + section maps) of {} packs | {:.1} s |",
        installed.len(),
        catalog_time.as_secs_f64()
    );
    println!(
        "| fresh build A (pack-id order) | {:.1} s, {} bytes |",
        build_a.as_secs_f64(),
        file_size(&a)
    );
    println!(
        "| fresh build B (reversed input order) | {:.1} s, {} bytes |",
        build_b.as_secs_f64(),
        file_size(&b)
    );
    println!(
        "| sha256 A | `{hash_a}` ({:.1} s to hash) |",
        hash_time.as_secs_f64()
    );
    println!("| sha256 B | `{hash_b}` |");
    println!(
        "| byte-identical | **{}** |",
        if identical { "yes" } else { "NO" }
    );
    let _ = fs::remove_file(&b);

    // Incremental work on A.
    let mut index = LinkIndex::open(&a)?;
    let (last_id, last_path) = installed.last().cloned().ok_or("no packs")?;
    let last_edges = index
        .installed()?
        .iter()
        .find(|(id, _)| *id == last_id)
        .map_or(0, |(_, n)| *n);
    let (_, removal) = time(|| index.remove_pack(&last_id));
    let (_, reinstall) = time(|| index.install_pack(&last_id, &last_path));
    let (first_id, first_path) = installed.first().cloned().ok_or("no packs")?;
    let first_edges = index
        .installed()?
        .iter()
        .find(|(id, _)| *id == first_id)
        .map_or(0, |(_, n)| *n);
    let (_, removal_first) = time(|| index.remove_pack(&first_id));
    let (_, reinstall_first) = time(|| index.install_pack(&first_id, &first_path));
    println!(
        "| removal of `{last_id}` ({last_edges} edges) | {} ms |",
        ms(removal)
    );
    println!("| reinstall of `{last_id}` | {} ms |", ms(reinstall));
    println!(
        "| removal of `{first_id}` ({first_edges} edges) | {} ms |",
        ms(removal_first)
    );
    println!("| reinstall of `{first_id}` | {} ms |", ms(reinstall_first));
    println!("| cited_count rows | {} |", index.count_rows()?);

    // In-place rebuild keeps the user edges.
    let before = index.user_edges()?;
    let (_, rebuild) = time(|| index.rebuild(&installed));
    let after = index.user_edges()?;
    println!(
        "| in-place rebuild (user edges kept) | {:.1} s, {} → {} user edges |",
        rebuild.as_secs_f64(),
        before.len(),
        after.len()
    );
    index
        .connection()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    println!(
        "| link index bytes after rebuild (WAL checkpointed) | {} |",
        file_size(&a)
    );

    // Queries: the count table against the old GROUP BY, and backlink counts.
    println!(
        "\n| Query | p50 ms | p95 ms | max ms | median result size |\n|---|---:|---:|---:|---:|"
    );
    let row = |name: &str, samples: &mut Vec<Duration>, sizes: &mut Vec<u64>| {
        let (p50, p95, max) = percentiles(samples);
        sizes.sort();
        println!(
            "| {name} | {} | {} | {} | {} |",
            ms(p50),
            ms(p95),
            ms(max),
            sizes[sizes.len() / 2]
        );
    };
    for rel in [None, Some("cross_reference"), Some("cites")] {
        let mut s = Vec::new();
        let mut sizes = Vec::new();
        for _ in 0..runs {
            let (r, d) = time(|| index.most_cited("bible", rel, 20));
            sizes.push(r?.len() as u64);
            s.push(d);
        }
        row(
            &format!(
                "most_cited(bible, {}, top 20) from cited_count",
                rel.unwrap_or("any rel")
            ),
            &mut s,
            &mut sizes,
        );
    }
    // Correctness: the count table agrees with a GROUP BY over link for the top 20.
    let top = index.most_cited("bible", None, 20)?;
    let direct: Vec<(String, u64)> = {
        let mut stmt = index.connection().prepare(
            "SELECT dst_locator, count(*) AS n FROM link WHERE dst_work = (SELECT id FROM work WHERE name = 'bible')
             GROUP BY dst_locator ORDER BY n DESC, dst_locator LIMIT 20",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
        })?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    let (_, group_by_time) = time(|| -> Res<()> {
        let mut stmt = index.connection().prepare(
            "SELECT dst_locator, count(*) AS n FROM link WHERE dst_work = (SELECT id FROM work WHERE name = 'bible')
             GROUP BY dst_locator ORDER BY n DESC, dst_locator LIMIT 20",
        )?;
        let n = stmt.query_map([], |r| r.get::<_, String>(0))?.count();
        assert_eq!(n, 20);
        Ok(())
    });
    let agrees = top
        .iter()
        .map(|(l, n)| (l.locator.clone(), *n))
        .collect::<Vec<_>>()
        == direct;
    println!(
        "| most_cited(bible, any rel, top 20) by GROUP BY over link (B064 path), one run | {} | | | 20 |",
        ms(group_by_time)
    );
    println!("\ncount table agrees with GROUP BY on the top 20: {agrees}");
    let mut qrng = Rng::new(99);
    let targets: Vec<Locator> = (0..runs)
        .map(|_| Locator::verse("bible", verses[qrng.zipf(verses.len() as u64) as usize]))
        .collect();
    let mut s = Vec::new();
    let mut sizes = Vec::new();
    for t in &targets {
        let (r, d) = time(|| index.backlink_count(t, None));
        sizes.push(r?);
        s.push(d);
    }
    row(
        "backlink_count from cited_count, skew-popular verses",
        &mut s,
        &mut sizes,
    );
    let mut s = Vec::new();
    let mut sizes = Vec::new();
    for t in &targets {
        let (r, d) = time(|| index.backlinks(t, None, 100));
        sizes.push(r?.len() as u64);
        s.push(d);
    }
    row(
        "backlinks (limit 100), skew-popular verses",
        &mut s,
        &mut sizes,
    );
    let chain_starts: Vec<Locator> = (0..runs)
        .map(|_| Locator {
            work: format!("work-{:02}", qrng.below(19)),
            locator: format!(
                "{}.{}.{}",
                1 + qrng.below(4),
                1 + qrng.below(25),
                1 + qrng.below(30)
            ),
        })
        .collect();
    let options = openlogos_retrieval::ChainOptions {
        rels: vec!["quotes".into()],
        direction: Direction::Both,
        max_depth: 6,
        fan_out: 50,
        max_nodes: 10_000,
    };
    let mut s = Vec::new();
    let mut sizes = Vec::new();
    for t in &chain_starts {
        let (r, d) = time(|| index.citation_chains(t, &options));
        sizes.push(r?.len() as u64);
        s.push(d);
    }
    row(
        "citation_chains (quotes, both ways, depth 6, fan-out 50)",
        &mut s,
        &mut sizes,
    );
    let mut s = Vec::new();
    let mut sizes = Vec::new();
    for (edge, context) in index.outlinks_with_context(&chain_starts[0], 50)? {
        let (r, d) = time(|| catalog.cited_target(&edge, context.as_deref()));
        sizes.push(u64::from(matches!(
            r?.status,
            openlogos_retrieval::TargetStatus::InLibrary { .. }
        )));
        s.push(d);
    }
    if !s.is_empty() {
        row(
            "catalog.cited_target per outlink of one work section (1 = in library)",
            &mut s,
            &mut sizes,
        );
    }
    let _ = Engine::new();
    Ok(())
}
