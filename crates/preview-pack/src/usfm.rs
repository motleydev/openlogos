//! The BSB's USFM (one file per book, `bsb2usfm` output). This is where the pack's structure
//! comes from: which verses exist, paragraphs, poetry, where a Psalm's title ends. The bytes of
//! each verse then come from the publisher's `bsb.txt` (`build::take_text_from_text_file`), which
//! this parser's text is checked against letter for letter. Only the running verse text is kept:
//!
//! - **Notes** `\f ... \f*` are dropped whole: caller, reference and note text.
//! - **Headings and front matter** `\id \usfm \h \toc1-3 \mt1-2 \s1-2 \r \ms \mr \qa`, and the
//!   book division lines `\d Psalms 1–41`, are dropped whole, cross references (`\r`) with them.
//! - **Paragraph and poetry markers** `\p \pmo \pc \q1 \q2 \qr \li1 \li2 \b` become
//!   `paragraph_start` and `poetry` on the verse that starts under them; inside a verse a line
//!   break is one space, which is how the publisher's own `bsb.txt` prints it.
//! - **Character markers** `\wj` (words of Jesus) are dropped, their text kept.
//! - **Psalm titles** `\d \v 1 title` are numbered verse 1 by the BSB. Where the `eng` scheme
//!   numbers a title verse 0 (specs/23 §1, the 63 Psalms the canon table has it for), the title
//!   becomes `Ps.N.0` (`superscription`) and the rest stays `Ps.N.1`. In the other 53 titled
//!   Psalms `eng` has no verse 0 (`org` verse 1 holds title and first line alike), so none is
//!   invented: the title stays at the start of verse 1. No word is changed or moved.
//!
//! A marker this module does not know is an error, so nothing new can leak into the text.

use openlogos_core::canon;
use openlogos_core::reference::BookId;
use openlogos_core::versification::SchemeId;
use openlogos_text::reading::ChapterVerse;

use crate::model::{
    Counters, ImportError, ImportedChapter, Layout, count, make_verse, parse_error,
    whitespace_words,
};

/// USFM book codes in [`BookId::all`] order.
pub const USFM_CODES: [&str; 66] = [
    "GEN", "EXO", "LEV", "NUM", "DEU", "JOS", "JDG", "RUT", "1SA", "2SA", "1KI", "2KI", "1CH",
    "2CH", "EZR", "NEH", "EST", "JOB", "PSA", "PRO", "ECC", "SNG", "ISA", "JER", "LAM", "EZK",
    "DAN", "HOS", "JOL", "AMO", "OBA", "JON", "MIC", "NAM", "HAB", "ZEP", "HAG", "ZEC", "MAL",
    "MAT", "MRK", "LUK", "JHN", "ACT", "ROM", "1CO", "2CO", "GAL", "EPH", "PHP", "COL", "1TH",
    "2TH", "1TI", "2TI", "TIT", "PHM", "HEB", "JAS", "1PE", "2PE", "1JN", "2JN", "3JN", "JUD",
    "REV",
];

#[derive(Debug, PartialEq, Eq)]
enum Tok<'a> {
    Marker(&'a str),
    Text(&'a str),
}

fn scan(source: &str) -> Vec<Tok<'_>> {
    let mut toks = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find('\\') {
        if at > 0 {
            toks.push(Tok::Text(&rest[..at]));
        }
        let after = &rest[at + 1..];
        let mut end = after
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(after.len());
        let closing = after[end..].starts_with('*');
        if closing {
            end += 1;
        }
        toks.push(Tok::Marker(&after[..end]));
        rest = &after[end..];
        // The one space after an opening marker belongs to the marker; what follows a closing
        // marker is text (`\wj* Jesus declared`).
        if !closing {
            rest = rest.strip_prefix(' ').unwrap_or(rest);
        }
    }
    if !rest.is_empty() {
        toks.push(Tok::Text(rest));
    }
    toks
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Between a chapter marker and the next block.
    Outside,
    /// A heading or front-matter line: its text is dropped.
    Heading,
    /// `\d` in a Psalm: a title if `\v` follows at once, else a division heading.
    PendingTitle,
    /// The title of the open verse.
    Title,
    /// Verse text.
    Body,
}

/// A verse being read.
struct Open {
    number: u32,
    title: String,
    text: String,
    layout: Layout,
    started: bool,
}

struct Reader<'a> {
    file: &'a str,
    edition: &'a str,
    book: BookId,
    chapter: u32,
    verses: Vec<ChapterVerse>,
    chapters: Vec<ImportedChapter>,
    open: Option<Open>,
    mode: Mode,
    poetry_block: bool,
    pending_paragraph: bool,
    need_space: bool,
    counters: &'a mut Counters,
}

