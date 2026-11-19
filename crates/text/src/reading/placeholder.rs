//! The placeholder pack (B078): a whole edition generated in-process from the canon table,
//! so a reader can scroll every chapter before the edition's text is installed (B071). Each
//! verse is a short line that says what it is, in the edition's script, never anything that
//! reads as Scripture: `Placeholder verse — John 3:15`, `Κείμενο κράτησης θέσης — 3:15`,
//! `טקסט ממלא מקום — 1:3`; every one carries `placeholder: true`. Verses the host has real
//! text for (the conformance fixtures) are overlaid with it and `placeholder: false`.
//!
//! The layout is a placeholder's too: a paragraph every five verses in prose, one line per
//! verse in the poetic books (Job 3:1–42:6, Psalms, Proverbs, Song of Songs, Lamentations),
//! and the Psalm titles of the scheme marked as superscriptions (verse 0 in `eng`; in `org`
//! the verses up to the one an `eng` title maps to).

use std::collections::HashMap;

use openlogos_core::canon;
use openlogos_core::reference::{BookId, Reference};
use openlogos_core::versification::SchemeId;
use serde::{Deserialize, Serialize};

use super::{BookShape, ChapterVerse, Edition, ReadingError, TextPack, VerseToken, book_title};

/// Real text for one verse of a placeholder edition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlayVerse {
    /// The segment id (`John.3.16`).
    pub segment_id: String,
    /// The text, byte-exact.
    pub text: String,
    /// Its tokens.
    #[serde(default)]
    pub tokens: Vec<VerseToken>,
}

/// A generated edition over the canon table.
#[derive(Debug, Clone)]
pub struct PlaceholderPack {
    edition: Edition,
    books: Vec<BookShape>,
    overlay: HashMap<String, OverlayVerse>,
}

impl PlaceholderPack {
    /// A placeholder edition of `books` (in the edition's book order) with real text for the
    /// overlay verses. `edition.placeholder` is set. Fails when the canon table does not
    /// know the edition's scheme.
    pub fn new(
        mut edition: Edition,
        books: &[BookId],
        overlay: Vec<OverlayVerse>,
    ) -> Result<PlaceholderPack, ReadingError> {
        let scheme = SchemeId::new(&edition.scheme);
        if !canon::knows(&scheme) {
            return Err(ReadingError::InvalidPack {
                detail: format!(
                    "the canon table has no scheme {} (it has {})",
                    edition.scheme,
                    canon::SCHEMES.join(", ")
                ),
            });
        }
        edition.placeholder = true;
        Ok(PlaceholderPack {
            books: super::canon_books(&scheme, books, edition.book_order),
            edition,
            overlay: overlay
                .into_iter()
                .map(|v| (v.segment_id.clone(), v))
                .collect(),
        })
    }
}

/// Whether a verse is set as poetry when its source says nothing about layout: the poetic
/// books (Job 3:1–42:6, Psalms, Proverbs, Song of Songs, Lamentations).
#[must_use]
pub fn is_poetry(book: BookId, chapter: u32, verse: u32) -> bool {
    match book {
        BookId::Ps | BookId::Prov | BookId::Song | BookId::Lam => true,
        BookId::Job => (3..=41).contains(&chapter) || (chapter == 42 && verse <= 6),
        _ => false,
    }
}

/// The last verse of a Psalm's title in the scheme: 0 in `eng` when it numbers one, the
/// verse an `eng` title maps to in `org`; `None` for a chapter without one.
#[must_use]
pub fn title_end(scheme: &SchemeId, book: BookId, chapter: u32) -> Option<u32> {
    if book != BookId::Ps {
        return None;
    }
    let eng = SchemeId::new("eng");
    match scheme.as_str() {
        "eng" => (canon::first_verse(&eng, book, chapter) == Some(0)).then_some(0),
        _ => {
            if canon::first_verse(&eng, book, chapter) != Some(0) {
                return None;
            }
            let mapped = canon::map(&Reference::verse(book, chapter, 0), &eng, scheme);
            mapped
                .targets
                .first()
                .filter(|t| t.chapter == chapter)
                .and_then(|t| t.verse)
        }
    }
}

