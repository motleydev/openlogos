//! Builds the preview pack: three verified sources into text packs and search packs in a
//! temporary directory that replaces the output only when everything passed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use openlogos_core::canon::{self, BookOrder};
use openlogos_core::reference::{BookId, Reference};
use openlogos_core::versification::SchemeId;
use openlogos_retrieval::VerseId;
use openlogos_retrieval_sqlite::{PackBuilder, PackManifest as SearchManifest};
use openlogos_text::reading::{
    DirSource, Edition, FilePack, PackWriter, ReadingLibrary, book_title,
};
use serde::Serialize;

use crate::model::{
    Counters, ImportError, ImportedBook, ImportedChapter, Layout, make_verse, parse_error,
    whitespace_words,
};
use crate::rights::{VerifiedSource, verify_source};
use crate::{oshb, sblgnt, usfm};

/// The CC BY 4.0 deed.
const CC_BY: &str = "https://creativecommons.org/licenses/by/4.0/";

/// Where the three sources live on the NAS, by source id.
pub const CORPORA: [(&str, &str); 3] = [
    ("S52", "S52-bsb"),
    ("S06", "S06-sblgnt"),
    ("S07", "S07-morphhb"),
];

/// The file holding the build's report, at the top of the output.
pub const REPORT_FILE: &str = "preview-pack.json";

/// What a build is told.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// The directory holding `S52-bsb/` and its siblings; only ever read.
    pub corpora: PathBuf,
    /// The repository's `rights/`.
    pub rights: PathBuf,
    /// The output directory.
    pub out: PathBuf,
}

/// A verse-count difference between an edition and the canon table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mismatch {
    /// `absent`: the table has the verse, the edition does not. `extra`: the reverse.
    /// `title_absent` / `title_extra`: the same for a Psalm's verse 0.
    pub kind: String,
    /// `Book.C.V`, or `Book.C` for a whole chapter.
    pub segment: String,
}

/// What one edition's build produced.
#[derive(Debug, Clone, Serialize)]
pub struct EditionReport {
    /// `bsb`.
    pub resource_id: String,
    /// The source.
    pub source_id: String,
    /// The edition id of the rights record.
    pub edition_id: String,
    /// The revision in the manifest.
    pub content_revision: String,
    /// SHA-256 of the source's `MANIFEST.sha256`.
    pub source_manifest_sha256: String,
    /// Files whose hash was checked.
    pub source_files_checked: usize,
    /// SHA-256 of the pack's `manifest.json`, what a catalog records.
    pub pack_manifest_sha256: String,
    /// Books, chapters, segments and tokens written.
    pub books: usize,
    /// Chapters.
    pub chapters: usize,
    /// Segments (a verse, or a Psalm title in `eng`).
    pub segments: usize,
    /// Tokens.
    pub tokens: usize,
    /// Bytes under `packs/<id>/`.
    pub pack_bytes: u64,
    /// Bytes of its search pack.
    pub search_bytes: u64,
    /// Differences from the canon table.
    pub mismatches: Vec<Mismatch>,
    /// What the parser met and left out, or changed.
    pub counters: Counters,
}

/// The build report, `preview-pack.json`.
#[derive(Debug, Clone, Serialize)]
pub struct BuildReport {
    /// The command that makes this.
    pub built_by: String,
    /// Seconds spent reading and checking the sources.
    pub verify_seconds: f64,
    /// Seconds for the whole build.
    pub total_seconds: f64,
    /// Per edition.
    pub editions: Vec<EditionReport>,
}

fn io(path: &Path, error: std::io::Error) -> ImportError {
    ImportError::Io {
        path: path.display().to_string(),
        error,
    }
}

fn utf8<'a>(source: &'a VerifiedSource, path: &str) -> Result<&'a str, ImportError> {
    let bytes = source
        .files
        .get(path)
        .ok_or_else(|| parse_error(path, "not in the verified source"))?;
    std::str::from_utf8(bytes).map_err(|e| parse_error(path, e.to_string()))
}

// --- BSB

