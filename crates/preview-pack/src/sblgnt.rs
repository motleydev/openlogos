//! The SBLGNT (`data/sblgnt/text/*.txt`, verse lines, with `data/sblgnt/xml/*.xml` for the
//! paragraphs and as a second reading of every verse).
//!
//! **Apparatus signs.** The running text carries the signs `⸀ ⸁ ⸂ ⸃ ⸄ ⸅` (`⸀1` numbered), pointers into an
//! apparatus (`data/sblgntapp`) this pack does not carry. They are removed, as the project's
//! SBLGNT fixtures already remove them (B006): a sign that points at nothing is noise in a reading text.
//! The editorial brackets `⟦ ⟧` (Mk 16:9–20, Jn 7:53–8:11) and `[ ]` say something about the
//! text itself and stay. Removing the signs is a change under CC BY 4.0 §3(a)(1)(B), and the
//! manifest's attribution says so. Nothing else changes; the space that ends every verse line is
//! the file's separator and is not part of the verse.

use std::collections::BTreeMap;

use openlogos_core::reference::{BookId, Reference};
use openlogos_text::reading::ChapterVerse;

use crate::model::{
    Counters, ImportError, ImportedChapter, Layout, count, make_verse, parse_error,
    whitespace_words,
};

/// The apparatus signs left out of the running text.
pub const APPARATUS_SIGNS: [char; 6] = ['⸀', '⸁', '⸂', '⸃', '⸄', '⸅'];

/// What the XML says about a book: the first verse of each paragraph, and each verse's words.
struct XmlBook {
    paragraphs: Vec<(u32, u32)>,
    words: BTreeMap<(u32, u32), String>,
    inside_a_verse: u64,
}

/// `(chapter, verse)` of `John 3:16`.
fn place(id: &str) -> Option<(u32, u32)> {
    let (_, place) = id.rsplit_once(' ')?;
    let (chapter, verse) = place.split_once(':')?;
    Some((chapter.parse().ok()?, verse.parse().ok()?))
}

fn read_xml(file: &str, xml: &str) -> Result<XmlBook, ImportError> {
    let mut paragraphs = Vec::new();
    let mut words: BTreeMap<(u32, u32), String> = BTreeMap::new();
    let mut inside_a_verse = 0;
    let mut current: Option<(u32, u32)> = None;
    let mut pending_paragraph = false;
    let mut rest = xml;
    while let Some(open) = rest.find('<') {
        let close = rest[open..]
            .find('>')
            .ok_or_else(|| parse_error(file, "an unclosed tag"))?
            + open;
        let tag = &rest[open + 1..close];
        rest = &rest[close + 1..];
        if tag == "p" {
            pending_paragraph = true;
        } else if let Some(id) = tag
            .strip_prefix("verse-number id=\"")
            .and_then(|t| t.strip_suffix('"'))
        {
            let id = place(id).ok_or_else(|| parse_error(file, format!("verse id {id}")))?;
            if pending_paragraph {
                paragraphs.push(id);
            }
            pending_paragraph = false;
            current = Some(id);
            words.insert(id, String::new());
        } else if tag == "w" {
            if pending_paragraph {
                inside_a_verse += 1;
                pending_paragraph = false;
            }
            let end = rest
                .find('<')
                .ok_or_else(|| parse_error(file, "an unclosed word"))?;
            let Some(id) = current.as_ref() else {
                return Err(parse_error(file, "a word before any verse"));
            };
            words
                .get_mut(id)
                .expect("inserted at the verse number")
                .push_str(&rest[..end]);
        }
    }
    Ok(XmlBook {
        paragraphs,
        words,
        inside_a_verse,
    })
}

/// A verse line without its apparatus signs. A sign may carry a number (`⸀1ἄλλῳ`, 1 Cor 12:10,
/// 2 Cor 7:12: the third variation unit of its kind in the verse); the number goes with it.
fn without_signs(body: &str) -> String {
    let mut clean = String::with_capacity(body.len());
    let mut after_sign = false;
    for c in body.chars() {
        if APPARATUS_SIGNS.contains(&c) {
            after_sign = true;
        } else if !(after_sign && c.is_ascii_digit()) {
            clean.push(c);
            after_sign = false;
        }
    }
    clean
}

