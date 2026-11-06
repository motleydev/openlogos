//! The Open Scriptures Hebrew Bible's OSIS (WLC 4.20). The reading stream of a verse is its
//! `<w>` and `<seg>` elements outside `<note>`:
//!
//! - **Morpheme separators**: `/` inside `<w>` is OSHB's markup, not the codex's text; it is
//!   removed and every other scalar of the word is kept in its source order.
//! - **Maqqef** `־` and **sof pasuq** `׃` attach to their words, as the source has them.
//! - **Paseq** `׀` and **reversed nun** `׆` are text of the codex and stay.
//! - **Petuchah and setumah** (`<seg type="x-pe">פ`, `x-samekh` ס) are layout markers, not words
//!   of a verse: they leave the text and start a paragraph at the next verse.
//! - **Small and suspended letters** (`x-small`, `x-suspended`, `x-large`) stay in their word;
//!   the size is not carried.
//! - **Ketiv and qere**: the written `<w type="x-ketiv">` stays in the verse as it stands in the
//!   source (unpointed), so the text is what the codex has; the qere is a `<note>` and does not
//!   enter the text, including the few words read but not written (`Ruth 3:5`). Counted.
//! - **Every other note** (`KJV:Ps.3.1`, textual and accent notes) is dropped.
//!
//! Whitespace between elements is one space, and each verse is its own segment, so no two words
//! fuse. Markup this module does not know is an error.

use std::collections::BTreeMap;

use openlogos_core::reference::{BookId, Reference};
use openlogos_core::versification::SchemeId;
use openlogos_text::reading::{ChapterVerse, is_poetry, title_end};

use crate::model::{
    Counters, ImportError, ImportedChapter, Layout, Span, count, make_verse, parse_error,
};

/// The Hebrew name of a book in the Hebrew Bible, what its heading shows.
#[must_use]
pub fn hebrew_title(book: BookId) -> Option<&'static str> {
    use BookId::*;
    Some(match book {
        Gen => "בראשית",
        Exod => "שמות",
        Lev => "ויקרא",
        Num => "במדבר",
        Deut => "דברים",
        Josh => "יהושע",
        Judg => "שופטים",
        Sam1 => "שמואל א",
        Sam2 => "שמואל ב",
        Kgs1 => "מלכים א",
        Kgs2 => "מלכים ב",
        Isa => "ישעיהו",
        Jer => "ירמיהו",
        Ezek => "יחזקאל",
        Hos => "הושע",
        Joel => "יואל",
        Amos => "עמוס",
        Obad => "עובדיה",
        Jonah => "יונה",
        Mic => "מיכה",
        Nah => "נחום",
        Hab => "חבקוק",
        Zeph => "צפניה",
        Hag => "חגי",
        Zech => "זכריה",
        Mal => "מלאכי",
        Ps => "תהלים",
        Job => "איוב",
        Prov => "משלי",
        Ruth => "רות",
        Song => "שיר השירים",
        Eccl => "קהלת",
        Lam => "איכה",
        Esth => "אסתר",
        Dan => "דניאל",
        Ezra => "עזרא",
        Neh => "נחמיה",
        Chr1 => "דברי הימים א",
        Chr2 => "דברי הימים ב",
        _ => return None,
    })
}

/// A tag's attribute value.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

/// The tag's element name (`w` of `<w lemma="1">`, `/w` of `</w>`).
fn tag_name(tag: &str) -> &str {
    tag.split(|c: char| c.is_whitespace() || c == '/' && !tag.starts_with('/'))
        .next()
        .unwrap_or(tag)
}

#[derive(Default)]
struct VerseBuilder {
    text: String,
    scalars: u32,
    words: Vec<Span>,
    word_start: Option<u32>,
    pending_space: bool,
    /// Whether the last thing written binds what follows (a maqqef).
    bound: bool,
    /// Whether the verse ended on a petuchah or setumah.
    section_end: bool,
}

impl VerseBuilder {
    fn push(&mut self, s: &str) {
        for c in s.chars() {
            self.text.push(c);
            self.scalars += 1;
        }
    }