/// The entries of a zip, through the OS's `unzip` (scripts/wasm-test.sh already relies on it):
/// the zip is the artifact the manifest hashes, and a new dependency to read it once is not worth
/// having. The bytes come from the checked copy, not the NAS.
fn unzip_entry(zip: &Path, entry: &str) -> Result<String, ImportError> {
    let output = std::process::Command::new("unzip")
        .arg("-p")
        .arg(zip)
        .arg(entry)
        .output()
        .map_err(|e| io(Path::new("unzip"), e))?;
    if !output.status.success() {
        return Err(parse_error(
            entry,
            format!("unzip: {}", String::from_utf8_lossy(&output.stderr)),
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| parse_error(entry, e.to_string()))
}

fn read_bsb(
    source: &VerifiedSource,
    work: &Path,
) -> Result<(Vec<ImportedBook>, Counters), ImportError> {
    let zip_bytes = source
        .files
        .get("bsb_usfm.zip")
        .ok_or_else(|| parse_error("bsb_usfm.zip", "not in the verified source"))?;
    let zip = work.join("bsb_usfm.zip");
    std::fs::write(&zip, zip_bytes).map_err(|e| io(&zip, e))?;
    let mut counters = Counters::new();
    let mut books = Vec::new();
    for (book, code) in BookId::all().into_iter().zip(usfm::USFM_CODES) {
        let entry = format!("bsb_usfm/{code}.usfm");
        let text = unzip_entry(&zip, &entry)?;
        let chapters = usfm::parse_book(&entry, &text, "bsb", book, &mut counters)?;
        books.push(ImportedBook {
            book,
            title: book_title(book).to_owned(),
            chapters,
        });
    }
    std::fs::remove_file(&zip).map_err(|e| io(&zip, e))?;
    take_text_from_text_file(&mut books, utf8(source, "bsb.txt")?, &mut counters)?;
    Ok((books, counters))
}

/// Where the BSB's two forms differ in their letters, each read against the source: the text
/// file is right and the USFM is not. Anything else stops the build.
const USFM_LETTER_DIFFERENCES: [(&str, &str); 2] = [
    (
        "Gen.35.18",
        "the USFM has a stray `vvv` before `him Ben-oni`",
    ),
    (
        "Hab.3.19",
        "the USFM has `For the choirmaster. With stringed instruments.` as a heading after the \\
         verse; the text file prints it as the verse's end, as the Hebrew does",
    ),
];

/// The letters and digits of a text, for comparing the BSB's two forms.
fn skeleton(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Where a Psalm title ends in the text file's line: after as many letters as the USFM's title
/// has, which must be the same letters. `None` when they are not.
fn title_end_in_line(title: &str, line: &str) -> Option<usize> {
    let wanted = skeleton(title);
    let mut matched = wanted.chars();
    let mut next = matched.next();
    for (at, c) in line.char_indices() {
        if !c.is_alphanumeric() {
            continue;
        }
        if next != Some(c) {
            return None;
        }
        next = matched.next();
        if next.is_none() {
            // The title runs to the next space (closing punctuation stays with it).
            let after = at + c.len_utf8();
            return Some(line[after..].find(' ').map_or(line.len(), |s| after + s));
        }
    }
    None
}

/// The verse lines of `bsb.txt`, by book position, chapter and verse. An empty line is `""`.
fn text_file_lines(oracle: &str) -> Result<BTreeMap<(usize, u32, u32), &str>, ImportError> {
    let mut names: Vec<&str> = Vec::new();
    let mut lines = BTreeMap::new();
    for line in oracle.trim_start_matches('\u{feff}').lines() {
        let line = line.trim_end_matches('\r');
        let Some((id, text)) = line.split_once('\t') else {
            continue;
        };
        let place = id.rsplit_once(' ').and_then(|(name, place)| {
            let (c, v) = place.split_once(':')?;
            Some((name, c.parse::<u32>().ok()?, v.parse::<u32>().ok()?))
        });
        // The header's lines do not end in a chapter and verse.
        let Some((name, chapter, verse)) = place else {
            continue;
        };
        if names.last() != Some(&name) {
            names.push(name);
        }
        lines.insert((names.len() - 1, chapter, verse), text);
    }
    if names.len() != BookId::all().len() {
        return Err(ImportError::Oracle(format!(
            "{} books in bsb.txt",
            names.len()
        )));
    }
    Ok(lines)
}

/// Replaces the USFM's verse text with the publisher's own text file, verse for verse.
///
/// The BSB's USFM is generated (`Autogenerated BSB by bsb2usfm`) and its running text carries
/// residue the publisher's `bsb.txt` does not: spaces around em dashes (`morning — the first
/// day`), `Likewise ,`, `1 ,700`, `[’’]` and `[of silver]`, column dots (`numbered . . . 40,500`),
/// a stray `vvv`. So the USFM supplies the structure (which verses exist, paragraphs, poetry,
/// where a Psalm's title ends) and `bsb.txt` the bytes. The two must agree letter for letter,
/// except for [`USFM_LETTER_DIFFERENCES`], and an empty line must be a verse the USFM lacks.
/// Any other difference stops the build.
fn take_text_from_text_file(
    books: &mut [ImportedBook],
    oracle: &str,
    counters: &mut Counters,
) -> Result<(), ImportError> {
    let mut lines = text_file_lines(oracle)?;
    let mut differences = Vec::new();
    let mut usfm_residue = 0u64;
    let mut known = 0u64;
    let mut taken = 0u64;
    for (b, book) in books.iter_mut().enumerate() {
        let id = book.book;
        for chapter in &mut book.chapters {
            let number = chapter.chapter;
            let mut title_rest: Option<String> = None;
            for verse in &mut chapter.verses {
                let segment = verse.segment_id.clone();
                let bytes: String = if verse.number == 0 {
                    let line = lines.get(&(b, number, 1)).copied().unwrap_or("");
                    let Some(end) = title_end_in_line(&verse.text, line) else {
                        differences.push(format!("{segment}: the title is not the line's start"));
                        continue;
                    };
                    title_rest = Some(line[end..].trim_start().to_owned());
                    line[..end].to_owned()
                } else if let Some(rest) = title_rest.take() {
                    lines.remove(&(b, number, verse.number));
                    rest
                } else {
                    match lines.remove(&(b, number, verse.number)) {
                        None | Some("") => {
                            differences.push(format!("{segment}: in the USFM, not in bsb.txt"));
                            continue;
                        }
                        Some(line) => line.to_owned(),
                    }
                };
                if bytes != bytes.trim() || bytes.contains("  ") || bytes.is_empty() {
                    differences.push(format!("{segment}: unexpected spacing: [{bytes}]"));
                }
                if skeleton(&bytes) != skeleton(&verse.text) {
                    if USFM_LETTER_DIFFERENCES.iter().any(|(s, _)| *s == segment) {
                        known += 1;
                    } else {
                        differences
                            .push(format!("{segment}:\n  usfm {}\n  text {bytes}", verse.text));
                    }
                }
                if bytes != verse.text {
                    usfm_residue += 1;
                }
                let layout = Layout {
                    paragraph_start: verse.paragraph_start,
                    poetry: verse.poetry,
                    superscription: verse.superscription,
                };
                let words = whitespace_words(&bytes);
                *verse = make_verse("bsb", id, number, verse.number, bytes, &words, layout)?;
                taken += 1;
            }
        }
    }
    let mut empty = 0u64;
    for ((b, chapter, verse), line) in &lines {
        if line.is_empty() {
            empty += 1;
        } else {
            differences.push(format!(
                "{} {chapter}:{verse}: in bsb.txt, not in the USFM",
                books[*b].book
            ));
        }
    }
    if !differences.is_empty() {
        return Err(ImportError::Oracle(format!(
            "{} differences between the BSB USFM and bsb.txt; the first:\n{}",
            differences.len(),
            differences
                .iter()
                .take(40)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        )));
    }
    counters.insert("bsb_txt_segments_taken".to_owned(), taken);
    counters.insert("bsb_txt_empty_lines".to_owned(), empty);
    counters.insert(
        "usfm_residue_replaced_by_text_file".to_owned(),
        usfm_residue,
    );
    counters.insert("usfm_known_letter_differences".to_owned(), known);
    Ok(())
}

// --- SBLGNT

fn read_sblgnt(source: &VerifiedSource) -> Result<(Vec<ImportedBook>, Counters), ImportError> {
    let mut counters = Counters::new();
    let mut books = Vec::new();
    for book in canon::new_testament() {
        let osis = book.as_osis();
        let text_path = format!("data/sblgnt/text/{osis}.txt");
        let xml_path = format!("data/sblgnt/xml/{osis}.xml");
        let (title, chapters) = sblgnt::parse_book(
            &text_path,
            utf8(source, &text_path)?,
            utf8(source, &xml_path)?,
            "sblgnt",
            book,
            &mut counters,
        )?;
        books.push(ImportedBook {
            book,
            title,
            chapters,
        });
    }
    Ok((books, counters))
}

// --- WLC

fn read_wlc(source: &VerifiedSource) -> Result<(Vec<ImportedBook>, Counters), ImportError> {
    let mut counters = Counters::new();
    let mut books = Vec::new();
    for book in BookOrder::TanakhBhs.arrange(&canon::old_testament()) {
        let path = format!("wlc/{}.xml", book.as_osis());
        let chapters = oshb::parse_book(&path, utf8(source, &path)?, "wlc", book, &mut counters)?;
        let title = oshb::hebrew_title(book)
            .ok_or_else(|| parse_error(&path, "no Hebrew title"))?
            .to_owned();
        books.push(ImportedBook {
            book,
            title,
            chapters,
        });
    }
    Ok((books, counters))
}

// --- Reconciling with the canon table

/// Every difference between an edition's verses and the canon table.
#[must_use]
pub fn reconcile(scheme: &str, books: &[ImportedBook]) -> Vec<Mismatch> {
    let scheme = SchemeId::new(scheme);
    let mut out = Vec::new();
    let mut push = |kind: &str, segment: String| {
        out.push(Mismatch {
            kind: kind.to_owned(),
            segment,
        });
    };
    for book in books {
        let id = book.book;
        let table_chapters = canon::chapter_count(&scheme, id).unwrap_or(0);
        for number in 1..=table_chapters {
            let held = book.chapters.iter().find(|c| c.chapter == number);
            let Some(held) = held else {
                push("absent", format!("{id}.{number}"));
                continue;
            };
            let first = canon::first_verse(&scheme, id, number).unwrap_or(1);
            let last = canon::last_verse(&scheme, id, number).unwrap_or(0);
            let has = |v: u32| held.verses.iter().any(|h| h.number == v);
            if first == 0 && !has(0) {
                push("title_absent", Reference::verse(id, number, 0).to_osis());
            }
            for v in 1..=last {
                if !has(v) {
                    push("absent", Reference::verse(id, number, v).to_osis());
                }
            }
            for verse in &held.verses {
                if verse.number == 0 && first != 0 {
                    push("title_extra", verse.segment_id.clone());
                } else if verse.number > last {
                    push("extra", verse.segment_id.clone());
                }
            }
        }
        for chapter in &book.chapters {
            if chapter.chapter > table_chapters {
                push("extra", format!("{id}.{}", chapter.chapter));
            }
        }
    }
    out
}

// --- Writing

fn size_of(path: &Path) -> Result<u64, ImportError> {
    let meta = std::fs::metadata(path).map_err(|e| io(path, e))?;
    if meta.is_file() {
        return Ok(meta.len());
    }
    let mut total = 0;
    for entry in std::fs::read_dir(path).map_err(|e| io(path, e))? {
        total += size_of(&entry.map_err(|e| io(path, e))?.path())?;
    }
    Ok(total)
}

struct EditionSpec {
    edition: Edition,
    license: String,
    source_line: String,
}

fn commit_short(source: &VerifiedSource) -> String {
    source
        .commit
        .as_deref()
        .map_or_else(String::new, |c| c.chars().take(7).collect())
}

fn bsb_spec(source: &VerifiedSource) -> EditionSpec {
    EditionSpec {
        edition: Edition {
            resource_id: "bsb".to_owned(),
            // The USFM export's date and the checked manifest: `bsb.txt` carries no date.
            content_revision: format!(
                "bsb-2026-03-08-{}",
                source.manifest_sha256.chars().take(7).collect::<String>()
            ),
            title: "BSB".to_owned(),
            name: "Berean Standard Bible".to_owned(),
            language: "en".to_owned(),
            direction: "ltr".to_owned(),
            scheme: "eng".to_owned(),
            book_order: BookOrder::Protestant,
            placeholder: false,
            attribution: Some(source.attribution.clone()),
        },
        license: format!("{}, https://berean.bible/terms.htm", source.license),
        source_line: format!(
            "Verse text from https://bereanbible.com/bsb.txt, structure from {} ({}), MANIFEST.sha256 {}",
            source.upstream_url, source.edition_id, source.manifest_sha256
        ),
    }
}

fn sblgnt_spec(source: &VerifiedSource) -> EditionSpec {
    let attribution = format!(
        "{} Licence: {CC_BY}. Source: {} at commit {}. Changed for this edition: the apparatus \
         signs {} are removed from the running text; the text is otherwise as published. The \
         licensor offers the work as-is and makes no warranties (CC BY 4.0 §5).",
        source.attribution,
        source.upstream_url,
        source.commit.as_deref().unwrap_or("unknown"),
        sblgnt::APPARATUS_SIGNS
            .iter()
            .map(char::to_string)
            .collect::<Vec<_>>()
            .join(" "),
    );
    EditionSpec {
        edition: Edition {
            resource_id: "sblgnt".to_owned(),
            content_revision: format!("sblgnt-1.2-{}", commit_short(source)),
            title: "SBLGNT".to_owned(),
            name: "SBL Greek New Testament".to_owned(),
            language: "grc".to_owned(),
            direction: "ltr".to_owned(),
            scheme: "org".to_owned(),
            book_order: BookOrder::Protestant,
            placeholder: false,
            attribution: Some(attribution),
        },
        license: format!("{}, {CC_BY}", source.license),
        source_line: format!(
            "{} ({}), MANIFEST.sha256 {}",
            source.upstream_url, source.edition_id, source.manifest_sha256
        ),
    }
}

fn wlc_spec(source: &VerifiedSource) -> EditionSpec {
    let attribution = format!(
        "{} Licence of the OSHB layer: {CC_BY}. Changed for this edition: OSHB's morpheme \
         separators and notes are not carried, and the qere readings are not in the running text.",
        source.attribution,
    );
    EditionSpec {
        edition: Edition {
            resource_id: "wlc".to_owned(),
            content_revision: format!("oshb-wlc-4.20-{}", commit_short(source)),
            title: "WLC".to_owned(),
            name: "Westminster Leningrad Codex".to_owned(),
            language: "hbo".to_owned(),
            direction: "rtl".to_owned(),
            scheme: "org".to_owned(),
            book_order: BookOrder::TanakhBhs,
            placeholder: false,
            attribution: Some(attribution),
        },
        license: format!(
            "{}: the text of the Westminster Leningrad Codex is public domain, the OSHB layer is CC BY 4.0, {CC_BY}",
            source.license
        ),
        source_line: format!(
            "{} ({}), MANIFEST.sha256 {}",
            source.upstream_url, source.edition_id, source.manifest_sha256
        ),
    }
}

/// Writes one edition's text pack and reads every chapter back through the reader.
fn write_text_pack(
    spec: &EditionSpec,
    books: &[ImportedBook],
    directory: &Path,
) -> Result<String, ImportError> {
    let pack_error = |e: &dyn std::fmt::Display| ImportError::Pack(e.to_string());
    let mut writer =
        PackWriter::new(&spec.edition, &spec.license, &spec.source_line, false).complete(true);
    for book in books {
        for ImportedChapter { chapter, verses } in &book.chapters {
            writer.chapter(book.book, &book.title, *chapter, verses.clone());
        }
    }
    let written = writer.finish().map_err(|e| pack_error(&e))?;
    std::fs::create_dir_all(directory).map_err(|e| io(directory, e))?;
    let manifest_path = directory.join("manifest.json");
    std::fs::write(&manifest_path, &written.manifest_json).map_err(|e| io(&manifest_path, e))?;
    for (path, bytes) in &written.files {
        let full = directory.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
        }
        std::fs::write(&full, bytes).map_err(|e| io(&full, e))?;
    }
    let pack = FilePack::from_manifest(
        &written.manifest_json,
        Some(Box::new(DirSource::new(directory))),
    )
    .map_err(|e| pack_error(&e))?;
    let mut library = ReadingLibrary::new();
    library.insert(Box::new(pack));
    for book in books {
        for chapter in &book.chapters {
            let key = format!("{}.{}", book.book, chapter.chapter);
            let block = library
                .chapter_content(&spec.edition.resource_id, &key)
                .map_err(|e| pack_error(&e))?;
            if block.verses != chapter.verses {
                return Err(ImportError::Pack(format!(
                    "{key} does not read back as it was written"
                )));
            }
        }
    }
    Ok(openlogos_text::reading::pack::sha256_hex(
        written.manifest_json.as_bytes(),
    ))
}

/// Writes one edition's FTS5 search pack with the B064 writer and the shared tokenizer.
fn write_search_pack(
    spec: &EditionSpec,
    books: &[ImportedBook],
    path: &Path,
) -> Result<(), ImportError> {
    let sql = |e: rusqlite::Error| ImportError::Pack(e.to_string());
    let manifest = SearchManifest {
        edition: spec.edition.resource_id.clone(),
        scheme: spec.edition.scheme.clone(),
        source: spec.source_line.clone(),
        license: spec.license.clone(),
        attribution: spec.edition.attribution.clone().unwrap_or_default(),
        work: None,
        locator_scheme: None,
    };
    let mut builder = PackBuilder::create(path, &manifest, &[]).map_err(sql)?;
    for book in books {
        for chapter in &book.chapters {
            let chapter_number = u16::try_from(chapter.chapter)
                .map_err(|_| parse_error(path.to_str().unwrap_or(""), "chapter number"))?;
            for verse in &chapter.verses {
                let number = u16::try_from(verse.number)
                    .map_err(|_| parse_error(path.to_str().unwrap_or(""), "verse number"))?;
                builder
                    .add_verse(
                        VerseId::new(book.book.number(), chapter_number, number),
                        &verse.text,
                    )
                    .map_err(sql)?;
            }
        }
    }
    builder.finish().map_err(sql)
}

fn refuse_inside(out: &Path, parent: &Path, what: &str) -> Result<(), ImportError> {
    if out.starts_with(parent) {
        return Err(ImportError::Usage(format!(
            "the output {} is inside {what}; a preview pack never goes there",
            out.display()
        )));
    }
    Ok(())
}

fn absolute(path: &Path) -> Result<PathBuf, ImportError> {
    std::path::absolute(path).map_err(|e| io(path, e))
}

/// The whole build. `repository` is refused as an output, and so are the corpora.
pub fn build(options: &BuildOptions, repository: &Path) -> Result<BuildReport, ImportError> {
    let started = Instant::now();
    let out = absolute(&options.out)?;
    refuse_inside(&out, &absolute(repository)?, "the repository")?;
    refuse_inside(&out, &absolute(&options.corpora)?, "the corpora")?;
    let work = out.with_extension("building");
    if work.exists() {
        std::fs::remove_dir_all(&work).map_err(|e| io(&work, e))?;
    }
    std::fs::create_dir_all(work.join("packs")).map_err(|e| io(&work, e))?;
    std::fs::create_dir_all(work.join("search")).map_err(|e| io(&work, e))?;

    let dir = |source: &str| {
        let name = CORPORA
            .iter()
            .find(|(id, _)| *id == source)
            .map_or("", |(_, name)| *name);
        options.corpora.join(name)
    };
    let bsb = verify_source("S52", &options.rights, &dir("S52"), |p| {
        p == "bsb_usfm.zip" || p == "bsb.txt"
    })?;
    let sblgnt_source = verify_source("S06", &options.rights, &dir("S06"), |p| {
        p.starts_with("data/sblgnt/text/") || p.starts_with("data/sblgnt/xml/")
    })?;
    let wlc = verify_source("S07", &options.rights, &dir("S07"), |p| {
        p.starts_with("wlc/") && p != "wlc/VerseMap.xml"
    })?;
    let verify_seconds = started.elapsed().as_secs_f64();

    let mut editions = Vec::new();
    let inputs: Vec<(EditionSpec, Vec<ImportedBook>, Counters, &VerifiedSource)> = vec![
        {
            let (books, counters) = read_bsb(&bsb, &work)?;
            (bsb_spec(&bsb), books, counters, &bsb)
        },
        {
            let (books, counters) = read_sblgnt(&sblgnt_source)?;
            (sblgnt_spec(&sblgnt_source), books, counters, &sblgnt_source)
        },
        {
            let (books, counters) = read_wlc(&wlc)?;
            (wlc_spec(&wlc), books, counters, &wlc)
        },
    ];
    for (spec, books, counters, source) in inputs {
        let id = spec.edition.resource_id.clone();
        let pack_dir = work.join("packs").join(&id);
        let manifest_hash = write_text_pack(&spec, &books, &pack_dir)?;
        let search_path = work.join("search").join(format!("{id}.sqlite"));
        write_search_pack(&spec, &books, &search_path)?;
        editions.push(EditionReport {
            resource_id: id,
            source_id: source.source_id.clone(),
            edition_id: source.edition_id.clone(),
            content_revision: spec.edition.content_revision.clone(),
            source_manifest_sha256: source.manifest_sha256.clone(),
            source_files_checked: source.files_checked,
            pack_manifest_sha256: manifest_hash,
            books: books.len(),
            chapters: books.iter().map(|b| b.chapters.len()).sum(),
            segments: books
                .iter()
                .flat_map(|b| &b.chapters)
                .map(|c| c.verses.len())
                .sum(),
            tokens: books
                .iter()
                .flat_map(|b| &b.chapters)
                .flat_map(|c| &c.verses)
                .map(|v| v.tokens.len())
                .sum(),
            pack_bytes: size_of(&pack_dir)?,
            search_bytes: size_of(&search_path)?,
            mismatches: reconcile(&spec.edition.scheme, &books),
            counters,
        });
    }
    let report = BuildReport {
        built_by: "just preview-pack".to_owned(),
        verify_seconds,
        total_seconds: started.elapsed().as_secs_f64(),
        editions,
    };
    let json =
        serde_json::to_string_pretty(&report).map_err(|e| ImportError::Pack(e.to_string()))?;
    let report_path = work.join(REPORT_FILE);
    std::fs::write(&report_path, json + "\n").map_err(|e| io(&report_path, e))?;
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|e| io(&out, e))?;
    }
    std::fs::rename(&work, &out).map_err(|e| io(&out, e))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Layout;
    use crate::model::make_verse;

    fn chapter(chapter: u32, book: BookId, numbers: &[u32]) -> ImportedChapter {
        ImportedChapter {
            chapter,
            verses: numbers
                .iter()
                .map(|n| {
                    make_verse(
                        "t",
                        book,
                        chapter,
                        *n,
                        "word".to_owned(),
                        &[(0, 4)],
                        Layout::default(),
                    )
                    .unwrap()
                })
                .collect(),
        }
    }

    #[test]
    fn reconciling_names_each_missing_and_extra_verse() {
        // The table has Philemon 1:1–25 in both schemes.
        let numbers: Vec<u32> = (1..=25).filter(|n| *n != 7).chain([26]).collect();
        let book = ImportedBook {
            book: BookId::Phlm,
            title: "Philemon".to_owned(),
            chapters: vec![chapter(1, BookId::Phlm, &numbers)],
        };
        let found = reconcile("org", &[book]);
        assert_eq!(
            found,
            vec![
                Mismatch {
                    kind: "absent".to_owned(),
                    segment: "Phlm.1.7".to_owned()
                },
                Mismatch {
                    kind: "extra".to_owned(),
                    segment: "Phlm.1.26".to_owned()
                },
            ]
        );
    }

    fn sample_books(usfm_text: &[(u32, &str)], book: BookId) -> Vec<ImportedBook> {
        // The oracle counts 66 books; only the first is filled in.
        let numbers: Vec<u32> = usfm_text.iter().map(|(n, _)| *n).collect();
        let mut built = chapter(1, book, &numbers);
        for (verse, (_, text)) in built.verses.iter_mut().zip(usfm_text) {
            verse.text = (*text).to_owned();
            verse.superscription = verse.number == 0;
        }
        vec![ImportedBook {
            book,
            title: "Test".to_owned(),
            chapters: vec![built],
        }]
    }

    /// A text file with 66 book names; the other 65 books have one empty verse.
    fn text_file(first: &str) -> String {
        let mut out = String::from("\u{feff}A header line.\t\nVerse\tBible\n");
        out.push_str(first);
        for i in 0..65 {
            out.push_str(&format!("Other{i} 1:1\t\n"));
        }
        out
    }

    #[test]
    fn the_text_file_supplies_the_bytes_when_the_letters_agree() {
        let mut books = sample_books(&[(1, "One — two ,"), (2, "Three.")], BookId::Gen);
        let mut counters = Counters::new();
        let file = text_file("Test 1:1\tOne—two,\r\nTest 1:2\tThree.\nTest 1:3\t\n");
        take_text_from_text_file(&mut books, &file, &mut counters).unwrap();
        assert_eq!(books[0].chapters[0].verses[0].text, "One—two,");
        assert_eq!(books[0].chapters[0].verses[0].tokens.len(), 1);
        assert_eq!(counters["bsb_txt_segments_taken"], 2);
        assert_eq!(counters["usfm_residue_replaced_by_text_file"], 1);
        assert_eq!(counters["bsb_txt_empty_lines"], 66);
    }

    #[test]
    fn a_letter_difference_or_a_missing_verse_stops_the_build() {
        let mut counters = Counters::new();
        let mut books = sample_books(&[(1, "One two")], BookId::Gen);
        let file = text_file("Test 1:1\tOne too\n");
        let error = take_text_from_text_file(&mut books, &file, &mut counters).unwrap_err();
        assert!(error.to_string().contains("usfm One two"), "{error}");
        let mut books = sample_books(&[(1, "One")], BookId::Gen);
        let file = text_file("Test 1:1\tOne\nTest 1:2\tTwo\n");
        assert!(take_text_from_text_file(&mut books, &file, &mut counters).is_err());
        let mut books = sample_books(&[(1, "One"), (2, "Two")], BookId::Gen);
        let file = text_file("Test 1:1\tOne\nTest 1:2\t\n");
        assert!(take_text_from_text_file(&mut books, &file, &mut counters).is_err());
    }

    #[test]
    fn a_title_is_split_from_verse_one_at_the_end_of_its_letters() {
        let mut books = sample_books(
            &[(0, "A Psalm — of David."), (1, "Lord, how many!")],
            BookId::Ps,
        );
        let mut counters = Counters::new();
        let file = text_file("Psalm 1:1\tA Psalm—of David. Lord, how many!\n");
        take_text_from_text_file(&mut books, &file, &mut counters).unwrap();
        let verses = &books[0].chapters[0].verses;
        assert_eq!(verses[0].text, "A Psalm—of David.");
        assert!(verses[0].superscription);
        assert_eq!(verses[0].label, "");
        assert_eq!(verses[1].text, "Lord, how many!");
    }

    #[test]
    fn the_output_may_not_be_in_the_repository() {
        let repo = Path::new("/r");
        assert!(refuse_inside(Path::new("/r/x"), repo, "the repository").is_err());
        assert!(refuse_inside(Path::new("/s/x"), repo, "the repository").is_ok());
    }
}
