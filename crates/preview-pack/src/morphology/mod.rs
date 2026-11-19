//! B026: the morphology layers over the preview pack's SBLGNT and WLC (docs/evidence/B026.md).
//!
//! `just preview-pack-morphology` runs [`build`]: MACULA Greek and Hebrew (S47) through the
//! B071 rights gate (status, permissions, every hash), their restricted columns checked
//! against the records' flags ([`restricted`]), the TSVs read through an allowlist
//! ([`macula`]), the OSHB markup (S07) read for its ketiv and qere ([`ketiv`]), each word
//! joined to a token of the built text packs ([`join`]), and one layer per edition written
//! with the retrieval engine's [`LayerWriter`]. The output is a separately versioned overlay
//! outside the repository, beside (never inside) the preview pack, which a rebuild of the
//! text packs replaces whole:
//!
//! ```text
//! ~/.cache/openlogos/morphology/
//!   morphology.json              the report: counts, classes, sizes, timings
//!   joins/sblgnt.json, wlc.json  every place of every class but `exact`
//!   layers/sblgnt.macula-greek.sqlite, wlc.macula-hebrew.sqlite
//! ```
//!
//! A layer names the text pack it was joined to (`text_content_revision` and the manifest's
//! SHA-256); a text pack rebuilt from other bytes needs the layers rebuilt.

pub mod bench;
pub mod join;
pub mod ketiv;
pub mod macula;
pub mod restricted;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use openlogos_core::canon::BookOrder;
use openlogos_retrieval::VerseId;
use openlogos_retrieval_sqlite::morphology::{LayerInfo, LayerWriter};
use openlogos_text::reading::pack::{PackManifest, sha256_hex};
use openlogos_text::reading::{DirSource, FilePack, ReadingLibrary};
use serde::Serialize;
use serde_json::Value;

use self::join::{JoinReport, Joined, PackVerses};
use self::restricted::SourceColumns;
use crate::model::ImportError;
use crate::rights::{VerifiedSource, verify_source};

/// The CC BY 4.0 deed.
const CC_BY: &str = "https://creativecommons.org/licenses/by/4.0/";

/// The report at the top of the output.
pub const REPORT_FILE: &str = "morphology.json";

/// What a build is told.
#[derive(Debug, Clone)]
pub struct MorphologyOptions {
    /// The directory holding `S47-macula-greek/` and its siblings; only read.
    pub corpora: PathBuf,
    /// The repository's `rights/`.
    pub rights: PathBuf,
    /// The built preview pack (its `packs/` holds the text packs); only read.
    pub preview_pack: PathBuf,
    /// The output directory.
    pub out: PathBuf,
}

/// One layer's line in the report.
#[derive(Debug, Clone, Serialize)]
pub struct LayerReport {
    /// The layer's manifest.
    pub layer: LayerInfo,
    /// The rights record.
    pub source_id: String,
    /// SHA-256 of the source's `MANIFEST.sha256`.
    pub source_manifest_sha256: String,
    /// Files whose hash was checked.
    pub source_files_checked: usize,
    /// The file, under `layers/`.
    pub file: String,
    /// Its size.
    pub bytes: u64,
    /// Seconds to read, join and write it (after the gate).
    pub seconds: f64,
    /// The join's counts; the places are in `joins/<edition>.json`.
    pub join: JoinReport,
}

/// `morphology.json`.
#[derive(Debug, Clone, Serialize)]
pub struct MorphologyReport {
    /// The command that makes this.
    pub built_by: String,
    /// Seconds spent in the rights gate (reading and hashing every listed file).
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
            "the output {} is inside {what}; a morphology layer never goes there",
            out.display()
        )));
    }
    Ok(())
}

/// A text pack's identity and tokens.
pub struct TextPack {
    /// Its manifest.
    pub manifest: PackManifest,
    /// SHA-256 of `manifest.json`.
    pub manifest_sha256: String,
    /// Tokens by verse.
    pub verses: PackVerses,
}