    /// Writes the pending space unless the next thing attaches to the previous one.
    fn before(&mut self, attaches: bool) {
        if self.pending_space && !attaches && !self.bound && !self.text.is_empty() {
            self.push(" ");
        }
        self.pending_space = false;
        self.bound = false;
    }
}

/// One book's OSIS into chapters.
pub fn parse_book(
    file: &str,
    source: &str,
    edition: &str,
    book: BookId,
    counters: &mut Counters,
) -> Result<Vec<ImportedChapter>, ImportError> {
    let org = SchemeId::new("org");
    let mut chapters: BTreeMap<u32, Vec<ChapterVerse>> = BTreeMap::new();
    let mut rest = source;
    let mut verse: Option<(Reference, VerseBuilder)> = None;
    let mut in_word = false;
    let mut note_depth = 0u32;
    let mut in_seg: Option<String> = None;
    let mut section_before = false;

    while let Some(open) = rest.find('<') {
        let between = &rest[..open];
        let close = rest[open..]
            .find('>')
            .ok_or_else(|| parse_error(file, "an unclosed tag"))?
            + open;
        let tag = &rest[open + 1..close];
        rest = &rest[close + 1..];

        if note_depth == 0
            && let Some((_, builder)) = verse.as_mut()
        {
            if between.contains('&') {
                return Err(parse_error(file, format!("an entity in text: {between}")));
            }
            if in_word {
                builder.push(&between.replace('/', ""));
            } else if let Some(kind) = in_seg.as_deref() {
                match kind {
                    // Layout, not text.
                    "x-pe" | "x-samekh" => {}
                    "x-maqqef" | "x-sof-pasuq" => {
                        builder.before(true);
                        builder.push(between);
                        builder.bound = kind == "x-maqqef";
                    }
                    _ => {
                        builder.before(false);
                        builder.push(between);
                    }
                }
            } else if !between.trim().is_empty() {
                return Err(parse_error(
                    file,
                    format!(
                        "{} has text outside a word: {}",
                        verse.as_ref().map_or(String::new(), |(r, _)| r.to_osis()),
                        between.trim()
                    ),
                ));
            } else if !between.is_empty() {
                builder.pending_space = true;
            }
        }

        if note_depth > 0 {
            match tag_name(tag) {
                "note" if !tag.ends_with('/') => note_depth += 1,
                "/note" => note_depth -= 1,
                _ => {}
            }
            continue;
        }
        let name = tag_name(tag);
        match name {
            "verse" => {
                let id = attr(tag, "osisID")
                    .ok_or_else(|| parse_error(file, "a verse without an osisID"))?;
                let reference = osis_verse(id, book)
                    .ok_or_else(|| parse_error(file, format!("verse id {id}")))?;
                verse = Some((reference, VerseBuilder::default()));
            }
            "/verse" => {
                let (reference, mut builder) = verse
                    .take()
                    .ok_or_else(|| parse_error(file, "a closing verse with none open"))?;
                builder.pending_space = false;
                if builder.text.is_empty() {
                    return Err(parse_error(
                        file,
                        format!("{} has no text", reference.to_osis()),
                    ));
                }
                let number = reference.verse.unwrap_or(0);
                let chapter = chapters.entry(reference.chapter).or_default();
                let superscription =
                    title_end(&org, book, reference.chapter).is_some_and(|end| number <= end);
                let layout = Layout {
                    paragraph_start: chapter.is_empty()
                        || section_before
                        || chapter.last().is_some_and(|v| v.superscription) && !superscription,
                    poetry: !superscription && is_poetry(book, reference.chapter, number),
                    superscription,
                };
                section_before = builder.section_end;
                chapter.push(make_verse(
                    edition,
                    book,
                    reference.chapter,
                    number,
                    builder.text,
                    &builder.words,
                    layout,
                )?);
            }
            "w" => {
                let (_, builder) = verse
                    .as_mut()
                    .ok_or_else(|| parse_error(file, "a word outside a verse"))?;
                builder.before(false);
                if attr(tag, "type") == Some("x-ketiv") {
                    count(counters, "ketiv_words_in_text", 1);
                }
                if tag.ends_with('/') {
                    return Err(parse_error(file, "an empty word"));
                }
                if builder.section_end {
                    // A section mark inside a verse (Gen 35:22): the format breaks only at
                    // verses, so it is counted and the verse runs on.
                    count(counters, "section_marks_inside_a_verse", 1);
                    builder.section_end = false;
                }
                builder.word_start = Some(builder.scalars);
                in_word = true;
            }
            "/w" => {
                let (_, builder) = verse
                    .as_mut()
                    .ok_or_else(|| parse_error(file, "a closing word outside a verse"))?;
                let start = builder
                    .word_start
                    .take()
                    .ok_or_else(|| parse_error(file, "a closing word with none open"))?;
                builder.words.push((start, builder.scalars));
                in_word = false;
            }
            "seg" => {
                let kind =
                    attr(tag, "type").ok_or_else(|| parse_error(file, "a seg without a type"))?;
                match kind {
                    "x-small" | "x-large" | "x-suspended" if in_word => {
                        count(counters, &format!("seg_{kind}_size_not_carried"), 1);
                    }
                    "x-maqqef" | "x-sof-pasuq" | "x-paseq" | "x-reversednun" if !in_word => {
                        count(counters, &format!("seg_{kind}"), 1);
                        in_seg = Some(kind.to_owned());
                    }
                    "x-pe" | "x-samekh" if !in_word => {
                        count(counters, &format!("seg_{kind}_to_paragraph"), 1);
                        if let Some((_, builder)) = verse.as_mut() {
                            builder.section_end = true;
                        }
                        in_seg = Some(kind.to_owned());
                    }
                    other => {
                        return Err(parse_error(file, format!("seg type {other}")));
                    }
                }
            }
            "/seg" => {
                if !in_word {
                    in_seg = None;
                }
            }
            "note" => {
                count(counters, "dropped_notes", 1);
                let qere = verse.is_some() && {
                    let end = rest.find("</note>").unwrap_or(0);
                    rest[..end].contains("x-qere")
                };
                if qere {
                    count(counters, "qere_notes_not_in_text", 1);
                    let end = rest.find("</note>").unwrap_or(0);
                    if !rest[..end].contains("<catchWord") {
                        count(counters, "qere_without_ketiv_not_in_text", 1);
                    }
                }
                if !tag.ends_with('/') {
                    note_depth = 1;
                }
            }
            _ => {
                if verse.is_some() {
                    return Err(parse_error(file, format!("an element <{name}> in a verse")));
                }
            }
        }
    }
    Ok(chapters
        .into_iter()
        .map(|(chapter, verses)| ImportedChapter { chapter, verses })
        .collect())
}