/// The placeholder line for one verse in a language: English unless the language is Greek
/// (`grc`, `el`) or Hebrew/Aramaic (`hbo`, `he`, `heb`, `arc`).
#[must_use]
pub fn placeholder_text(language: &str, book: BookId, chapter: u32, verse: u32) -> String {
    let base = language.split('-').next().unwrap_or(language);
    let place = if book.is_single_chapter() {
        verse.to_string()
    } else {
        format!("{chapter}:{verse}")
    };
    match base {
        "grc" | "el" => format!("Κείμενο κράτησης θέσης — {place}"),
        "hbo" | "he" | "heb" | "arc" => format!("טקסט ממלא מקום — {place}"),
        _ if verse == 0 => format!("Placeholder title — Psalm {chapter}"),
        _ if book.is_single_chapter() => {
            format!("Placeholder verse — {} {verse}", book_title(book))
        }
        _ => format!("Placeholder verse — {} {chapter}:{verse}", book_title(book)),
    }
}

impl TextPack for PlaceholderPack {
    fn edition(&self) -> &Edition {
        &self.edition
    }

    fn books(&self) -> &[BookShape] {
        &self.books
    }

    fn verses(&self, book: BookId, chapter: u32) -> Result<Vec<ChapterVerse>, ReadingError> {
        let scheme = SchemeId::new(&self.edition.scheme);
        let shape = self
            .books
            .iter()
            .find(|b| b.book == book)
            .and_then(|b| b.chapters.iter().find(|c| c.chapter == chapter))
            .ok_or_else(|| ReadingError::UnknownChapter {
                resource_id: self.edition.resource_id.clone(),
                chapter_key: super::chapter_key(book, chapter),
            })?;
        let title = title_end(&scheme, book, chapter);
        Ok((shape.first_verse..=shape.last_verse)
            .map(|verse| {
                let segment_id = Reference::verse(book, chapter, verse).to_osis();
                let superscription = title.is_some_and(|end| verse <= end);
                let poetry = !superscription && is_poetry(book, chapter, verse);
                let after_title = title.map_or(shape.first_verse, |end| end + 1);
                let paragraph_start = superscription && verse == shape.first_verse
                    || verse == after_title
                    || (!poetry && verse > after_title && (verse - after_title).is_multiple_of(5));
                let (text, tokens, placeholder) = match self.overlay.get(&segment_id) {
                    Some(real) => (real.text.clone(), real.tokens.clone(), false),
                    None => (
                        placeholder_text(&self.edition.language, book, chapter, verse),
                        Vec::new(),
                        true,
                    ),
                };
                ChapterVerse {
                    label: if verse == 0 {
                        String::new()
                    } else {
                        verse.to_string()
                    },
                    segment_id,
                    number: verse,
                    last_number: verse,
                    text,
                    paragraph_start,
                    poetry,
                    superscription,
                    placeholder,
                    tokens,
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reading::{Granularity, ReadingLibrary};
    use openlogos_core::canon::BookOrder;

    fn edition(id: &str, language: &str, scheme: &str) -> Edition {
        Edition {
            resource_id: id.to_owned(),
            content_revision: "0".to_owned(),
            title: id.to_uppercase(),
            name: id.to_owned(),
            language: language.to_owned(),
            direction: "ltr".to_owned(),
            scheme: scheme.to_owned(),
            book_order: BookOrder::Protestant,
            placeholder: false,
            attribution: None,
        }
    }

    fn library() -> ReadingLibrary {
        let mut library = ReadingLibrary::new();
        let overlay = vec![OverlayVerse {
            segment_id: "John.3.16".to_owned(),
            text: "For God so loved the world".to_owned(),
            tokens: Vec::new(),
        }];
        library.insert(Box::new(
            PlaceholderPack::new(edition("bsb", "en", "eng"), &BookId::all(), overlay).unwrap(),
        ));
        library.insert(Box::new(
            PlaceholderPack::new(
                edition("wlc", "hbo", "org"),
                &canon::old_testament(),
                Vec::new(),
            )
            .unwrap(),
        ));
        library
    }

    #[test]
    fn a_whole_bible_streams_across_books_and_testaments() {
        let library = library();
        let n = |key: &str| library.chapter_neighbors("bsb", key).unwrap();
        assert_eq!(n("Gen.1").prev, None);
        assert_eq!(n("Mal.4").next.as_deref(), Some("Matt.1"));
        assert_eq!(n("Matt.1").prev.as_deref(), Some("Mal.4"));
        assert_eq!(n("John.21").next.as_deref(), Some("Acts.1"));
        assert_eq!(n("Rev.22").next, None);
        assert_eq!(n("Obad.1").next.as_deref(), Some("Jonah.1"));
        let wlc = library.chapter_neighbors("wlc", "Mal.3").unwrap();
        assert_eq!(wlc.next, None);
        assert!(library.chapter_neighbors("wlc", "Mal.4").is_err());
    }

    #[test]
    fn chapters_carry_placeholders_and_the_overlay() {
        let library = library();
        let john = library.chapter_content("bsb", "John.3").unwrap();
        assert_eq!(john.verses.len(), 36);
        assert_eq!(john.heading.book_title, None);
        assert_eq!(john.heading.chapter_label, "Chapter 3");
        assert_eq!(john.heading.short_label, "Jn 3");
        let v15 = &john.verses[14];
        assert_eq!(v15.text, "Placeholder verse — John 3:15");
        assert!(v15.placeholder);
        let v16 = john.verse_holding(16).unwrap();
        assert!(!v16.placeholder);
        assert_eq!(v16.text, "For God so loved the world");
        let gen1 = library.chapter_content("wlc", "Gen.1").unwrap();
        assert_eq!(gen1.heading.book_title.as_deref(), Some("Genesis"));
        assert_eq!(gen1.verses[2].text, "טקסט ממלא מקום — 1:3");
        let ps3 = library.chapter_content("bsb", "Ps.3").unwrap();
        assert!(ps3.verses[0].superscription);
        assert_eq!(ps3.verses[0].segment_id, "Ps.3.0");
        assert_eq!(ps3.verses[0].text, "Placeholder title — Psalm 3");
        assert!(ps3.verses[1].poetry && ps3.verses[1].paragraph_start);
        let ps51 = library.chapter_content("wlc", "Ps.51").unwrap();
        let titles: Vec<u32> = ps51
            .verses
            .iter()
            .filter(|v| v.superscription)
            .map(|v| v.number)
            .collect();
        assert_eq!(titles, [1, 2]);
    }

    #[test]
    fn locate_finds_ranges_chapters_and_books() {
        let library = library();
        let l = library.locate("bsb", "Jn 3:16-18").unwrap();
        assert_eq!(l.chapter_key, "John.3");
        assert_eq!(l.segment_id, "John.3.16");
        assert_eq!(l.highlight_range.as_deref(), Some("John.3.16-John.3.18"));
        let book = library.locate("bsb", "John").unwrap();
        assert_eq!(book.granularity, Granularity::Book);
        assert_eq!(book.segment_id, "John.1.1");
        assert_eq!(book.highlight_range, None);
        let ps = library.locate("bsb", "Ps 23").unwrap();
        assert_eq!(ps.segment_id, "Ps.23.1");
        assert_eq!(library.locate("bsb", "Ps 3").unwrap().segment_id, "Ps.3.0");
        assert!(matches!(
            library.locate("bsb", "John 3:37"),
            Err(ReadingError::VerseOutOfRange { last_verse: 36, .. })
        ));
        assert!(matches!(
            library.locate("wlc", "John 3:16"),
            Err(ReadingError::BookNotInEdition { .. })
        ));
        assert!(matches!(
            library.locate("bsb", "Hezekiah 1"),
            Err(ReadingError::InvalidReference { .. })
        ));
    }
}
