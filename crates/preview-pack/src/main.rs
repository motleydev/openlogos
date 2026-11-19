//! `openlogos-preview-pack build`: see the library's documentation and docs/evidence/B071.md.

use std::path::PathBuf;
use std::process::ExitCode;

use openlogos_preview_pack::bench;
use openlogos_preview_pack::build::{BuildOptions, build};
use openlogos_preview_pack::model::ImportError;
use openlogos_preview_pack::xrefs;
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
    let mut stride = 20usize;
    let mut limit = 12usize;
    let mut preview = false;
    let mut shown: Vec<String> = Vec::new();
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
            "--stride" | "--limit" => {
                let n = value(&mut args, &flag)?
                    .parse()
                    .map_err(|_| ImportError::Usage(format!("{flag} needs a number")))?;
                if flag == "--stride" {
                    stride = n;
                } else {
                    limit = n;
                }
            }
            "--preview" => preview = true,
            "--morphology" => morphology_out = PathBuf::from(value(&mut args, &flag)?),
            other if command == "morphology-show" && !other.starts_with("--") => {
                shown.push(other.to_owned());
            }
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
        "xrefs" => {
            let repository = rights
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map_or_else(|| PathBuf::from("."), PathBuf::from);
            let report = xrefs::build(
                &xrefs::XrefOptions {
                    corpora,
                    rights,
                    out: out.clone(),
                },
                &repository,
            )?;
            let s = &report.shape;
            println!(
                "{}: {} rows from {} verses, {} distinct targets, {} ranges ({} across chapters, {} across books); scheme {} ({} of {} references not in org)",
                report.source_id,
                s.rows,
                s.source_verses,
                s.distinct_targets,
                s.range_targets,
                s.ranges_across_chapters,
                s.ranges_across_books,
                report.scheme.scheme,
                report.scheme.not_in_org,
                report.scheme.references
            );
            println!(
                "duplicates {}, self links {}, reversed pairs {}, reversed into ranges {}, single inside a range of the same source {}; votes: {} negative, {} zero, {} positive, {}..{}",
                s.duplicate_rows,
                s.self_links,
                s.reversed_pairs,
                s.reversed_into_ranges,
                s.single_inside_range_of_same_source,
                report.votes.negative,
                report.votes.zero,
                report.votes.positive,
                report.votes.min,
                report.votes.max
            );
            for e in &report.editions {
                println!(
                    "{} ({}): targets {:?}; mapping {:?}; renumbered {}",
                    e.resource_id, e.scheme, e.targets, e.mapping, e.renumbered
                );
            }
            println!(
                "pack {} bytes ({} edges, {} range rows) in {:.1} s: {}",
                report.pack.bytes,
                report.pack.edges,
                report.pack.range_rows,
                report.seconds,
                report.pack.path
            );
            Ok(())
        }
        "xrefs-show" => {
            print!("{}", xrefs::show(&out, limit, preview)?);
            Ok(())
        }
        "xrefs-bench" => {
            let rows = xrefs::bench(&out, stride)?;
            println!("group | samples | p50 ms | p95 ms | max ms | slowest");
            for r in &rows {
                println!(
                    "{} | {} | {:.3} | {:.3} | {:.3} | {}",
                    r.group, r.samples, r.p50_ms, r.p95_ms, r.max_ms, r.slowest
                );
            }
            if let Some(path) = json {
                let text = serde_json::to_string_pretty(&rows)
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
        "morphology-show" => {
            // `morphology-show Gen.1.1 Dan.2.4`: every token of each verse with what the
            // layers say, to read against the source by eye.
            let layers = openlogos_retrieval_sqlite::MorphologyLayers::open_directory(
                &morphology_out.join("layers"),
            )
            .map_err(|e| ImportError::Pack(e.to_string()))?;
            for place in &shown {
                // `Book.C.V`, or `Book.C` for every verse of a chapter.
                let mut parts = place.split('.');
                let book = parts
                    .next()
                    .and_then(openlogos_core::reference::BookId::from_osis)
                    .ok_or_else(|| ImportError::Usage(format!("not an OSIS place: {place}")))?;
                let chapter: u16 = parts
                    .next()
                    .and_then(|c| c.parse().ok())
                    .ok_or_else(|| ImportError::Usage(format!("no chapter in {place}")))?;
                let only: Option<u16> = parts.next().and_then(|v| v.parse().ok());
                let edition = if book.number() >= 40 { "sblgnt" } else { "wlc" };
                let pack = morphology::read_text_pack(&out.join("packs").join(edition))?;
                let ids: Vec<openlogos_retrieval::VerseId> = pack
                    .verses
                    .keys()
                    .filter(|v| {
                        v.book() == book.number()
                            && v.chapter() == chapter
                            && only.is_none_or(|o| v.verse() == o)
                    })
                    .copied()
                    .collect();
                for id in ids {
                let verse = format!("{}.{}.{}", book.as_osis(), id.chapter(), id.verse());
                println!("== {verse} ({edition})");
                for (word, text) in pack.verses.get(&id).into_iter().flatten() {
                    let token = format!(
                        "{edition}:{:02}{:03}{:03}{word:03}",
                        id.book(),
                        id.chapter(),
                        id.verse()
                    );
                    let a = layers.analyze(&token).map_err(|e| ImportError::Pack(e.to_string()))?;
                    println!("{token} {text} [{:?}{}]", a.status, a.absent_reason.map(|r| format!(" {r}")).unwrap_or_default());
                    for x in &a.analyses {
                        println!("  {} reading={} join={} text={}", x.layer_id, x.reading.as_str(), x.join, x.text);
                        for m in &x.morphemes {
                            println!(
                                "    {} {:?} {}..{} {} lemma={} strong={} {} {} = {} | {}",
                                m.id.as_deref().unwrap_or("-"),
                                m.surface,
                                m.start.map_or("-".to_owned(), |s| s.to_string()),
                                m.end.map_or("-".to_owned(), |s| s.to_string()),
                                m.language.tag(),
                                m.lemma,
                                m.strong,
                                m.morph_code,
                                m.external_id,
                                m.description,
                                m.gloss
                            );
                        }
                    }
                }
                }
            }
            Ok(())
        }
        "morphology-bench" => {
            let report = morphology::bench::run(&morphology_out.join("layers"), rounds)?;
            let samples = morphology_out.join(morphology::bench::SAMPLES_FILE);
            let text = serde_json::to_string(&report.samples)
                .map_err(|e| ImportError::Pack(e.to_string()))?;
            std::fs::write(&samples, text).map_err(|error| ImportError::Io {
                path: samples.display().to_string(),
                error,
            })?;
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
            "usage: openlogos-preview-pack build [--corpora DIR] [--rights DIR] [--out DIR]\n       openlogos-preview-pack bench [--out DIR] [--rounds N] [--json FILE]\n       openlogos-preview-pack xrefs [--corpora DIR] [--rights DIR] [--out DIR]\n       openlogos-preview-pack xrefs-show [--out DIR] [--limit N] [--preview]\n       openlogos-preview-pack xrefs-bench [--out DIR] [--stride N] [--json FILE]\n       openlogos-preview-pack morphology [--corpora DIR] [--rights DIR] [--out PREVIEW_PACK] [--morphology DIR]\n       openlogos-preview-pack morphology-bench [--morphology DIR] [--rounds N] [--json FILE]\n       openlogos-preview-pack morphology-show OSIS... [--out PREVIEW_PACK] [--morphology DIR]"
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
