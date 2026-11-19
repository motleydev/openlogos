//! Readers for the three files of a Clear alignment (S55, `docs/formats.md` there): the
//! target token table, the source token table and the Scripture Burrito 0.3 alignment JSON.
//! Each checks its header or envelope exactly and stops on anything it does not know, so a
//! changed upstream format fails the build instead of being read loosely.

use std::collections::BTreeMap;

use openlogos_core::reference::BookId;
use serde::Deserialize;
use serde_json::Value;

use crate::model::{ImportError, parse_error};

/// The target table's header (`data/eng/targets/BSB/*.tsv`).
pub const TARGET_HEADER: &str =
    "id\tsource_verse\ttext\tskip_space_after\texclude\tid_range_end\tsource_verse_range_end";

/// The source table's header (`data/sources/*+required.tsv`).
pub const SOURCE_HEADER: &str =
    "id\taltId\ttext\tstrongs\tgloss\tgloss2\tlemma\tpos\tmorph\trequired";

/// One token of the target table, a word or a punctuation mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetToken {
    /// `BBCCCVVVWWW` in Clear's book numbering (Matthew is 40).
    pub id: String,
    /// The book.
    pub book: BookId,
    /// Chapter.
    pub chapter: u16,
    /// Verse (0 for a Psalm title).
    pub verse: u16,
    /// Its letters.
    pub text: String,
    /// No space follows it in the running text.
    pub skip_space_after: bool,
    /// Clear excludes it from alignment (punctuation).
    pub exclude: bool,
}

/// One row of the source table: only the id and the letters are read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceToken {
    /// MACULA `xml:id` (`n40001001001`, `o010010010011`).
    pub id: String,
    /// Its letters as Clear gives them.
    pub text: String,
}

/// One alignment record as the file has it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClearRecord {
    /// Source token ids.
    pub source: Vec<String>,
    /// Target token ids.
    pub target: Vec<String>,
    /// `id`, `origin`, `status`, as given.
    pub meta: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    docid: String,
    scheme: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    documents: Vec<Document>,
    meta: BTreeMap<String, Value>,
    roles: Vec<String>,
    #[serde(rename = "type")]
    kind: String,
    records: Vec<ClearRecord>,
}

/// An alignment file's envelope and records.
#[derive(Debug, Clone)]
pub struct ClearAlignment {
    /// The file-level metadata (`conformsTo`, `creator`).
    pub meta: BTreeMap<String, String>,
    /// The records in file order.
    pub records: Vec<ClearRecord>,
}

/// The book of Clear's number: the Protestant canon in order, 1 to 66.
pub fn clear_book(number: u8) -> Option<BookId> {
    match number {
        1..=39 => BookId::from_number(number),
        // USFM numbering leaves 40 out: Matthew is 41 there.
        40..=66 => BookId::from_number(number + 1),
        _ => None,
    }
}

fn flag(path: &str, line: usize, cell: &str) -> Result<bool, ImportError> {
    match cell {
        "" => Ok(false),
        "y" => Ok(true),
        other => Err(parse_error(
            path,
            format!("line {line}: flag `{other}` is neither empty nor y"),
        )),
    }
}

/// Reads a target table. Range columns must be empty (none are used by the BSB tables).
pub fn read_targets(path: &str, text: &str) -> Result<Vec<TargetToken>, ImportError> {
    let mut lines = text.lines();
    let header = lines
        .next()
        .unwrap_or_default()
        .trim_start_matches('\u{feff}');
    if header != TARGET_HEADER {
        return Err(parse_error(path, format!("unexpected header `{header}`")));
    }
    let mut out = Vec::new();
    for (index, line) in lines.enumerate() {
        let number = index + 2;
        if line.is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != 7 {
            return Err(parse_error(
                path,
                format!("line {number}: {} cells, not 7", cells.len()),
            ));
        }
        if !cells[5].is_empty() || !cells[6].is_empty() {
            return Err(parse_error(
                path,
                format!("line {number}: a range column is set; ranges are not read"),
            ));
        }
        let id = cells[0];
        if id.len() != 11 || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(parse_error(path, format!("line {number}: id `{id}`")));
        }
        let book = clear_book(id[0..2].parse().unwrap_or(0))
            .ok_or_else(|| parse_error(path, format!("line {number}: book of `{id}`")))?;
        if cells[2].is_empty() || cells[2].chars().any(char::is_whitespace) {
            return Err(parse_error(
                path,
                format!(
                    "line {number}: token `{}` is empty or holds a space",
                    cells[2]
                ),
            ));
        }
        out.push(TargetToken {
            id: id.to_owned(),
            book,
            chapter: id[2..5].parse().unwrap_or(0),
            verse: id[5..8].parse().unwrap_or(0),
            text: cells[2].to_owned(),
            skip_space_after: flag(path, number, cells[3])?,
            exclude: flag(path, number, cells[4])?,
        });
    }
    Ok(out)
}