/// Appends text with every whitespace run as one space, none at either end.
fn push_text(buffer: &mut String, piece: &str, need_space: &mut bool) {
    for c in piece.chars() {
        if c.is_whitespace() {
            *need_space = true;
        } else {
            if *need_space && !buffer.is_empty() {
                buffer.push(' ');
            }
            buffer.push(c);
            *need_space = false;
        }
    }
}

impl Reader<'_> {
    fn close_verse(&mut self) -> Result<(), ImportError> {
        let Some(mut open) = self.open.take() else {
            return Ok(());
        };
        let numbered =
            canon::first_verse(&SchemeId::new("eng"), self.book, self.chapter) == Some(0);
        if !open.title.is_empty() && !numbered {
            count(self.counters, "psalm_titles_left_in_verse_1", 1);
            open.text = if open.text.is_empty() {
                open.title
            } else {
                format!("{} {}", open.title, open.text)
            };
        } else if !open.title.is_empty() {
            let words = whitespace_words(&open.title);
            self.verses.push(make_verse(
                self.edition,
                self.book,
                self.chapter,
                0,
                open.title,
                &words,
                Layout {
                    paragraph_start: true,
                    poetry: false,
                    superscription: true,
                },
            )?);
            count(self.counters, "psalm_titles_as_verse_0", 1);
        }
        if open.text.is_empty() {
            return Err(parse_error(
                self.file,
                format!("{} {}:{} has no text", self.book, self.chapter, open.number),
            ));
        }
        let words = whitespace_words(&open.text);
        self.verses.push(make_verse(
            self.edition,
            self.book,
            self.chapter,
            open.number,
            open.text,
            &words,
            open.layout,
        )?);
        Ok(())
    }

    fn close_chapter(&mut self) -> Result<(), ImportError> {
        self.close_verse()?;
        if self.chapter > 0 {
            if self.verses.is_empty() {
                return Err(parse_error(
                    self.file,
                    format!("chapter {} has no verses", self.chapter),
                ));
            }
            self.chapters.push(ImportedChapter {
                chapter: self.chapter,
                verses: std::mem::take(&mut self.verses),
            });
        }
        Ok(())
    }

    fn block(&mut self, marker: &str) -> Result<(), ImportError> {
        self.need_space = true;
        let in_verse_text = self.open.as_ref().is_some_and(|o| o.started);
        match marker {
            "p" | "pmo" | "pc" => {
                self.mode = Mode::Body;
                self.poetry_block = false;
                self.pending_paragraph = true;
                if in_verse_text {
                    count(self.counters, "paragraph_breaks_inside_a_verse", 1);
                }
            }
            "q1" | "q2" | "qr" | "li1" | "li2" => {
                self.mode = Mode::Body;
                self.poetry_block = true;
                count(self.counters, "poetry_or_list_lines", 1);
            }
            "b" => {
                self.pending_paragraph = true;
                if self.mode == Mode::Title {
                    self.mode = Mode::Body;
                }
            }
            "d" if self.book == BookId::Ps => self.mode = Mode::PendingTitle,
            "d" => {
                // Zechariah 12:1 and its kind: a descriptive line that is verse text.
                self.mode = Mode::Body;
                self.poetry_block = false;
                self.pending_paragraph = true;
            }
            "s1" | "s2" | "r" | "ms" | "mr" | "qa" | "mt1" | "mt2" | "h" | "toc1" | "toc2"
            | "toc3" | "id" | "usfm" => {
                self.mode = Mode::Heading;
                self.pending_paragraph = true;
                count(self.counters, &format!("dropped_{marker}"), 1);
            }
            other => {
                return Err(parse_error(self.file, format!("unknown marker \\{other}")));
            }
        }
        Ok(())
    }

    fn text(&mut self, piece: &str) -> Result<(), ImportError> {
        if piece.trim().is_empty() {
            self.need_space = self.need_space || !piece.is_empty();
            return Ok(());
        }
        match self.mode {
            Mode::Heading | Mode::Outside => Ok(()),
            Mode::PendingTitle => {
                // A division line (`\d Psalms 1–41`), not a title.
                self.mode = Mode::Heading;
                count(self.counters, "dropped_d_division", 1);
                Ok(())
            }
            Mode::Title => {
                let open = self
                    .open
                    .as_mut()
                    .expect("title mode is entered with a verse");
                push_text(&mut open.title, piece, &mut self.need_space);
                Ok(())
            }
            Mode::Body => {
                let Some(open) = self.open.as_mut() else {
                    return Err(parse_error(
                        self.file,
                        format!("text outside any verse: {}", piece.trim()),
                    ));
                };
                if !open.started {
                    open.started = true;
                    open.layout.paragraph_start = self.pending_paragraph || self.verses.is_empty();
                    open.layout.poetry = self.poetry_block;
                    self.pending_paragraph = false;
                    // The separator after a title is not part of the verse.
                    self.need_space = false;
                }
                push_text(&mut open.text, piece, &mut self.need_space);
                Ok(())
            }
        }
    }

    fn verse(&mut self, text: &str) -> Result<String, ImportError> {
        self.close_verse()?;
        let (number, rest) = text
            .split_once(char::is_whitespace)
            .unwrap_or((text.trim_end(), ""));
        let number: u32 = number.parse().map_err(|_| {
            parse_error(
                self.file,
                format!("{}: \\v {number} is not one verse number", self.chapter),
            )
        })?;
        if self.chapter == 0 {
            return Err(parse_error(self.file, "a verse before any chapter"));
        }
        if self.mode == Mode::PendingTitle {
            if number != 1 {
                return Err(parse_error(
                    self.file,
                    format!("a Psalm title on verse {number} of {}", self.chapter),
                ));
            }
            self.mode = Mode::Title;
        } else if self.mode != Mode::Body {
            return Err(parse_error(
                self.file,
                format!("\\v {number} of {} outside a text block", self.chapter),
            ));
        }
        self.open = Some(Open {
            number,
            title: String::new(),
            text: String::new(),
            layout: Layout::default(),
            started: false,
        });
        self.need_space = false;
        Ok(rest.to_owned())
    }
}

