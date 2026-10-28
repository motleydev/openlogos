//! A small ingestion path from Text-Fabric's file format into a pack, enough for the R6
//! benchmark on N1904 (CenterBLC/N1904, converted from MACULA Greek lowfat trees). It is not
//! the production pipeline of specs/19 ("Pipelines"), which mints ids from the upstream XML;
//! it exists so the engine can be checked against Text-Fabric on identical data.
//!
//! Text-Fabric format (https://annotation.github.io/text-fabric/tf/about/fileformats.html):
//! a `.tf` file is `@node` or `@edge`, metadata lines, a blank line, then data lines. A node
//! line is `value` for the next node or `node<TAB>value` to jump; `otype.tf` lists node
//! ranges per type and `oslots.tf` maps every non-slot node to its slots.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use openlogos_retrieval::VerseId;

use crate::pack::{PackBuilder, PackManifest, TokenRow};

/// Token features taken from N1904, in pack order. `domain` and `ln` (Louw-Nida) are UBS
/// data used by MACULA with permission and are deliberately not ingested (specs/19).
pub const N1904_FEATURES: &[&str] = &[
    "lemma",
    "sp",
    "case",
    "number",
    "gender",
    "tense",
    "voice",
    "mood",
    "person",
    "degree",
    "gloss",
    "strong",
    "morph",
    "normalized",
    "unaccent",
];

/// Container node types ingested, each becoming a container kind of the same name.
pub const N1904_CONTAINERS: &[&str] = &["sentence", "clause", "phrase", "wg"];

/// Book names as N1904's `book` feature spells them, with their USFM numbers.
pub const N1904_BOOKS: &[(&str, u8)] = &[
    ("Matthew", 41),
    ("Mark", 42),
    ("Luke", 43),
    ("John", 44),
    ("Acts", 45),
    ("Romans", 46),
    ("I_Corinthians", 47),
    ("II_Corinthians", 48),
    ("Galatians", 49),
    ("Ephesians", 50),
    ("Philippians", 51),
    ("Colossians", 52),
    ("I_Thessalonians", 53),
    ("II_Thessalonians", 54),
    ("I_Timothy", 55),
    ("II_Timothy", 56),
    ("Titus", 57),
    ("Philemon", 58),
    ("Hebrews", 59),
    ("James", 60),
    ("I_Peter", 61),
    ("II_Peter", 62),
    ("I_John", 63),
    ("II_John", 64),
    ("III_John", 65),
    ("Jude", 66),
    ("Revelation", 67),
];

/// What the ingestion produced.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IngestReport {
    /// Tokens (Text-Fabric slots) written.
    pub tokens: u64,
    /// Verses written.
    pub verses: u64,
    /// Containers written per kind.
    pub containers: Vec<(String, u64)>,
    /// Containers whose slots are not contiguous; stored as their hull span.
    pub discontinuous: u64,
}

fn unescape(value: &str) -> String {
    if !value.contains('\\') {
        return value.to_owned();
    }
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The data lines of a `.tf` file (everything after the first blank line).
fn data_lines(text: &str) -> impl Iterator<Item = &str> {
    let body = text.split_once("\n\n").map_or("", |(_, b)| b);
    body.lines()
}

/// Reads a node feature into `values[node]` (index 0 unused).
fn read_node_feature(path: &Path, max_node: usize) -> io::Result<Vec<Option<String>>> {
    let text = fs::read_to_string(path)?;
    let mut values: Vec<Option<String>> = vec![None; max_node + 1];
    let mut node = 0usize;
    for line in data_lines(&text) {
        let (n, value) = match line.split_once('\t') {
            Some((n, v)) => (
                n.parse::<usize>().map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{}: {e}", path.display()),
                    )
                })?,
                v,
            ),
            None => (node + 1, line),
        };
        node = n;
        if node <= max_node {
            values[node] = Some(unescape(value));
        }
    }
    Ok(values)
}