/// Reads every chapter of a built text pack through the reader (which checks each chunk's
/// hash) and takes each token's letters from its verse by scalar range.
pub fn read_text_pack(directory: &Path) -> Result<TextPack, ImportError> {
    let pack_error = |e: &dyn std::fmt::Display| ImportError::Pack(e.to_string());
    let path = directory.join("manifest.json");
    let json = std::fs::read_to_string(&path).map_err(|e| io(&path, e))?;
    let manifest: PackManifest = serde_json::from_str(&json).map_err(|e| pack_error(&e))?;
    let pack = FilePack::from_manifest(&json, Some(Box::new(DirSource::new(directory))))
        .map_err(|e| pack_error(&e))?;
    let mut library = ReadingLibrary::new();
    library.insert(Box::new(pack));
    let mut verses = BTreeMap::new();
    for book in &manifest.books {
        for chapter in &book.chapters {
            let key = format!("{}.{}", book.book, chapter.chapter);
            let block = library
                .chapter_content(&manifest.resource_id, &key)
                .map_err(|e| pack_error(&e))?;
            for verse in &block.verses {
                let chars: Vec<char> = verse.text.chars().collect();
                let id = VerseId::new(
                    book.book.number(),
                    u16::try_from(chapter.chapter).unwrap_or(0),
                    u16::try_from(verse.number).unwrap_or(0),
                );
                let tokens = verse
                    .tokens
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let text: String = chars[t.start as usize..t.end as usize].iter().collect();
                        (i as u32 + 1, text)
                    })
                    .collect();
                verses.insert(id, tokens);
            }
        }
    }
    Ok(TextPack {
        manifest_sha256: sha256_hex(json.as_bytes()),
        manifest,
        verses,
    })
}

fn record(rights: &Path, source_id: &str) -> Result<Value, ImportError> {
    let path = rights.join(format!("{source_id}.json"));
    serde_json::from_slice(&std::fs::read(&path).map_err(|e| io(&path, e))?).map_err(|e| {
        ImportError::Rights {
            source_id: source_id.to_owned(),
            detail: format!("the rights record does not parse: {e}"),
        }
    })
}

fn utf8<'a>(source: &'a VerifiedSource, path: &str) -> Result<&'a str, ImportError> {
    let bytes = source.files.get(path).ok_or_else(|| ImportError::Rights {
        source_id: source.source_id.clone(),
        detail: format!("{path} is not in the manifest"),
    })?;
    std::str::from_utf8(bytes).map_err(|_| crate::model::parse_error(path, "not UTF-8"))
}

fn commit_short(source: &VerifiedSource) -> String {
    source
        .commit
        .as_deref()
        .map_or_else(String::new, |c| c.chars().take(7).collect())
}

/// The layer manifest of a MACULA source over a text pack.
fn layer_info(
    source: &VerifiedSource,
    columns: &SourceColumns,
    text: &TextPack,
    layer_id: &str,
    title: &str,
    changes: &str,
) -> LayerInfo {
    let excluded = columns
        .restricted
        .iter()
        .map(|c| format!("`{c}`"))
        .collect::<Vec<_>>()
        .join(", ");
    LayerInfo {
        layer_id: layer_id.to_owned(),
        edition: text.manifest.resource_id.clone(),
        title: title.to_owned(),
        revision: format!(
            "{layer_id}-{}-on-{}",
            commit_short(source),
            text.manifest.content_revision
        ),
        license: format!("{}, {CC_BY}", source.license),
        attribution: format!(
            "{} Licence: {CC_BY}. Source: {} at commit {}. Changed for this layer: each word is \
             joined to a token of the {} text pack ({}); {changes}; the columns {excluded}, \
             which MACULA uses with the permission of the United Bible Societies and which are \
             not under CC BY, are not included, nor are the syntax trees, roles, referents and \
             Chinese glosses. The licensor offers the work as-is and makes no warranties \
             (CC BY 4.0 §5).",
            source.attribution,
            source.upstream_url,
            source.commit.as_deref().unwrap_or("unknown"),
            text.manifest.title,
            text.manifest.content_revision,
        ),
        source: format!(
            "{} ({}), MANIFEST.sha256 {}",
            source.upstream_url, source.edition_id, source.manifest_sha256
        ),
        text_content_revision: text.manifest.content_revision.clone(),
        text_manifest_sha256: text.manifest_sha256.clone(),
        excluded_columns: columns.restricted.iter().map(|c| (*c).to_owned()).collect(),
    }
}

