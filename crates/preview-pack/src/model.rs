//! What every parser produces: an edition's books, chapters and verses, plus counters of what
//! was deliberately left out of the reading text, so the build report can say so.

use std::collections::BTreeMap;

use openlogos_core::identity::mint_verse;
use openlogos_core::reference::{BookId, Reference};
use openlogos_text::reading::{ChapterVerse, VerseToken};

/// Why an import stopped. Nothing is skipped quietly: unknown markup is an error.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// The rights record or the artifact's hashes refuse the source.
    #[error("{source_id}: {detail}")]
    Rights {
        /// `S52`.
        source_id: String,
        /// What did not match.
        detail: String,
    },
    /// The source text does not parse as this importer understands it.
    #[error("{file}: {detail}")]
    Parse {
        /// The file.
        file: String,
        /// What was found.
        detail: String,
    },
    /// The output disagrees with the source's own second form.
    #[error("{0}")]
    Oracle(String),
    /// A file could not be read or written.
    #[error("{path}: {error}")]
    Io {
        /// The path.
        path: String,
        /// The error.
        error: std::io::Error,
    },
    /// The pack writer or reader refused.
    #[error("pack: {0}")]
    Pack(String),
    /// A usage error.
    #[error("{0}")]
    Usage(String),
}

/// A shorthand for a parse error.
pub(crate) fn parse_error(file: &str, detail: impl Into<String>) -> ImportError {
    ImportError::Parse {
        file: file.to_owned(),
        detail: detail.into(),
    }
}

/// One chapter as imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedChapter {
    /// Its number.
    pub chapter: u32,
    /// Its verses in order.
    pub verses: Vec<ChapterVerse>,
}

/// One book as imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedBook {
    /// The book.
    pub book: BookId,
    /// The title its heading shows, in the edition's language.
    pub title: String,
    /// Its chapters in order.
    pub chapters: Vec<ImportedChapter>,
}

/// Counters of what the parser met and did not carry into the text, by name.
pub type Counters = BTreeMap<String, u64>;

/// Adds to a counter.
pub(crate) fn count(counters: &mut Counters, name: &str, by: u64) {
    *counters.entry(name.to_owned()).or_insert(0) += by;
}

/// A word's scalar range in a verse.
pub(crate) type Span = (u32, u32);

/// The spans of the fields of `text` that hold a letter or digit: an orthographic word with
/// its attached punctuation (`Son,`), as the conformance fixtures tokenize BSB and SBLGNT. A
/// free-standing dash takes no number (core::identity::mint rule 2).
pub(crate) fn whitespace_words(text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut start: Option<u32> = None;
    let mut has_word = false;
    let mut at = 0u32;
    for c in text.chars() {
        if c.is_whitespace() {
            if let Some(s) = start.take()
                && has_word
            {
                spans.push((s, at));
            }
            has_word = false;
        } else {
            start.get_or_insert(at);
            has_word |= c.is_alphanumeric();
        }
        at += 1;
    }
    if let Some(s) = start
        && has_word
    {
        spans.push((s, at));
    }
    spans
}

/// The verse fields a parser decides.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Layout {
    /// A paragraph starts here.
    pub paragraph_start: bool,
    /// Set as poetry.
    pub poetry: bool,
    /// A Psalm title.
    pub superscription: bool,
}

/// Builds a [`ChapterVerse`] with minted token ids (`edition:BBCCCVVVWWW`, one per word).
pub(crate) fn make_verse(
    edition: &str,
    book: BookId,
    chapter: u32,
    number: u32,
    text: String,
    words: &[Span],
    layout: Layout,
) -> Result<ChapterVerse, ImportError> {
    let reference = Reference::verse(book, chapter, number);
    let ids = mint_verse(edition, &reference, &vec![1; words.len()])
        .map_err(|e| parse_error(&reference.to_osis(), format!("cannot mint its tokens: {e}")))?;
    let tokens = ids
        .iter()
        .zip(words)
        .map(|(id, (start, end))| VerseToken {
            id: id.to_string(),
            start: *start,
            end: *end,
        })
        .collect();
    Ok(ChapterVerse {
        segment_id: reference.to_osis(),
        number,
        last_number: number,
        label: if number == 0 {
            String::new()
        } else {
            number.to_string()
        },
        text,
        paragraph_start: layout.paragraph_start,
        poetry: layout.poetry,
        superscription: layout.superscription,
        placeholder: false,
        tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_keep_attached_punctuation_and_skip_free_standing_dashes() {
        let text = "children of God — born, not";
        let spans = whitespace_words(text);
        let words: Vec<String> = spans
            .iter()
            .map(|(s, e)| {
                text.chars()
                    .skip(*s as usize)
                    .take((e - s) as usize)
                    .collect()
            })
            .collect();
        assert_eq!(words, ["children", "of", "God", "born,", "not"]);
    }
}
