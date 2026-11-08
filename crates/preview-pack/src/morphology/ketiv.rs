//! Where the WLC writes one thing and reads another: the ketiv and qere of the Open Scriptures
//! Hebrew Bible (S07, the bytes the WLC text pack was built from, through the same gate).
//! The text pack prints the written ketiv and drops every note (B071 decision 16); MACULA
//! Hebrew analyses what is read. To join the two without guessing, this reads the source's own
//! markup:
//!
//! - `<w type="x-ketiv">` is a written word of the main stream; it keeps its number;
//! - the `<note type="variant">` after it holds `<catchWord>` (the ketiv again) and
//!   `<rdg type="x-qere">` with the words read for it, none when the word is not read
//!   (`Ruth 3:12`); one note may read for a run of ketiv words (`Judg 16:25`);
//! - a qere note with no `catchWord` is read where nothing is written (`Ruth 3:5`).
//!
//! Every other note is skipped. A catch word that is not the run of ketiv words before it, a
//! ketiv with no note, or markup this module does not know stops the import.

use std::collections::BTreeMap;

use openlogos_core::reference::BookId;

use crate::model::{ImportError, parse_error};

/// One word of a verse's main stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenWord {
    /// Its letters with OSHB's `/` removed: the text pack's token.
    pub text: String,
    /// `<w type="x-ketiv">`.
    pub ketiv: bool,
}

/// One qere note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qere {
    /// Main-stream words before it.
    pub after: u32,
    /// The ketiv words it reads for (1-based word numbers); empty for a qere with no ketiv.
    pub ketiv: Vec<u32>,
    /// The words read, `/` removed; empty when the ketiv is not read.
    pub words: Vec<String>,
}

/// One verse's written words and qere notes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VerseReadings {
    /// The main stream in order; word `n` is `words[n - 1]`.
    pub words: Vec<WrittenWord>,
    /// The qere notes in order.
    pub qere: Vec<Qere>,
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

fn tag_name(tag: &str) -> &str {
    tag.split(|c: char| c.is_whitespace() || c == '/' && !tag.starts_with('/'))
        .next()
        .unwrap_or(tag)
}

/// Chapter and verse of an `osisID` in `book`.
fn verse_of(id: &str, book: BookId) -> Option<(u32, u32)> {
    let mut parts = id.split('.');
    (BookId::from_osis(parts.next()?)? == book).then_some(())?;
    let chapter = parts.next()?.parse().ok()?;
    let verse = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((chapter, verse))
}

#[derive(Default)]
struct Note {
    depth: u32,
    qere: bool,
    in_rdg: bool,
    in_catch: bool,
    in_word: bool,
    catch: String,
    words: Vec<String>,
}