/// Writes one layer and returns its size.
pub fn write_layer(
    path: &Path,
    info: &LayerInfo,
    scheme: &str,
    parsing: &[&str],
    joined: &Joined,
) -> Result<u64, ImportError> {
    let pack = |e: openlogos_retrieval::RetrievalError| ImportError::Pack(e.to_string());
    let mut writer = LayerWriter::create(path, info, scheme, parsing).map_err(pack)?;
    for row in &joined.rows {
        writer.add(row).map_err(pack)?;
    }
    for (verse, word, reason) in &joined.absent {
        writer.absent(*verse, *word, reason);
    }
    writer.finish().map_err(pack)?;
    Ok(std::fs::metadata(path).map_err(|e| io(path, e))?.len())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), ImportError> {
    let text = serde_json::to_string_pretty(value).map_err(|e| ImportError::Pack(e.to_string()))?;
    std::fs::write(path, text + "\n").map_err(|e| io(path, e))
}

/// Writes a layer, its join file and its report line.
fn emit(
    work: &Path,
    source: &VerifiedSource,
    info: LayerInfo,
    scheme: &str,
    parsing: &[&str],
    mut joined: Joined,
    started: Instant,
) -> Result<LayerReport, ImportError> {
    let file = format!("{}.{}.sqlite", info.edition, info.layer_id);
    let bytes = write_layer(
        &work.join("layers").join(&file),
        &info,
        scheme,
        parsing,
        &joined,
    )?;
    write_json(
        &work.join("joins").join(format!("{}.json", info.edition)),
        &joined.report,
    )?;
    joined.report.places.clear();
    Ok(LayerReport {
        layer: info,
        source_id: source.source_id.clone(),
        source_manifest_sha256: source.manifest_sha256.clone(),
        source_files_checked: source.files_checked,
        file,
        bytes,
        seconds: started.elapsed().as_secs_f64(),
        join: joined.report,
    })
}

