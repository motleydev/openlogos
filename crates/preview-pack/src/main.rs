//! `openlogos-preview-pack build`: see the library's documentation and docs/evidence/B071.md.

use std::path::PathBuf;
use std::process::ExitCode;

use openlogos_preview_pack::bench;
use openlogos_preview_pack::build::{BuildOptions, build};
use openlogos_preview_pack::model::ImportError;
use openlogos_preview_pack::morphology::{self, MorphologyOptions};

const DEFAULT_CORPORA: &str = "/Volumes/office-hub/openlogos/corpora";

fn default_morphology() -> PathBuf {
    std::env::var_os("OPENLOGOS_MORPHOLOGY").map_or_else(
        || {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".cache/openlogos/morphology")
        },
        PathBuf::from,
    )
}

fn default_out() -> PathBuf {
    std::env::var_os("OPENLOGOS_PREVIEW_PACK").map_or_else(
        || {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".cache/openlogos/preview-pack")
        },
        PathBuf::from,
    )
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, ImportError> {
    args.next()
        .ok_or_else(|| ImportError::Usage(format!("{flag} needs a value")))
}

fn run() -> Result<(), ImportError> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let mut corpora = PathBuf::from(DEFAULT_CORPORA);
    let mut rights = PathBuf::from("rights");
    let mut out = default_out();
    let mut morphology_out = default_morphology();
    let mut rounds = 3usize;
    let mut json: Option<PathBuf> = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--corpora" => corpora = PathBuf::from(value(&mut args, &flag)?),
            "--rights" => rights = PathBuf::from(value(&mut args, &flag)?),
            "--out" => out = PathBuf::from(value(&mut args, &flag)?),
            "--rounds" => {
                rounds = value(&mut args, &flag)?
                    .parse()
                    .map_err(|_| ImportError::Usage("--rounds needs a number".to_owned()))?;
            }
            "--json" => json = Some(PathBuf::from(value(&mut args, &flag)?)),
            "--morphology" => morphology_out = PathBuf::from(value(&mut args, &flag)?),
            other => return Err(ImportError::Usage(format!("unknown argument {other}"))),
        }
    }
    match command.as_str() {
        "build" => {
            let repository = rights
                .parent()
                .map_or_else(|| PathBuf::from("."), PathBuf::from);
            let repository = if repository.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                repository
            };
            let options = BuildOptions {
                corpora,
                rights,
                out: out.clone(),
            };
            let report = build(&options, &repository)?;
            for e in &report.editions {
                println!(
                    "{}: {} books, {} chapters, {} segments, {} tokens; pack {} bytes, search {} bytes; {} differences from the canon table",
                    e.resource_id,
                    e.books,
                    e.chapters,
                    e.segments,
                    e.tokens,
                    e.pack_bytes,
                    e.search_bytes,
                    e.mismatches.len()
                );
            }
            println!(
                "built in {:.1} s ({:.1} s reading and checking the sources): {}",
                report.total_seconds,
                report.verify_seconds,
                out.display()
            );
            Ok(())
        }
        "bench" => {
            let report = bench::run(&out.join("search"), rounds)?;
            println!("group | samples | p50 ms | p95 ms | max ms | slowest");
            for r in &report.rows {
                println!(
                    "{} | {} | {:.2} | {:.2} | {:.2} | {}",
                    r.group, r.samples, r.p50_ms, r.p95_ms, r.max_ms, r.slowest
                );
            }
            println!("spot hits (limit 50): {:?}", report.spot_hits);
            println!("budget: p95 <= {} ms", report.budget_ms);
            if let Some(path) = json {
                let text = serde_json::to_string_pretty(&report)
                    .map_err(|e| ImportError::Pack(e.to_string()))?;
                std::fs::write(&path, text + "\n").map_err(|error| ImportError::Io {
                    path: path.display().to_string(),
                    error,
                })?;
            }
            Ok(())
        }
        "morphology" => {
            let repository = rights
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map_or_else(|| PathBuf::from("."), PathBuf::from);
            let options = MorphologyOptions {
                corpora,
                rights,
                preview_pack: out,
                out: morphology_out.clone(),
            };
            let report = morphology::build(&options, &repository)?;
            for l in &report.layers {
                let j = &l.join;
                println!(
                    "{} over {}: {} words ({} morphemes, {} implied), {} joined, {} not joined; {} of {} tokens analysed, {} without; {} bytes",
                    l.layer.layer_id,
                    l.layer.edition,
                    j.source_words,
                    j.source_morphemes,
                    j.implied_morphemes,
                    j.words_joined,
                    j.words_not_joined,
                    j.tokens_analyzed,
                    j.pack_tokens,
                    j.tokens_without_analysis,
                    l.bytes
                );
                println!("  word classes: {:?}", j.word_classes);
                println!("  without analysis: {:?}", j.absent_reasons);
            }
            println!(
                "built in {:.1} s ({:.1} s in the rights gate): {}",
                report.total_seconds,
                report.verify_seconds,
                morphology_out.display()
            );
            Ok(())
        }
        "morphology-bench" => {
            let report = morphology::bench::run(&morphology_out.join("layers"), rounds)?;
            println!("loaded in {:.2} s", report.load_seconds);
            println!("group | samples | p50 ms | p95 ms | max ms | slowest");
            for r in &report.rows {
                println!(
                    "{} | {} | {:.3} | {:.3} | {:.3} | {}",
                    r.group, r.samples, r.p50_ms, r.p95_ms, r.max_ms, r.slowest
                );
            }
            for (q, by, total, lemmas) in &report.spot {
                println!("spot {q}: {by}, {total} matches, {lemmas:?}");
            }
            println!("budget: p95 <= {} ms", report.budget_ms);
            if let Some(path) = json {
                let text = serde_json::to_string_pretty(&report)
                    .map_err(|e| ImportError::Pack(e.to_string()))?;
                std::fs::write(&path, text + "\n").map_err(|error| ImportError::Io {
                    path: path.display().to_string(),
                    error,
                })?;
            }
            Ok(())
        }
        _ => Err(ImportError::Usage(
            "usage: openlogos-preview-pack build [--corpora DIR] [--rights DIR] [--out DIR]\n       openlogos-preview-pack bench [--out DIR] [--rounds N] [--json FILE]\n       openlogos-preview-pack morphology [--corpora DIR] [--rights DIR] [--out PREVIEW_PACK] [--morphology DIR]\n       openlogos-preview-pack morphology-bench [--morphology DIR] [--rounds N] [--json FILE]"
                .to_owned(),
        )),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("preview-pack: {error}");
            ExitCode::FAILURE
        }
    }
}