/// The letters of a verse, for comparing the two forms of the source.
fn letters(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// One book's text and XML into its title and chapters.
pub fn parse_book(
    file: &str,
    text: &str,
    xml: &str,
    edition: &str,
    book: BookId,
    counters: &mut Counters,
) -> Result<(String, Vec<ImportedChapter>), ImportError> {
    let xml = read_xml(file, xml.trim_start_matches('\u{feff}'))?;
    count(
        counters,
        "paragraph_breaks_inside_a_verse",
        xml.inside_a_verse,
    );
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    let title = lines
        .next()
        .map(str::trim)
        .filter(|t| !t.is_empty() && !t.contains('\t'))
        .ok_or_else(|| parse_error(file, "the first line is not the title"))?
        .to_owned();
    let mut chapters: BTreeMap<u32, Vec<ChapterVerse>> = BTreeMap::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let (id, body) = line
            .split_once('\t')
            .ok_or_else(|| parse_error(file, format!("a line without a verse: {line}")))?;
        let Some((chapter, number)) = place(id) else {
            return Err(parse_error(file, format!("verse id {id}")));
        };
        let body = body.strip_suffix(' ').unwrap_or(body);
        for sign in APPARATUS_SIGNS {
            count(
                counters,
                &format!("apparatus_sign_{sign}"),
                body.matches(sign).count() as u64,
            );
        }
        let clean = without_signs(body);
        if clean.trim().is_empty()
            || clean != clean.trim()
            || clean.contains("  ")
            || clean.chars().any(|c| c.is_ascii_digit())
        {
            return Err(parse_error(
                file,
                format!("{id}: unexpected spacing after removing the signs: [{clean}]"),
            ));
        }
        let Some(xml_words) = xml.words.get(&(chapter, number)) else {
            return Err(ImportError::Oracle(format!("{id} is not in the XML")));
        };
        // The text and the XML hold the same words.
        if letters(&clean) != letters(xml_words) {
            return Err(ImportError::Oracle(format!(
                "{id}: the text and the XML differ"
            )));
        }
        let verses = chapters.entry(chapter).or_default();
        let reference = Reference::verse(book, chapter, number);
        let words = whitespace_words(&clean);
        let layout = Layout {
            paragraph_start: verses.is_empty() || xml.paragraphs.contains(&(chapter, number)),
            poetry: false,
            superscription: false,
        };
        verses.push(make_verse(
            edition,
            reference.book,
            chapter,
            number,
            clean,
            &words,
            layout,
        )?);
    }
    let total: usize = chapters.values().map(Vec::len).sum();
    if total != xml.words.len() {
        return Err(ImportError::Oracle(format!(
            "{file}: {total} verse lines, {} verses in the XML",
            xml.words.len()
        )));
    }
    Ok((
        title,
        chapters
            .into_iter()
            .map(|(chapter, verses)| ImportedChapter { chapter, verses })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Format only: the Greek here is a few words written for the test.
    const TEXT: &str =
        "ΤΙΤΛΟΣ\nJohn 1:1\tλόγος ⸀καὶ ⸂θεὸς⸃, \nJohn 1:2\t⟦καὶ [πάλιν]⟧ \nJohn 2:1\tκαὶ⸆ \n";
    const XML: &str = "<book id=\"Jn\"><title>ΤΙΤΛΟΣ</title><p><verse-number id=\"John 1:1\">1:1</verse-number><w>λόγος</w><suffix> </suffix><prefix> ⸀</prefix><w>καὶ</w><suffix></suffix><prefix> ⸂</prefix><w>θεὸς</w><suffix>⸃, </suffix></p><verse-number id=\"John 1:2\">2</verse-number><w>καὶ</w><suffix> </suffix><w>πάλιν</w></book>";

    #[test]
    fn signs_leave_brackets_stay_and_the_line_end_is_not_text() {
        let text = TEXT.replace("John 2:1\tκαὶ⸆ \n", "");
        let mut counters = Counters::new();
        let (title, chapters) =
            parse_book("t", &text, XML, "sblgnt", BookId::John, &mut counters).unwrap();
        assert_eq!(title, "ΤΙΤΛΟΣ");
        let v = &chapters[0].verses;
        assert_eq!(v[0].text, "λόγος καὶ θεὸς,");
        assert_eq!(v[1].text, "⟦καὶ [πάλιν]⟧");
        assert!(v[0].paragraph_start && !v[1].paragraph_start);
        assert_eq!(v[0].tokens.len(), 3);
        assert_eq!(counters["apparatus_sign_⸀"], 1);
        assert_eq!(counters["apparatus_sign_⸃"], 1);
    }

    #[test]
    fn a_numbered_sign_goes_with_its_number() {
        assert_eq!(without_signs("⸁ἄλλῳ, ⸀1ἄλλῳ ⸀2ἄλλῳ"), "ἄλλῳ, ἄλλῳ ἄλλῳ");
    }

    #[test]
    fn the_text_and_the_xml_must_agree() {
        let mut counters = Counters::new();
        let bad = TEXT
            .replace("John 2:1\tκαὶ⸆ \n", "")
            .replace("θεὸς", "θεοῦ");
        assert!(parse_book("t", &bad, XML, "sblgnt", BookId::John, &mut counters).is_err());
        // A verse the XML does not have.
        assert!(parse_book("t", TEXT, XML, "sblgnt", BookId::John, &mut counters).is_err());
    }
}