fn osis_verse(id: &str, expected: BookId) -> Option<Reference> {
    let mut parts = id.split('.');
    let book = BookId::from_osis(parts.next()?)?;
    let chapter = parts.next()?.parse().ok()?;
    let verse = parts.next()?.parse().ok()?;
    (book == expected && parts.next().is_none()).then(|| Reference::verse(book, chapter, verse))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlogos_core::anchor::scalar_slice;

    /// Source format only; the words are hand-written, not a corpus excerpt.
    const SAMPLE: &str = r#"<div><chapter osisID="Ps.3">
        <verse osisID="Ps.3.1">
          <w lemma="1" morph="x" id="a">מִ/זְמ֥וֹר</w>
          <w lemma="2" n="1" morph="x" id="b">לְ/דָוִ֑ד</w>
          <seg type="x-paseq">׀</seg>
          <w lemma="3" morph="x" id="c">אב/ן</w><seg type="x-sof-pasuq">׃</seg>
        </verse>
        <verse osisID="Ps.3.2">
          <note>KJV:Ps.3.1</note>
          <w lemma="4" morph="x" id="d">אֶל</w><seg type="x-maqqef">־</seg><w lemma="5" morph="x" id="e">הַ/ר</w>
          <w type="x-ketiv" lemma="6" morph="x" id="f">כת/ב</w><note type="variant"><catchWord>כת/ב</catchWord><rdg type="x-qere"><w lemma="6" morph="x" id="g">קְרִ/י</w></rdg></note>
          <note type="variant"><rdg type="x-qere"><w lemma="7" morph="x" id="h">אֵלַ֖/י</w></rdg></note>
          <w lemma="8" morph="x" id="i">רְשָׁ<seg type="x-suspended">עִ֣</seg>ים</w><seg type="x-sof-pasuq">׃</seg>
          <seg type="x-pe">פ</seg>
        </verse>
        <verse osisID="Ps.3.3">
          <seg type="x-reversednun">׆</seg>
          <w lemma="9" morph="x" id="j">ט/וב</w>
          <note type="exegesis">WLC has this word divided as <rdg>ט/ו ב</rdg>.</note><seg type="x-sof-pasuq">׃</seg>
        </verse>
        <verse osisID="Ps.3.4">
          <w lemma="10" morph="x" id="k">סוף</w><seg type="x-sof-pasuq">׃</seg>
        </verse>
    </chapter></div>"#;

    fn read() -> (Vec<ChapterVerse>, Counters) {
        let mut counters = Counters::new();
        let source = SAMPLE.replacen("<div>", "", 1);
        let mut chapters = parse_book("t", &source, "wlc", BookId::Ps, &mut counters).unwrap();
        (chapters.remove(0).verses, counters)
    }

    #[test]
    fn morpheme_separators_and_notes_leave_the_text() {
        let (verses, _) = read();
        assert_eq!(verses[0].text, "מִזְמ֥וֹר לְדָוִ֑ד ׀ אבן׃");
        assert_eq!(verses[1].text, "אֶל־הַר כתב רְשָׁעִ֣ים׃");
        assert_eq!(verses[2].text, "׆ טוב׃");
        for v in &verses {
            assert!(!v.text.contains('/') && !v.text.contains("KJV"));
        }
    }

    #[test]
    fn tokens_are_words_without_maqqef_paseq_or_sof_pasuq() {
        let (verses, _) = read();
        let words = |v: &ChapterVerse| -> Vec<String> {
            v.tokens
                .iter()
                .map(|t| scalar_slice(&v.text, t.start, t.end))
                .collect()
        };
        assert_eq!(words(&verses[0]), ["מִזְמ֥וֹר", "לְדָוִ֑ד", "אבן"]);
        assert_eq!(words(&verses[1]), ["אֶל", "הַר", "כתב", "רְשָׁעִ֣ים"]);
        assert_eq!(verses[1].tokens[1].id, "wlc:19003002002");
    }

    #[test]
    fn sections_become_paragraphs_and_are_counted() {
        let (verses, counters) = read();
        assert!(verses[0].paragraph_start, "first of a chapter");
        assert!(verses[1].paragraph_start, "after the title");
        assert!(verses[2].paragraph_start, "after a petuchah");
        assert!(!verses[3].paragraph_start);
        assert_eq!(counters["seg_x-pe_to_paragraph"], 1);
        assert_eq!(counters["ketiv_words_in_text"], 1);
        assert_eq!(counters["qere_notes_not_in_text"], 2);
        assert_eq!(counters["qere_without_ketiv_not_in_text"], 1);
    }

    #[test]
    fn a_titled_psalm_marks_its_org_title() {
        let (verses, _) = read();
        assert!(verses[0].superscription, "Psalm 3's title is org verse 1");
        assert_eq!(verses[0].label, "1", "org numbers and prints it");
        assert!(!verses[1].superscription && verses[1].poetry);
    }

    #[test]
    fn unknown_markup_stops_the_import() {
        let mut counters = Counters::new();
        let bad = r#"<chapter osisID="Ps.3"><verse osisID="Ps.3.1"><foo/></verse></chapter>"#;
        assert!(parse_book("t", bad, "wlc", BookId::Ps, &mut counters).is_err());
        let bad = r#"<chapter osisID="Ps.3"><verse osisID="Ps.3.1">bare</verse></chapter>"#;
        assert!(parse_book("t", bad, "wlc", BookId::Ps, &mut counters).is_err());
        let bad = r#"<chapter osisID="Ps.3"><verse osisID="Gen.3.1"><w>א</w></verse></chapter>"#;
        assert!(parse_book("t", bad, "wlc", BookId::Ps, &mut counters).is_err());
    }
}