/// Reads `otype.tf`: `(first node, last node, type)` ranges.
fn read_otype(path: &Path) -> io::Result<Vec<(u32, u32, String)>> {
    let text = fs::read_to_string(path)?;
    let mut ranges = Vec::new();
    let mut node = 0u32;
    for line in data_lines(&text) {
        let (range, kind) = line.split_once('\t').unwrap_or((line, line));
        let (first, last) = match range.split_once('-') {
            Some((a, b)) => (a.parse::<u32>(), b.parse::<u32>()),
            None => (range.parse::<u32>(), range.parse::<u32>()),
        };
        let (first, last) = match (first, last) {
            (Ok(a), Ok(b)) => (a, b),
            _ => (node + 1, node + 1),
        };
        node = last;
        ranges.push((first, last, kind.to_owned()));
    }
    Ok(ranges)
}

/// One node's slots: the hull `(first, last)` and, only when the slots are not one contiguous
/// range, the exact ascending slot list.
type Slots = (u32, u32, Option<Vec<u32>>);

/// Reads `oslots.tf`: for every non-slot node, its slot hull and exact slots when
/// discontinuous.
fn read_oslots(path: &Path, max_slot: u32) -> io::Result<HashMap<u32, Slots>> {
    let text = fs::read_to_string(path)?;
    let mut map = HashMap::new();
    let mut node = max_slot;
    for line in data_lines(&text) {
        let (n, slots) = match line.split_once('\t') {
            Some((n, s)) => (n.parse::<u32>().unwrap_or(node + 1), s),
            None => (node + 1, line),
        };
        node = n;
        let mut first = u32::MAX;
        let mut last = 0;
        let mut ranges: Vec<(u32, u32)> = Vec::new();
        for part in slots.split(',') {
            let (a, b) = match part.split_once('-') {
                Some((a, b)) => (a.parse::<u32>().unwrap_or(0), b.parse::<u32>().unwrap_or(0)),
                None => {
                    let v = part.parse::<u32>().unwrap_or(0);
                    (v, v)
                }
            };
            first = first.min(a);
            last = last.max(b);
            ranges.push((a, b));
        }
        let covered: u64 = ranges.iter().map(|(a, b)| u64::from(b - a + 1)).sum();
        let exact = if covered == u64::from(last - first + 1) {
            None
        } else {
            ranges.sort_unstable();
            Some(ranges.iter().flat_map(|(a, b)| *a..=*b).collect())
        };
        map.insert(node, (first, last, exact));
    }
    Ok(map)
}