/// The whole build. The repository, the corpora and the preview pack are refused as outputs.
pub fn build(
    options: &MorphologyOptions,
    repository: &Path,
) -> Result<MorphologyReport, ImportError> {
    let started = Instant::now();
    let out = absolute(&options.out)?;
    refuse_inside(&out, &absolute(repository)?, "the repository")?;
    refuse_inside(&out, &absolute(&options.corpora)?, "the corpora")?;
    refuse_inside(&out, &absolute(&options.preview_pack)?, "the preview pack")?;
    let work = out.with_extension("building");
    if work.exists() {
        std::fs::remove_dir_all(&work).map_err(|e| io(&work, e))?;
    }
    for dir in ["layers", "joins"] {
        std::fs::create_dir_all(work.join(dir)).map_err(|e| io(&work, e))?;
    }

    let packs = options.preview_pack.join("packs");
    let sblgnt = read_text_pack(&packs.join("sblgnt"))?;
    let wlc = read_text_pack(&packs.join("wlc"))?;

    let gate = Instant::now();
    let greek_record = record(&options.rights, restricted::GREEK.source_id)?;
    restricted::check(&restricted::GREEK, &greek_record)?;
    let hebrew_record = record(&options.rights, restricted::HEBREW.source_id)?;
    restricted::check(&restricted::HEBREW, &hebrew_record)?;
    const GREEK_TSV: &str = "SBLGNT/tsv/macula-greek-SBLGNT.tsv";
    const HEBREW_TSV: &str = "WLC/tsv/macula-hebrew.tsv";
    let greek = verify_source(
        restricted::GREEK.source_id,
        &options.rights,
        &options.corpora.join("S47-macula-greek"),
        |p| p == GREEK_TSV,
    )?;
    let hebrew = verify_source(
        restricted::HEBREW.source_id,
        &options.rights,
        &options.corpora.join("S47-macula-hebrew"),
        |p| p == HEBREW_TSV,
    )?;
    let oshb = verify_source(
        "S07",
        &options.rights,
        &options.corpora.join("S07-morphhb"),
        |p| p.starts_with("wlc/") && p != "wlc/VerseMap.xml",
    )?;
    let verify_seconds = gate.elapsed().as_secs_f64();

    let mut layers = Vec::new();
    {
        let at = Instant::now();
        let words = macula::read(
            GREEK_TSV,
            utf8(&greek, GREEK_TSV)?,
            &restricted::GREEK,
            true,
        )?;
        let joined = join::join_greek(&sblgnt.manifest.resource_id, &words, &sblgnt.verses)?;
        let info = layer_info(
            &greek,
            &restricted::GREEK,
            &sblgnt,
            "macula-greek",
            "MACULA Greek",
            "the elision mark MACULA keeps after a word is the token's own",
        );
        layers.push(emit(
            &work,
            &greek,
            info,
            &sblgnt.manifest.scheme,
            &macula::GREEK_PARSING,
            joined,
            at,
        )?);
    }
    {
        let at = Instant::now();
        let words = macula::read(
            HEBREW_TSV,
            utf8(&hebrew, HEBREW_TSV)?,
            &restricted::HEBREW,
            false,
        )?;
        let mut readings = BTreeMap::new();
        for book in BookOrder::TanakhBhs.arrange(&openlogos_core::canon::old_testament()) {
            let path = format!("wlc/{}.xml", book.as_osis());
            for ((chapter, verse), r) in ketiv::parse_book(&path, utf8(&oshb, &path)?, book)? {
                readings.insert(
                    VerseId::new(
                        book.number(),
                        u16::try_from(chapter).unwrap_or(0),
                        u16::try_from(verse).unwrap_or(0),
                    ),
                    r,
                );
            }
        }
        let joined = join::join_hebrew(&wlc.manifest.resource_id, &words, &wlc.verses, &readings)?;
        let info = layer_info(
            &hebrew,
            &restricted::HEBREW,
            &wlc,
            "macula-hebrew",
            "MACULA Hebrew",
            "MACULA analyses the qere; its words are joined to the ketiv tokens they are read \
             for, which the text pack prints, as located by the ketiv and qere markup of the \
             Open Scriptures Hebrew Bible (CC BY 4.0) the pack was built from",
        );
        layers.push(emit(
            &work,
            &hebrew,
            info,
            &wlc.manifest.scheme,
            &macula::HEBREW_PARSING,
            joined,
            at,
        )?);
    }

    let report = MorphologyReport {
        built_by: "just preview-pack-morphology".to_owned(),
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

#[cfg(test)]
mod tests {
    use openlogos_retrieval_sqlite::MorphologyLayers;
    use openlogos_retrieval_sqlite::morphology::AnalysisStatus;

    use super::ketiv::{Qere, VerseReadings, WrittenWord};
    use super::restricted::{GREEK, HEBREW, SourceColumns};
    use super::*;

    /// TSV rows in the source's format (not its data); every restricted cell holds a marker.
    fn snippet(columns: &SourceColumns, rows: &[&[(&str, &str)]], marker: &str) -> String {
        let mut text = columns.header.join("\t");
        for row in rows {
            let cells: Vec<String> = columns
                .header
                .iter()
                .map(|h| {
                    if columns.restricted.contains(h) {
                        format!("{marker}{h}")
                    } else {
                        row.iter()
                            .find(|(k, _)| k == h)
                            .map_or_else(String::new, |(_, v)| (*v).to_owned())
                    }
                })
                .collect();
            text.push('\n');
            text.push_str(&cells.join("\t"));
        }
        text + "\n"
    }

    fn temp(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "openlogos-b026-{name}-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    fn info(edition: &str, columns: &SourceColumns) -> LayerInfo {
        LayerInfo {
            layer_id: format!("test-{edition}"),
            edition: edition.to_owned(),
            title: "Test".to_owned(),
            revision: "r".to_owned(),
            license: "CC-BY-4.0".to_owned(),
            attribution: "Test attribution".to_owned(),
            source: "test".to_owned(),
            text_content_revision: "t".to_owned(),
            text_manifest_sha256: "0".repeat(64),
            excluded_columns: columns.restricted.iter().map(|c| (*c).to_owned()).collect(),
        }
    }

    const MARKER: &str = "RESTRICTED-CELL-";

    /// Gen 8:17 in miniature: a written word of two morphemes, a ketiv read as a qere, a qere
    /// with nothing written, and a ketiv not read.
    fn hebrew() -> (
        Vec<macula::MaculaWord>,
        PackVerses,
        BTreeMap<VerseId, VerseReadings>,
    ) {
        let r = |id: &'static str,
                 reference: &'static str,
                 text: &'static str,
                 lemma: &'static str,
                 pos: &'static str|
         -> Vec<(&'static str, &'static str)> {
            vec![
                ("xml:id", id),
                ("ref", reference),
                ("text", text),
                ("lemma", lemma),
                ("pos", pos),
                ("lang", "H"),
                ("morph", "X"),
                ("gloss", "g"),
            ]
        };
        let rows = [
            r("o010080170131", "GEN 8:17!1", "הָ", "הַ", "particle"),
            r("o010080170132", "GEN 8:17!1", "אָ֖רֶץ", "אֶרֶץ", "noun"),
            r("o010080170151", "GEN 8:17!3", "הַיְצֵ֣א", "יָצָא", "verb"),
            r("o010080170161", "GEN 8:17!4", "אֵלַ֖י", "אֶל", "preposition"),
        ];
        let rows: Vec<&[(&str, &str)]> = rows.iter().map(Vec::as_slice).collect();
        let text = snippet(&HEBREW, &rows, MARKER);
        let words = macula::read("h.tsv", &text, &HEBREW, false).unwrap();
        let verse = VerseId::new(1, 8, 17);
        let pack: PackVerses = [(
            verse,
            vec![
                (1, "הָאָ֖רֶץ".to_owned()),
                (2, "הוצא".to_owned()),
                (3, "אם".to_owned()),
            ],
        )]
        .into();
        let readings = [(
            verse,
            VerseReadings {
                words: vec![
                    WrittenWord {
                        text: "הָאָ֖רֶץ".into(),
                        ketiv: false,
                    },
                    WrittenWord {
                        text: "הוצא".into(),
                        ketiv: true,
                    },
                    WrittenWord {
                        text: "אם".into(),
                        ketiv: true,
                    },
                ],
                qere: vec![
                    Qere {
                        after: 2,
                        ketiv: vec![2],
                        words: vec!["הַיְצֵ֣א".into()],
                    },
                    Qere {
                        after: 2,
                        ketiv: vec![],
                        words: vec!["אֵלַ֖י".into()],
                    },
                    Qere {
                        after: 3,
                        ketiv: vec![3],
                        words: vec![],
                    },
                ],
            },
        )]
        .into();
        (words, pack, readings)
    }

    #[test]
    fn hebrew_words_join_by_the_ketiv_and_qere_markup() {
        let (words, pack, readings) = hebrew();
        let joined = join::join_hebrew("wlc", &words, &pack, &readings).unwrap();
        let r = &joined.report;
        assert_eq!(r.word_classes.get("exact"), Some(&1));
        assert_eq!(r.word_classes.get("qere"), Some(&1));
        assert_eq!(r.word_classes.get("qere_without_ketiv"), Some(&1));
        assert_eq!(r.absent_reasons.get("ketiv_not_read"), Some(&1));
        assert_eq!(
            (r.words_joined, r.words_not_joined, r.tokens_analyzed),
            (2, 1, 2)
        );
        assert_eq!(joined.rows[0].span, Some((0, 2)));
        assert_eq!(joined.rows[1].span, Some((2, 8)));
        // A word whose letters are not the token's leaves the verse unaligned, listed.
        let (mut words, pack, readings) = hebrew();
        words[0].morphs[1].text = "אֶרֶץ".into();
        let joined = join::join_hebrew("wlc", &words, &pack, &readings).unwrap();
        assert_eq!(joined.report.word_classes.get("unaligned"), Some(&3));
        assert_eq!(joined.report.words_joined, 0);
    }

    #[test]
    fn restricted_cells_never_reach_a_layer_or_a_report() {
        // Rights flags: S47-macula-greek `ln`, `domain`; S47-macula-hebrew `sdbh`,
        // `lexdomain`, `coredomain`, `contextualdomain` (module documentation of `restricted`).
        let (words, pack, readings) = hebrew();
        let hebrew_joined = join::join_hebrew("wlc", &words, &pack, &readings).unwrap();
        let greek_text = snippet(
            &GREEK,
            &[&[
                ("xml:id", "n43003016004"),
                ("ref", "JHN 3:16!4"),
                ("text", "ἠγάπησεν"),
                ("lemma", "ἀγαπάω"),
                ("class", "verb"),
                ("morph", "V-AAI-3S"),
                ("tense", "aorist"),
            ]],
            MARKER,
        );
        let greek_words = macula::read("g.tsv", &greek_text, &GREEK, true).unwrap();
        let greek_pack: PackVerses =
            [(VerseId::new(43, 3, 16), vec![(4, "ἠγάπησεν".to_owned())])].into();
        let greek_joined = join::join_greek("sblgnt", &greek_words, &greek_pack).unwrap();
        for (edition, columns, joined, parsing) in [
            ("wlc", &HEBREW, &hebrew_joined, &macula::HEBREW_PARSING[..]),
            ("sblgnt", &GREEK, &greek_joined, &macula::GREEK_PARSING[..]),
        ] {
            let path = temp(edition);
            write_layer(&path, &info(edition, columns), "org", parsing, joined).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            let report = serde_json::to_vec(&joined.report).unwrap();
            for haystack in [&bytes, &report] {
                assert!(
                    !haystack
                        .windows(MARKER.len())
                        .any(|w| w == MARKER.as_bytes()),
                    "a restricted cell of {edition} reached the output"
                );
            }
            let mut layers = MorphologyLayers::new().unwrap();
            layers.attach_file(&path).unwrap();
            let listed: Vec<&str> = columns.restricted.to_vec();
            assert_eq!(layers.layers()[0].excluded_columns, listed);
        }
    }

    #[test]
    fn a_built_layer_answers_for_written_qere_and_unread_tokens() {
        let (words, pack, readings) = hebrew();
        let joined = join::join_hebrew("wlc", &words, &pack, &readings).unwrap();
        let path = temp("answers");
        write_layer(
            &path,
            &info("wlc", &HEBREW),
            "org",
            &macula::HEBREW_PARSING,
            &joined,
        )
        .unwrap();
        let mut layers = MorphologyLayers::new().unwrap();
        layers.attach_file(&path).unwrap();
        let written = layers.analyze("wlc:01008017001").unwrap();
        let ids: Vec<_> = written.analyses[0]
            .morphemes
            .iter()
            .map(|m| m.id.clone().unwrap())
            .collect();
        assert_eq!(ids, ["wlc:01008017001a", "wlc:01008017001b"]);
        let qere = layers.analyze("wlc:01008017002").unwrap();
        assert_eq!(qere.analyses[0].text, "הַיְצֵ֣א");
        let unread = layers.analyze("wlc:01008017003").unwrap();
        assert_eq!(unread.status, AnalysisStatus::Absent);
        assert_eq!(unread.absent_reason.as_deref(), Some("ketiv_not_read"));
        assert_eq!(
            layers.lemma_search(&[], "ארץ", 5).unwrap().hits[0]
                .token_id
                .as_deref(),
            Some("wlc:01008017001")
        );
    }
}