/// One book's USFM into chapters.
pub fn parse_book(
    file: &str,
    source: &str,
    edition: &str,
    book: BookId,
    counters: &mut Counters,
) -> Result<Vec<ImportedChapter>, ImportError> {
    let mut reader = Reader {
        file,
        edition,
        book,
        chapter: 0,
        verses: Vec::new(),
        chapters: Vec::new(),
        open: None,
        mode: Mode::Outside,
        poetry_block: false,
        pending_paragraph: true,
        need_space: false,
        counters,
    };
    let toks = scan(source);
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Text(text) => reader.text(text)?,
            Tok::Marker("c") => {
                reader.close_chapter()?;
                let Some(Tok::Text(text)) = toks.get(i + 1) else {
                    return Err(parse_error(file, "\\c without a number"));
                };
                reader.chapter = text.trim().parse().map_err(|_| {
                    parse_error(file, format!("\\c {} is not a number", text.trim()))
                })?;
                reader.mode = Mode::Outside;
                reader.poetry_block = false;
                reader.pending_paragraph = true;
                i += 1;
            }
            Tok::Marker("v") => {
                let Some(Tok::Text(text)) = toks.get(i + 1) else {
                    return Err(parse_error(file, "\\v without a number"));
                };
                let rest = reader.verse(text)?;
                reader.text(&rest)?;
                i += 1;
            }
            Tok::Marker("f") => {
                // A note, whole: nothing up to its closing marker reaches the text.
                count(reader.counters, "dropped_footnotes", 1);
                loop {
                    i += 1;
                    match toks.get(i) {
                        Some(Tok::Marker("f*")) => break,
                        Some(Tok::Marker("fr" | "ft" | "fqa" | "fv" | "fv*" | "ref" | "ref*")) => {}
                        Some(Tok::Text(_)) => {}
                        Some(Tok::Marker(other)) => {
                            return Err(parse_error(
                                file,
                                format!("unknown note marker \\{other}"),
                            ));
                        }
                        None => return Err(parse_error(file, "an unclosed note")),
                    }
                }
            }
            Tok::Marker("wj" | "wj*") => {}
            Tok::Marker("ref" | "ref*") => {
                if reader.mode != Mode::Heading {
                    return Err(parse_error(file, "a \\ref outside a heading or note"));
                }
            }
            Tok::Marker(marker) => {
                let marker = (*marker).to_owned();
                reader.block(&marker)?;
            }
        }
        i += 1;
    }
    reader.close_chapter()?;
    Ok(reader.chapters)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Source format only: every sentence here is written for the test.
    const SAMPLE: &str = "\\id PSA test\n\n\\usfm 3.1\n\\h Psalm\n\\toc1 Psalm\n\\mt1 \n\\c 1\n\\ms BOOK I\n\\d Psalms 1–41\n\\s1 A Heading\n\\r (\\ref Matthew 5:3–12|MAT 5:3-12\\ref*)\n\\q1 \\v 1 First line\n\\q2 second line.\n\\b \n\\q1 \\v 2 Next\\f + \\fr 1:2 \\ft Or \\fqa other\\f* stanza\n\\qr Selah\n\\c 2\n\\q1 \\v 1 Untitled.\n\\c 3\n\\s1 Titled\n\\d \\v 1 A title\\f + \\fr 3:1 \\ft Note\\f* of it.\n\\b \n\\q1 The first line.\n\\q1 \\v 2 And on.\n";

    fn verses(source: &str, book: BookId) -> Vec<ImportedChapter> {
        let mut counters = Counters::new();
        parse_book("test", source, "bsb", book, &mut counters).unwrap()
    }

    #[test]
    fn notes_headings_and_markers_leave_only_verse_text() {
        let chapters = verses(SAMPLE, BookId::Ps);
        assert_eq!(chapters.len(), 3);
        let c1 = &chapters[0].verses;
        assert_eq!(c1[0].segment_id, "Ps.1.1");
        assert_eq!(c1[0].text, "First line second line.");
        assert!(c1[0].poetry && c1[0].paragraph_start);
        assert_eq!(c1[1].text, "Next stanza Selah");
        assert!(
            c1[1].poetry && c1[1].paragraph_start,
            "a \\b breaks a stanza"
        );
        for v in c1 {
            assert!(!v.text.contains('\\') && !v.text.contains('|'));
        }
    }

    #[test]
    fn a_psalm_title_becomes_verse_zero_and_the_verse_keeps_its_number() {
        let chapters = verses(SAMPLE, BookId::Ps);
        let c2 = &chapters[2].verses;
        assert_eq!(c2[0].segment_id, "Ps.3.0");
        assert_eq!(c2[0].text, "A title of it.");
        assert!(c2[0].superscription && !c2[0].poetry);
        assert_eq!(c2[0].label, "");
        assert_eq!(c2[1].segment_id, "Ps.3.1");
        assert_eq!(c2[1].text, "The first line.");
        assert!(c2[1].poetry && !c2[1].superscription);
        assert_eq!(c2[2].segment_id, "Ps.3.2");
    }

    #[test]
    fn a_title_where_the_scheme_has_no_verse_zero_stays_in_verse_one() {
        // eng numbers no verse 0 in Psalm 11.
        let source = "\\c 11\n\\d \\v 1 A title.\n\\b \n\\q1 First line.\n\\q1 \\v 2 And on.\n";
        let c = &verses(source, BookId::Ps)[0].verses;
        assert_eq!(c[0].segment_id, "Ps.11.1");
        assert_eq!(c[0].text, "A title. First line.");
        assert!(!c[0].superscription);
        assert_eq!(c[0].tokens.len(), 4);
        assert_eq!(c[1].segment_id, "Ps.11.2");
    }

    #[test]
    fn character_markers_and_closing_markers_keep_the_spacing() {
        let source = "\\c 1\n\\p \\v 1 He said, \\wj “Go.”\\wj* Then she\\f + \\ft x\\f* left, \\v 2 and came.\n\\s1 Mid\n\\p back.\n";
        let c = &verses(source, BookId::John)[0].verses;
        assert_eq!(c[0].text, "He said, “Go.” Then she left,");
        assert_eq!(c[1].text, "and came. back.");
        let ids: Vec<&str> = c[0].tokens.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids[0], "bsb:44001001001");
        assert_eq!(c[0].tokens.len(), 6);
    }

    #[test]
    fn a_descriptive_line_outside_the_psalms_is_verse_text() {
        let source = "\\c 12\n\\d \\v 1 This is the word.\n\\p Thus says.\n";
        let c = &verses(source, BookId::Zech)[0].verses;
        assert_eq!(c[0].segment_id, "Zech.12.1");
        assert_eq!(c[0].text, "This is the word. Thus says.");
    }

    #[test]
    fn unknown_markers_and_unclosed_notes_stop_the_import() {
        let mut counters = Counters::new();
        let bad = "\\c 1\n\\p \\v 1 Text \\zz odd\n";
        assert!(parse_book("t", bad, "bsb", BookId::John, &mut counters).is_err());
        let bad = "\\c 1\n\\p \\v 1 Text\\f + \\ft odd\n";
        assert!(parse_book("t", bad, "bsb", BookId::John, &mut counters).is_err());
        let bad = "\\c 1\n\\p Text before a verse\n";
        assert!(parse_book("t", bad, "bsb", BookId::John, &mut counters).is_err());
    }
}