/// Builds a pack at `out` from the Text-Fabric directory `tf_dir` (the `tf/1.0.0` folder).
pub fn build_n1904_pack(
    tf_dir: &Path,
    out: &Path,
    manifest: &PackManifest,
) -> io::Result<IngestReport> {
    let otype = read_otype(&tf_dir.join("otype.tf"))?;
    let max_slot = otype
        .iter()
        .find(|(_, _, k)| k == "word")
        .map(|(_, last, _)| *last)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "otype has no word range"))?;
    let oslots = read_oslots(&tf_dir.join("oslots.tf"), max_slot)?;

    let book = read_node_feature(&tf_dir.join("book.tf"), max_slot as usize)?;
    let chapter = read_node_feature(&tf_dir.join("chapter.tf"), max_slot as usize)?;
    let verse = read_node_feature(&tf_dir.join("verse.tf"), max_slot as usize)?;
    let text = read_node_feature(&tf_dir.join("text.tf"), max_slot as usize)?;
    let before = read_node_feature(&tf_dir.join("before.tf"), max_slot as usize)?;
    let after = read_node_feature(&tf_dir.join("after.tf"), max_slot as usize)?;
    let mut features: Vec<Vec<Option<String>>> = Vec::with_capacity(N1904_FEATURES.len());
    for f in N1904_FEATURES {
        features.push(read_node_feature(
            &tf_dir.join(format!("{f}.tf")),
            max_slot as usize,
        )?);
    }
    let books: HashMap<&str, u8> = N1904_BOOKS.iter().copied().collect();

    let verse_of = |slot: usize| -> io::Result<VerseId> {
        let b = book[slot].as_deref().unwrap_or("");
        let number = *books.get(b).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown book `{b}` at slot {slot}"),
            )
        })?;
        let c: u16 = chapter[slot].as_deref().unwrap_or("0").parse().unwrap_or(0);
        let v: u16 = verse[slot].as_deref().unwrap_or("0").parse().unwrap_or(0);
        Ok(VerseId::new(number, c, v))
    };

    let mut builder = PackBuilder::create(out, manifest, N1904_FEATURES).map_err(sql)?;
    let mut report = IngestReport::default();

    // Verses: concatenate before + text + after per slot, byte for byte.
    let mut current: Option<(VerseId, String)> = None;
    let mut word_no = 0u32;
    for slot in 1..=max_slot as usize {
        let vid = verse_of(slot)?;
        if current.as_ref().is_none_or(|(v, _)| *v != vid) {
            if let Some((v, body)) = current.take() {
                builder.add_verse(v, body.trim_end()).map_err(sql)?;
                report.verses += 1;
            }
            current = Some((vid, String::new()));
            word_no = 0;
        }
        let body = &mut current.as_mut().expect("set above").1;
        body.push_str(before[slot].as_deref().unwrap_or(""));
        body.push_str(text[slot].as_deref().unwrap_or(""));
        body.push_str(after[slot].as_deref().unwrap_or(""));
        word_no += 1;
        let values: Vec<Option<&str>> = features.iter().map(|f| f[slot].as_deref()).collect();
        builder
            .add_token(&TokenRow {
                verse: vid,
                word: word_no,
                surface: text[slot].as_deref().unwrap_or(""),
                features: values,
            })
            .map_err(sql)?;
        report.tokens += 1;
    }
    if let Some((v, body)) = current.take() {
        builder.add_verse(v, body.trim_end()).map_err(sql)?;
        report.verses += 1;
    }

    // Containers: hull spans with exact slots where discontinuous, parents by span nesting
    // across the ingested kinds.
    let mut spans: Vec<(u32, u32, usize, u32)> = Vec::new(); // (first, last, kind index, tf node)
    for (kind_index, kind) in N1904_CONTAINERS.iter().enumerate() {
        let kind_id = builder.add_container_kind(kind);
        debug_assert_eq!(kind_id, kind_index);
        for (first_node, last_node, k) in &otype {
            if k != kind {
                continue;
            }
            for node in *first_node..=*last_node {
                if let Some((first, last, exact)) = oslots.get(&node) {
                    spans.push((*first - 1, *last - 1, kind_index, node));
                    if exact.is_some() {
                        report.discontinuous += 1;
                    }
                }
            }
        }
    }
    // Outer spans first: ascending first, descending last; for equal spans keep kind order
    // (sentence before clause before phrase before wg) so the parent chain stays sensible.
    spans.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
    let mut stack: Vec<(u32, u32, u32)> = Vec::new(); // (last, container id, first)
    let mut counts = vec![0u64; N1904_CONTAINERS.len()];
    for (first, last, kind, node) in &spans {
        while let Some((top_last, _, top_first)) = stack.last() {
            if *first > *top_last || *last > *top_last || *first < *top_first {
                stack.pop();
            } else {
                break;
            }
        }
        let parent = stack.last().map(|(_, id, _)| *id);
        let id = match oslots.get(node).and_then(|(_, _, exact)| exact.as_ref()) {
            Some(slots) => {
                let zero_based: Vec<u32> = slots.iter().map(|s| s - 1).collect();
                builder.add_container_with_slots(*kind, parent, &zero_based)
            }
            None => builder.add_container(*kind, parent, *first, *last),
        };
        counts[*kind] += 1;
        stack.push((*last, id, *first));
    }
    report.containers = N1904_CONTAINERS
        .iter()
        .zip(counts)
        .map(|(k, n)| ((*k).to_owned(), n))
        .collect();
    builder.finish().map_err(sql)?;
    Ok(report)
}