/// One book's ketiv and qere, by `(chapter, verse)`.
pub fn parse_book(
    file: &str,
    source: &str,
    book: BookId,
) -> Result<BTreeMap<(u32, u32), VerseReadings>, ImportError> {
    let mut out = BTreeMap::new();
    let mut verse: Option<((u32, u32), VerseReadings, Vec<u32>)> = None;
    let mut word: Option<WrittenWord> = None;
    let mut note: Option<Note> = None;
    let mut rest = source;
    while let Some(open) = rest.find('<') {
        let between = &rest[..open];
        let close = rest[open..]
            .find('>')
            .ok_or_else(|| parse_error(file, "an unclosed tag"))?
            + open;
        let tag = &rest[open + 1..close];
        rest = &rest[close + 1..];
        let name = tag_name(tag);
        let key = verse.as_ref().map(|(k, _, _)| *k);
        let here = || key.map_or_else(String::new, |(c, v)| format!("{}.{c}.{v}", book.as_osis()));

        if let Some(n) = note.as_mut() {
            if n.in_catch {
                n.catch.push_str(between);
            }
            if n.in_word {
                n.words
                    .last_mut()
                    .expect("a word was opened")
                    .push_str(&between.replace('/', ""));
            }
            match name {
                "note" if !tag.ends_with('/') => n.depth += 1,
                "/note" => {
                    n.depth -= 1;
                    if n.depth == 0 {
                        let n = note.take().expect("inside a note");
                        if n.qere {
                            let (_, readings, pending) = verse
                                .as_mut()
                                .ok_or_else(|| parse_error(file, "a qere outside a verse"))?;
                            let after = readings.words.len() as u32;
                            let catch = n.catch.trim().replace('/', "");
                            let ketiv = if catch.is_empty() {
                                if !pending.is_empty() {
                                    return Err(parse_error(
                                        file,
                                        format!(
                                            "{}: a ketiv is followed by a qere with no catch word",
                                            here()
                                        ),
                                    ));
                                }
                                Vec::new()
                            } else {
                                let written: Vec<&str> = pending
                                    .iter()
                                    .map(|w| readings.words[*w as usize - 1].text.as_str())
                                    .collect();
                                // The catch word repeats the run's letters, with a space, a
                                // maqqef or nothing between its words (`2Kgs 6:25`).
                                let letters = |t: &str| -> String {
                                    t.chars()
                                        .filter(|c| !c.is_whitespace() && *c != '\u{05BE}')
                                        .collect()
                                };
                                if letters(&written.concat()) != letters(&catch) {
                                    return Err(parse_error(
                                        file,
                                        format!(
                                            "{}: catch word {catch} is not the ketiv {written:?}",
                                            here()
                                        ),
                                    ));
                                }
                                std::mem::take(pending)
                            };
                            readings.qere.push(Qere {
                                after,
                                ketiv,
                                words: n.words,
                            });
                        }
                    }
                }
                "catchWord" => n.in_catch = true,
                "/catchWord" => n.in_catch = false,
                "rdg" => {
                    if attr(tag, "type") == Some("x-qere") {
                        n.qere = true;
                        n.in_rdg = !tag.ends_with('/');
                    }
                }
                "/rdg" => n.in_rdg = false,
                "w" if n.in_rdg => {
                    n.words.push(String::new());
                    n.in_word = true;
                }
                "/w" if n.in_rdg => n.in_word = false,
                _ => {}
            }
            continue;
        }

        if let Some(w) = word.as_mut() {
            w.text.push_str(&between.replace('/', ""));
            match name {
                "/w" => {
                    let w = word.take().expect("inside a word");
                    let (_, readings, pending) = verse
                        .as_mut()
                        .ok_or_else(|| parse_error(file, "a word outside a verse"))?;
                    if w.ketiv {
                        pending.push(readings.words.len() as u32 + 1);
                    }
                    readings.words.push(w);
                }
                "seg" | "/seg" => {}
                other => {
                    return Err(parse_error(
                        file,
                        format!("{}: <{other}> inside a word", here()),
                    ));
                }
            }
            continue;
        }

        match name {
            "verse" => {
                let id = attr(tag, "osisID")
                    .ok_or_else(|| parse_error(file, "a verse without an osisID"))?;
                let key = verse_of(id, book)
                    .ok_or_else(|| parse_error(file, format!("verse id {id}")))?;
                verse = Some((key, VerseReadings::default(), Vec::new()));
            }
            "/verse" => {
                let (key, readings, pending) = verse
                    .take()
                    .ok_or_else(|| parse_error(file, "a closing verse with none open"))?;
                if !pending.is_empty() {
                    return Err(parse_error(
                        file,
                        format!(
                            "{}.{}.{}: a ketiv with no qere note",
                            book.as_osis(),
                            key.0,
                            key.1
                        ),
                    ));
                }
                out.insert(key, readings);
            }
            "w" if verse.is_some() => {
                if tag.ends_with('/') {
                    return Err(parse_error(file, "an empty word"));
                }
                word = Some(WrittenWord {
                    text: String::new(),
                    ketiv: attr(tag, "type") == Some("x-ketiv"),
                });
            }
            "note" if verse.is_some() && !tag.ends_with('/') => {
                note = Some(Note {
                    depth: 1,
                    ..Note::default()
                });
            }
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The markup's shapes, written for the test (letters are placeholders, not the codex).
    const SNIPPET: &str = r#"<chapter osisID="Ruth.3">
<verse osisID="Ruth.3.5">
  <w lemma="1" morph="HC" id="a1">א/ב</w>
  <note type="variant"><rdg type="x-qere"><w lemma="2" id="a2">ג/ד</w></rdg></note>
  <w type="x-ketiv" lemma="3" id="a3">הו</w><note type="variant"><catchWord>הו</catchWord><rdg type="x-qere"><w id="a4">ה/י</w><w id="a5">ז</w></rdg></note>
  <w type="x-ketiv" lemma="4" id="a6">כי</w><seg type="x-maqqef">־</seg><w type="x-ketiv" id="a7">טב</w><note type="variant"><catchWord>כיטב</catchWord><rdg type="x-qere"><w id="a8">כט<seg type="x-small">ו</seg>ב</w></rdg></note>
  <w type="x-ketiv" lemma="5" id="a9">אם</w><note type="variant"><catchWord>אם</catchWord><rdg type="x-qere"/></note>
  <w lemma="6" id="b1">ח</w><note n="a">a note that is not a qere</note><seg type="x-sof-pasuq">׃</seg>
</verse>
</chapter>"#;

    #[test]
    fn written_words_keep_their_numbers_and_each_qere_finds_its_ketiv() {
        let verses = parse_book("Ruth.xml", SNIPPET, BookId::Ruth).unwrap();
        let v = &verses[&(3, 5)];
        let written: Vec<(&str, bool)> =
            v.words.iter().map(|w| (w.text.as_str(), w.ketiv)).collect();
        assert_eq!(
            written,
            [
                ("אב", false),
                ("הו", true),
                ("כי", true),
                ("טב", true),
                ("אם", true),
                ("ח", false)
            ]
        );
        assert_eq!(
            v.qere,
            [
                Qere {
                    after: 1,
                    ketiv: vec![],
                    words: vec!["גד".into()]
                },
                Qere {
                    after: 2,
                    ketiv: vec![2],
                    words: vec!["הי".into(), "ז".into()]
                },
                Qere {
                    after: 4,
                    ketiv: vec![3, 4],
                    words: vec!["כטוב".into()]
                },
                Qere {
                    after: 5,
                    ketiv: vec![5],
                    words: vec![]
                },
            ]
        );
    }

    #[test]
    fn a_catch_word_that_is_not_the_ketiv_or_a_ketiv_without_a_note_stops_the_import() {
        let wrong = SNIPPET.replace("<catchWord>הו</catchWord>", "<catchWord>זז</catchWord>");
        assert!(parse_book("Ruth.xml", &wrong, BookId::Ruth).is_err());
        let orphan = SNIPPET.replace(
            r#"<note type="variant"><catchWord>אם</catchWord><rdg type="x-qere"/></note>"#,
            "",
        );
        assert!(parse_book("Ruth.xml", &orphan, BookId::Ruth).is_err());
    }
}
