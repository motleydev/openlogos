//! `openlogos-institutes-pack build | bench | show`: the B016 importer (see
//! `openlogos_preview_pack::institutes` and docs/evidence/B016.md).

use std::path::PathBuf;
use std::process::ExitCode;

use openlogos_preview_pack::institutes::build::{
    InstitutesOptions, RESOURCE_ID, bench, build, show,
};
use openlogos_preview_pack::model::ImportError;

const DEFAULT_CORPORA: &str = "/Volumes/office-hub/openlogos/corpora";

/// The places read by eye in docs/evidence/B016.md.
const SPREAD: [&str; 12] = [
    "dedication",
    "1.1.1",
    "1.13.1",
    "1.13.7",
    "1.13.29",
    "3.2.7",
    "3.21.1",
    "3.21.7",
    "4.17.1",
    "4.17.50",
    "4.20.32",
    "2.8.13",
];

fn default_out() -> PathBuf {
    std::env::var_os("OPENLOGOS_WORKS").map_or_else(
        || {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".cache/openlogos/works")
        },
        PathBuf::from,
    )
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, ImportError> {
    args.next()
        .ok_or_else(|| ImportError::Usage(format!("{flag} needs a value")))
}

fn write_json(path: Option<PathBuf>, value: &impl serde::Serialize) -> Result<(), ImportError> {
    if let Some(path) = path {
        let text =
            serde_json::to_string_pretty(value).map_err(|e| ImportError::Pack(e.to_string()))?;
        std::fs::write(&path, text + "\n").map_err(|error| ImportError::Io {
            path: path.display().to_string(),
            error,
        })?;
    }
    Ok(())
}

fn run() -> Result<(), ImportError> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let mut corpora = PathBuf::from(DEFAULT_CORPORA);
    let mut rights = PathBuf::from("rights");
    let mut out = default_out();
    let mut rounds = 3usize;
    let mut json: Option<PathBuf> = None;
    let mut places: Vec<String> = Vec::new();
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
            other if command == "show" && !other.starts_with("--") => places.push(other.to_owned()),
            other => return Err(ImportError::Usage(format!("unknown argument {other}"))),
        }
    }
    let pack = out.join(format!("{RESOURCE_ID}.sqlite"));
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
            let report = build(
                &InstitutesOptions {
                    corpora,
                    rights,
                    out,
                },
                &repository,
            )?;
            let c = &report.citations;
            println!(
                "{}: {} sections ({:?} by division), {} notes ({} + {}), {} findings",
                RESOURCE_ID,
                report.chapters.iter().map(|c| c.sections).sum::<usize>(),
                report.sections_by_division,
                report.notes_per_volume[0] + report.notes_per_volume[1],
                report.notes_per_volume[0],
                report.notes_per_volume[1],
                report.findings.len()
            );
            println!(
                "citations: {} found, {} resolved ({:.1} %; {:.1} % of those naming a book), {} ranges linked; unresolved {:?}",
                c.total,
                c.resolved,
                c.rate * 100.0,
                c.rate_named_books * 100.0,
                c.ranges,
                c.unresolved
            );
            println!(
                "pack {} bytes, sha256 {}; built in {:.1} s ({:.1} s reading and checking the sources): {}",
                report.pack_bytes,
                report.pack_sha256,
                report.total_seconds,
                report.verify_seconds,
                report.pack
            );
            write_json(json, &report)
        }
        "bench" => {
            let report = bench(&pack, rounds)?;
            println!("group | samples | p50 ms | p95 ms | max ms | slowest");
            for r in &report.rows {
                println!(
                    "{} | {} | {:.3} | {:.3} | {:.3} | {}",
                    r.group, r.samples, r.p50_ms, r.p95_ms, r.max_ms, r.slowest
                );
            }
            println!("spot hits (limit 50): {:?}", report.spot_hits);
            println!(
                "budgets: chapter <= {} ms, search p95 <= {} ms",
                report.budgets_ms.0, report.budgets_ms.1
            );
            write_json(json, &report)
        }
        "show" => {
            if places.is_empty() {
                places = SPREAD.iter().map(|p| (*p).to_owned()).collect();
            }
            print!("{}", show(&pack, &places)?);
            Ok(())
        }
        _ => Err(ImportError::Usage(
            "usage: openlogos-institutes-pack build [--corpora DIR] [--rights DIR] [--out DIR] [--json FILE]\n       openlogos-institutes-pack bench [--out DIR] [--rounds N] [--json FILE]\n       openlogos-institutes-pack show [--out DIR] [LOCATOR ...]"
                .to_owned(),
        )),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("institutes-pack: {error}");
            ExitCode::FAILURE
        }
    }
}