fn sql(e: rusqlite::Error) -> io::Error {
    io::Error::other(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::fixture::temp_path;

    /// A three-word Text-Fabric corpus written for this test (CC0): one verse, one sentence,
    /// one clause with a nested wg; the second word has no lemma line (implicit skip).
    fn write_corpus(dir: &Path) {
        let header = "@node\n@valueType=str\n\n";
        let edge = "@edge\n@valueType=str\n\n";
        let files: &[(&str, String)] = &[
            (
                "otype.tf",
                format!("{header}1-3\tword\n4\tverse\n5\tsentence\n6\tclause\n7\twg\n"),
            ),
            ("oslots.tf", format!("{edge}4\t1-3\n1-3\n1-3\n2-3\n")),
            ("book.tf", format!("{header}John\nJohn\nJohn\n")),
            ("chapter.tf", format!("{header}1\n1\n1\n")),
            ("verse.tf", format!("{header}1\n1\n1\n")),
            ("text.tf", format!("{header}Ἐν\nἀρχῇ\nἦν\n")),
            ("before.tf", header.to_string()),
            ("after.tf", format!("{header} \n \n.\n")),
            ("lemma.tf", format!("{header}ἐν\n3\tεἰμί\n")),
            ("sp.tf", format!("{header}prep\nsubs\nverb\n")),
            ("case.tf", format!("{header}2\tdative\n")),
            ("number.tf", format!("{header}2\tsingular\nsingular\n")),
            ("gender.tf", format!("{header}2\tfeminine\n")),
            ("tense.tf", format!("{header}3\timperfect\n")),
            ("voice.tf", format!("{header}3\tactive\n")),
            ("mood.tf", format!("{header}3\tindicative\n")),
            ("person.tf", format!("{header}3\tp3\n")),
            ("degree.tf", header.to_string()),
            ("gloss.tf", format!("{header}in\nbeginning\nwas\n")),
            ("strong.tf", format!("{header}1722\n746\n1510\n")),
            ("morph.tf", format!("{header}PREP\nN-DSF\nV-IAI-3S\n")),
            ("normalized.tf", format!("{header}ἐν\nἀρχῇ\nἦν\n")),
            ("unaccent.tf", format!("{header}εν\nαρχη\nην\n")),
        ];
        for (name, body) in files {
            fs::write(dir.join(name), body).unwrap();
        }
    }

    #[test]
    fn text_fabric_files_become_a_pack() {
        let pack = temp_path("ingest");
        let dir = pack.parent().unwrap().join("tf");
        fs::create_dir_all(&dir).unwrap();
        write_corpus(&dir);
        let manifest = PackManifest {
            edition: "tf-test".into(),
            scheme: "org".into(),
            source: "test".into(),
            license: "CC0-1.0".into(),
            attribution: String::new(),
        };
        let report = build_n1904_pack(&dir, &pack, &manifest).unwrap();
        assert_eq!(
            (report.tokens, report.verses, report.discontinuous),
            (3, 1, 0)
        );
        assert_eq!(
            report.containers,
            vec![
                ("sentence".into(), 1),
                ("clause".into(), 1),
                ("phrase".into(), 0),
                ("wg".into(), 1)
            ]
        );
        let conn = crate::pack::open_read_only(&pack).unwrap();
        let body: String = conn
            .query_row("SELECT body FROM text", [], |r| r.get(0))
            .unwrap();
        assert_eq!(body, "Ἐν ἀρχῇ ἦν.");
        let vid: i64 = conn
            .query_row("SELECT verse_id FROM text", [], |r| r.get(0))
            .unwrap();
        assert_eq!(vid, 44_001_001);
        // Lemma of the second word is absent (implicit skip), the third is εἰμί.
        let lemma3: String = conn
            .query_row(
                "SELECT value FROM feature_value WHERE feature_id = 1 AND value_id =
                 (SELECT value_id FROM feature_value WHERE feature_id = 1 AND value = 'εἰμί')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lemma3, "εἰμί");
        let col: Vec<u8> = conn
            .query_row(
                "SELECT value_ids FROM token_column WHERE feature_id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(col, [1u8, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0]);
        // The wg (slots 2-3) nests inside the clause, which nests inside the sentence.
        let parents: Vec<(i64, Option<i64>)> = conn
            .prepare("SELECT id, parent_id FROM container ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(parents, vec![(1, None), (2, Some(1)), (3, Some(2))]);
    }

    #[test]
    fn escapes_and_ranges_parse() {
        assert_eq!(unescape("a\\tb\\nc\\\\d"), "a\tb\nc\\d");
        assert_eq!(unescape("plain"), "plain");
        let dir = temp_path("ingest-ranges").parent().unwrap().to_path_buf();
        fs::write(dir.join("oslots.tf"), "@edge\n\n5\t1-3,7\n2-2\n").unwrap();
        let map = read_oslots(&dir.join("oslots.tf"), 4).unwrap();
        assert_eq!(map[&5], (1, 7, Some(vec![1, 2, 3, 7])));
        assert_eq!(map[&6], (2, 2, None));
    }
}
