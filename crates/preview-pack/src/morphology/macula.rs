//! MACULA's token TSVs, read through the allowlist of [`super::restricted`]: one row per
//! Greek word, one row per Hebrew morpheme. A row's `ref` (`GEN 1:1!1`) names its verse and
//! the source's word number; rows of one word are consecutive. Anything unexpected (another
//! header, a short row, a word split across the file, an unknown book or language) stops
//! the import.

use std::collections::{BTreeMap, BTreeSet};

use openlogos_core::reference::BookId;
use openlogos_retrieval_sqlite::morphology::Language;

use super::restricted::SourceColumns;
use crate::model::{ImportError, parse_error};
use crate::usfm::USFM_CODES;

/// One morpheme (a Greek word is one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaculaMorph {
    /// `xml:id`.
    pub xml_id: String,
    /// Its letters (`text`); empty for an article the vowel of a prefix implies.
    pub text: String,
    /// What follows it (`after`): a space, punctuation, maqqef, sof pasuq.
    pub after: String,
    /// `lemma`.
    pub lemma: String,
    /// `strong` (Greek) or `strongnumberx` (Hebrew).
    pub strong: String,
    /// `morph`.
    pub morph: String,
    /// `class` (Greek) or `pos` (Hebrew).
    pub pos: String,
    /// Greek, or `lang` H / A.
    pub language: Language,
    /// The decoded parsing fields that have a value.
    pub parsing: BTreeMap<String, String>,
    /// `gloss`.
    pub gloss: String,
    /// `english`.
    pub english: String,
    /// `transliteration` (Hebrew).
    pub transliteration: String,
}

/// One word of the source: its verse, number and morphemes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaculaWord {
    /// The `ref`, as the source has it.
    pub reference: String,
    /// Its book.
    pub book: BookId,
    /// Chapter.
    pub chapter: u32,
    /// Verse.
    pub verse: u32,
    /// The source's word number in the verse (after `!`).
    pub number: u32,
    /// Its morphemes in order.
    pub morphs: Vec<MaculaMorph>,
}

impl MaculaWord {
    /// The morphemes' letters run together.
    #[must_use]
    pub fn text(&self) -> String {
        self.morphs.iter().map(|m| m.text.as_str()).collect()
    }
}

/// The parsing fields carried for each language, in a grammar's order.
pub const GREEK_PARSING: [&str; 9] = [
    "type", "person", "number", "gender", "case", "tense", "voice", "mood", "degree",
];
/// The Hebrew and Aramaic parsing fields.
pub const HEBREW_PARSING: [&str; 6] = ["type", "person", "gender", "number", "state", "stem"];

fn book_of(code: &str) -> Option<BookId> {
    let index = USFM_CODES.iter().position(|c| *c == code)?;
    BookId::all().get(index).copied()
}

/// `GEN 1:1!1` into book, chapter, verse and word number.
fn parse_ref(reference: &str) -> Option<(BookId, u32, u32, u32)> {
    let (book, rest) = reference.split_once(' ')?;
    let (cv, word) = rest.split_once('!')?;
    let (chapter, verse) = cv.split_once(':')?;
    Some((
        book_of(book)?,
        chapter.parse().ok()?,
        verse.parse().ok()?,
        word.parse().ok()?,
    ))
}