/// Reads a source table: the id and the letters of each row, nothing else.
pub fn read_sources(path: &str, text: &str) -> Result<Vec<SourceToken>, ImportError> {
    let mut lines = text.lines();
    let header = lines
        .next()
        .unwrap_or_default()
        .trim_start_matches('\u{feff}');
    if header != SOURCE_HEADER {
        return Err(parse_error(path, format!("unexpected header `{header}`")));
    }
    let mut out = Vec::new();
    for (index, line) in lines.enumerate() {
        if line.is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != 10 {
            return Err(parse_error(
                path,
                format!("line {}: {} cells, not 10", index + 2, cells.len()),
            ));
        }
        out.push(SourceToken {
            id: cells[0].to_owned(),
            text: cells[2].to_owned(),
        });
    }
    Ok(out)
}

/// Reads an alignment file whose documents must be `source` then `target` (`SBLGNT`, `BSB`).
pub fn read_alignment(
    path: &str,
    bytes: &[u8],
    source: &str,
    target: &str,
) -> Result<ClearAlignment, ImportError> {
    let envelope: Envelope =
        serde_json::from_slice(bytes).map_err(|e| parse_error(path, e.to_string()))?;
    let docs: Vec<(&str, &str)> = envelope
        .documents
        .iter()
        .map(|d| (d.docid.as_str(), d.scheme.as_str()))
        .collect();
    if docs != [(source, "BCVWP"), (target, "BCVW")] {
        return Err(parse_error(path, format!("documents {docs:?}")));
    }
    if envelope.roles != ["source", "target"] || envelope.kind != "translation" {
        return Err(parse_error(
            path,
            format!("roles {:?}, type {}", envelope.roles, envelope.kind),
        ));
    }
    let meta = envelope
        .meta
        .into_iter()
        .map(|(k, v)| (k, value_text(&v)))
        .collect();
    Ok(ClearAlignment {
        meta,
        records: envelope.records,
    })
}

/// A metadata value as text: a string as it is, anything else as JSON.
pub fn value_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_books_skip_usfm_forty() {
        assert_eq!(clear_book(1), Some(BookId::from_osis("Gen").unwrap()));
        assert_eq!(clear_book(39), Some(BookId::from_osis("Mal").unwrap()));
        assert_eq!(clear_book(40), Some(BookId::from_osis("Matt").unwrap()));
        assert_eq!(clear_book(43), Some(BookId::from_osis("John").unwrap()));
        assert_eq!(clear_book(66), Some(BookId::from_osis("Rev").unwrap()));
        assert_eq!(clear_book(67), None);
    }

    #[test]
    fn tables_and_envelope_are_read_exactly() {
        let targets = format!(
            "{TARGET_HEADER}\n43001001001\t43001001\tIn\t\t\t\t\n43001001002\t43001001\tthe\t\t\t\t\n43001001003\t43001001\tWord\ty\t\t\t\n43001001004\t43001001\t,\t\ty\t\t\n"
        );
        let t = read_targets("t.tsv", &targets).unwrap();
        assert_eq!(t.len(), 4);
        assert_eq!(t[0].book, BookId::from_osis("John").unwrap());
        assert!(t[2].skip_space_after && !t[2].exclude);
        assert!(t[3].exclude);
        let bad = format!("{TARGET_HEADER}\n43001001001\t43001001\tIn\tn\t\t\t\n");
        assert!(read_targets("t.tsv", &bad).is_err());
        assert!(read_targets("t.tsv", "id\ttext\n").is_err());

        let sources =
            format!("{SOURCE_HEADER}\nn43001001001\tἘν-1\tἘν\tG1722\tin\tin\tἐν\tprep\tP\tFalse\n");
        let s = read_sources("s.tsv", &sources).unwrap();
        assert_eq!(
            (s[0].id.as_str(), s[0].text.as_str()),
            ("n43001001001", "Ἐν")
        );

        let json = br#"{"documents": [{"docid": "SBLGNT", "scheme": "BCVWP"}, {"docid": "BSB", "scheme": "BCVW"}],
            "meta": {"conformsTo": "0.3", "creator": "x"}, "roles": ["source", "target"], "type": "translation",
            "records": [{"source": ["n43001001001"], "target": ["43001001001"], "meta": {"id": "43001001.001", "origin": "manual", "status": "created"}}]}"#;
        let a = read_alignment("a.json", json, "SBLGNT", "BSB").unwrap();
        assert_eq!(a.records.len(), 1);
        assert_eq!(a.meta["creator"], "x");
        assert!(read_alignment("a.json", json, "WLCM", "BSB").is_err());
    }
}