/// Reads one TSV. `greek` picks the column names that differ between the two files.
pub fn read(
    file: &str,
    source: &str,
    columns: &SourceColumns,
    greek: bool,
) -> Result<Vec<MaculaWord>, ImportError> {
    let mut lines = source.lines();
    let header: Vec<&str> = lines
        .next()
        .ok_or_else(|| parse_error(file, "no header"))?
        .split('\t')
        .collect();
    if header != columns.header {
        return Err(parse_error(
            file,
            format!("the header is {header:?}, not the one this importer reads"),
        ));
    }
    // Only allowlisted columns have an index; a restricted cell is never looked at.
    let index: BTreeMap<&str, usize> = columns
        .used
        .iter()
        .map(|name| {
            let i = header
                .iter()
                .position(|h| h == name)
                .expect("checked against the header");
            (*name, i)
        })
        .collect();
    let parsing: &[&str] = if greek {
        &GREEK_PARSING
    } else {
        &HEBREW_PARSING
    };
    let mut words: Vec<MaculaWord> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (n, line) in lines.enumerate() {
        if line.is_empty() {
            continue;
        }
        let at = || format!("{file} line {}", n + 2);
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != header.len() {
            return Err(parse_error(
                &at(),
                format!("{} cells, not {}", cells.len(), header.len()),
            ));
        }
        let cell = |name: &str| -> &str { cells[index[name]] };
        let reference = cell("ref");
        let (book, chapter, verse, number) =
            parse_ref(reference).ok_or_else(|| parse_error(&at(), format!("ref {reference}")))?;
        let language = if greek {
            Language::Greek
        } else {
            match cell("lang") {
                "H" => Language::Hebrew,
                "A" => Language::Aramaic,
                other => return Err(parse_error(&at(), format!("lang {other}"))),
            }
        };
        let morph = MaculaMorph {
            xml_id: cell("xml:id").to_owned(),
            text: cell("text").to_owned(),
            after: cell("after").to_owned(),
            lemma: cell("lemma").to_owned(),
            strong: cell(if greek { "strong" } else { "strongnumberx" }).to_owned(),
            morph: cell("morph").to_owned(),
            pos: cell(if greek { "class" } else { "pos" }).to_owned(),
            language,
            parsing: parsing
                .iter()
                .filter(|p| !cell(p).is_empty())
                .map(|p| ((*p).to_owned(), cell(p).to_owned()))
                .collect(),
            gloss: cell("gloss").to_owned(),
            english: cell("english").to_owned(),
            transliteration: if greek {
                String::new()
            } else {
                cell("transliteration").to_owned()
            },
        };
        if morph.xml_id.is_empty() || morph.lemma.is_empty() && !morph.text.is_empty() {
            return Err(parse_error(&at(), "a row without an id or a lemma"));
        }
        match words.last_mut() {
            Some(word) if word.reference == reference => word.morphs.push(morph),
            _ => {
                if !seen.insert(reference.to_owned()) {
                    return Err(parse_error(
                        &at(),
                        format!("{reference} resumes after another word"),
                    ));
                }
                words.push(MaculaWord {
                    reference: reference.to_owned(),
                    book,
                    chapter,
                    verse,
                    number,
                    morphs: vec![morph],
                });
            }
        }
    }
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::super::restricted::{GREEK, HEBREW};
    use super::*;

    /// Two Hebrew rows in the source's format (not its data): a prefix and a noun of one word.
    pub(crate) fn hebrew_snippet(marker: &str) -> String {
        let mut text = HEBREW.header.join("\t");
        for (id, class, txt, lemma, pos) in [
            ("o010010010011", "prep", "בְּ", "בְּ", "preposition"),
            ("o010010010012", "noun", "רֵאשִׁ֖ית", "רֵאשִׁית", "noun"),
        ] {
            let row: Vec<String> = HEBREW
                .header
                .iter()
                .map(|h| match *h {
                    "xml:id" => id.to_owned(),
                    "ref" => "GEN 1:1!1".to_owned(),
                    "class" => class.to_owned(),
                    "text" => txt.to_owned(),
                    "lemma" => lemma.to_owned(),
                    "pos" => pos.to_owned(),
                    "lang" => "H".to_owned(),
                    "morph" => "R".to_owned(),
                    name if HEBREW.restricted.contains(&name) => format!("{marker}-{name}"),
                    _ => String::new(),
                })
                .collect();
            text.push('\n');
            text.push_str(&row.join("\t"));
        }
        text + "\n"
    }

    #[test]
    fn rows_of_one_word_group_and_restricted_cells_are_not_kept() {
        let words = read("h.tsv", &hebrew_snippet("SECRET"), &HEBREW, false).unwrap();
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text(), "בְּרֵאשִׁ֖ית");
        assert_eq!(
            (
                words[0].book,
                words[0].chapter,
                words[0].verse,
                words[0].number
            ),
            (BookId::Gen, 1, 1, 1)
        );
        assert!(!format!("{words:?}").contains("SECRET"));
    }

    #[test]
    fn another_header_a_short_row_or_a_resumed_word_stops_the_import() {
        assert!(read("g.tsv", "xml:id\tref\n", &GREEK, true).is_err());
        let snippet = hebrew_snippet("x");
        let short = snippet.replace("\tH\t", "\t");
        assert!(read("h.tsv", &short, &HEBREW, false).is_err());
        let mut lines: Vec<&str> = snippet.lines().collect();
        let first = lines[1].replace("GEN 1:1!1", "GEN 1:1!2");
        lines.insert(2, &first);
        let resumed = lines.join("\n");
        assert!(read("h.tsv", &resumed, &HEBREW, false).is_err());
    }
}
