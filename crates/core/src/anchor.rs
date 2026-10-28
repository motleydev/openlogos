//! Stable anchors into edition text: token IDs, scalar-value ranges, quote selectors and the
//! anchor record an annotation targets (specs/03-data-model.md, "Notes and synchronization").
//!
//! Offsets are counted in Unicode scalar values so a serialized anchor means the same thing
//! on every platform; `crates/text` translates them to UTF-16 units and grapheme clusters
//! for renderers. Token IDs follow the `edition:BBCCCVVVWWW[s]` format that the pipeline mints
//! (specs/19-search-and-retrieval.md): USFM book number, chapter, verse, word, and an optional
//! morpheme suffix for Hebrew prefix segmentation.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::reference::{BookId, Reference};

pub mod reanchor;
pub mod selection;

pub use reanchor::{ReanchorResult, Revision, TokenSpan, reanchor, reanchor_across};
pub use selection::{Segment, split_selection};

/// Errors from parsing or building anchors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnchorError {
    /// Text that is not `edition:BBCCCVVVWWW[s]`.
    #[error("`{text}` is not a token id of the form edition:BBCCCVVVWWW[s]")]
    TokenIdSyntax {
        /// The text as given.
        text: String,
    },
    /// A `BB` field naming no book this crate knows.
    #[error("token id book number {number} is not a known book")]
    UnknownBook {
        /// The two-digit field as a number.
        number: u8,
    },
    /// A chapter, verse or word number that does not fit its three-digit field.
    #[error("token id {field} {value} does not fit three digits")]
    FieldTooLarge {
        /// `chapter`, `verse` or `word`.
        field: String,
        /// The value given.
        value: u32,
    },
    /// A range whose end precedes its start.
    #[error("range runs backwards: {start} to {end}")]
    ReversedRange {
        /// Start offset.
        start: u32,
        /// End offset.
        end: u32,
    },
    /// An offset past the end of the text, in scalar values.
    #[error("scalar offset {offset} is past the end of the text ({length} scalars)")]
    OutOfBounds {
        /// The offending offset.
        offset: u32,
        /// Length of the text in scalar values.
        length: u32,
    },
}

/// Largest value of a three-digit token id field.
const FIELD_MAX: u32 = 999;

/// The identity of one token occurrence: `edition:BBCCCVVVWWW[s]`.
///
/// `book` is written as the USFM book number, `chapter`, `verse` and `word` as three digits
/// each, and `suffix` (lower-case ASCII letters) distinguishes morphemes of one orthographic
/// word. IDs are minted by the pipeline and never renumbered; see [`TokenId::parse`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TokenId {
    /// Edition the token belongs to, e.g. `wlc` or `bsb`.
    pub edition: String,
    /// The book.
    pub book: BookId,
    /// Chapter, 1-based, at most 999.
    pub chapter: u32,
    /// Verse, at most 999; 0 is a Psalm superscription where the scheme numbers it so.
    pub verse: u32,
    /// Word position within the verse, 1-based, at most 999.
    pub word: u32,
    /// Morpheme suffix, e.g. `a`, when one word carries several tokens.
    pub suffix: Option<String>,
}

impl TokenId {
    /// Builds an id, rejecting fields that do not fit their width.
    pub fn new(
        edition: &str,
        book: BookId,
        chapter: u32,
        verse: u32,
        word: u32,
        suffix: Option<&str>,
    ) -> Result<TokenId, AnchorError> {
        for (field, value) in [("chapter", chapter), ("verse", verse), ("word", word)] {
            if value > FIELD_MAX {
                return Err(AnchorError::FieldTooLarge {
                    field: field.to_owned(),
                    value,
                });
            }
        }
        Ok(TokenId {
            edition: edition.to_owned(),
            book,
            chapter,
            verse,
            word,
            suffix: suffix.map(str::to_owned),
        })
    }

    /// Parses `edition:BBCCCVVVWWW[s]`, e.g. `wlc:01001001003` or `wlc:01001001003b`.
    ///
    /// The edition is everything before the last `:`; the numeric block is exactly eleven
    /// digits; the suffix is any run of lower-case ASCII letters after them.
    pub fn parse(text: &str) -> Result<TokenId, AnchorError> {
        let syntax = || AnchorError::TokenIdSyntax {
            text: text.to_owned(),
        };
        let (edition, rest) = text.rsplit_once(':').ok_or_else(syntax)?;
        if edition.is_empty() || rest.len() < 11 || !rest.is_char_boundary(11) {
            return Err(syntax());
        }
        let (digits, suffix) = rest.split_at(11);
        if !digits.bytes().all(|b| b.is_ascii_digit())
            || !suffix.bytes().all(|b| b.is_ascii_lowercase())
        {
            return Err(syntax());
        }
        let field =
            |range: std::ops::Range<usize>| digits[range].parse::<u32>().map_err(|_| syntax());
        let number = field(0..2)? as u8;
        let book = BookId::from_number(number).ok_or(AnchorError::UnknownBook { number })?;
        TokenId::new(
            edition,
            book,
            field(2..5)?,
            field(5..8)?,
            field(8..11)?,
            (!suffix.is_empty()).then_some(suffix),
        )
    }

    /// The verse this token is in.
    #[must_use]
    pub fn reference(&self) -> Reference {
        Reference::verse(self.book, self.chapter, self.verse)
    }
}

impl fmt::Display for TokenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{:02}{:03}{:03}{:03}{}",
            self.edition,
            self.book.number(),
            self.chapter,
            self.verse,
            self.word,
            self.suffix.as_deref().unwrap_or("")
        )
    }
}

/// A half-open range `[start, end)` of offsets counted in Unicode scalar values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ScalarRange {
    /// First scalar in the range.
    pub start: u32,
    /// One past the last scalar in the range.
    pub end: u32,
}

impl ScalarRange {
    /// A range, rejecting one that runs backwards.
    pub fn new(start: u32, end: u32) -> Result<ScalarRange, AnchorError> {
        if end < start {
            return Err(AnchorError::ReversedRange { start, end });
        }
        Ok(ScalarRange { start, end })
    }

    /// Number of scalars in the range.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Whether the range covers no scalars.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// Whether the two ranges share at least one scalar.
    #[must_use]
    pub fn overlaps(&self, other: &ScalarRange) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// A W3C Web Annotation `TextQuoteSelector`: the selected text with context either side.
///
/// `prefix` and `suffix` are empty when there is no context (selection at the segment edge,
/// or rights that forbid storing more text).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextQuoteSelector {
    /// The selected text, byte-exact.
    pub exact: String,
    /// Up to [`QUOTE_CONTEXT`] scalars before the selection.
    pub prefix: String,
    /// Up to [`QUOTE_CONTEXT`] scalars after the selection.
    pub suffix: String,
}

/// How much context, in scalar values, [`TextQuoteSelector::from_text`] keeps on each side.
pub const QUOTE_CONTEXT: u32 = 32;

impl TextQuoteSelector {
    /// Builds a selector for `range` within `text`, with [`QUOTE_CONTEXT`] scalars of context.
    pub fn from_text(text: &str, range: ScalarRange) -> Result<TextQuoteSelector, AnchorError> {
        let length = scalar_len(text);
        if range.end < range.start {
            return Err(AnchorError::ReversedRange {
                start: range.start,
                end: range.end,
            });
        }
        if range.end > length {
            return Err(AnchorError::OutOfBounds {
                offset: range.end,
                length,
            });
        }
        let prefix_start = range.start.saturating_sub(QUOTE_CONTEXT);
        let suffix_end = (range.end + QUOTE_CONTEXT).min(length);
        Ok(TextQuoteSelector {
            exact: scalar_slice(text, range.start, range.end),
            prefix: scalar_slice(text, prefix_start, range.start),
            suffix: scalar_slice(text, range.end, suffix_end),
        })
    }
}

/// Where an annotation points: a scalar range in one segment of one edition revision, the
/// tokens under it, and the quoted text that lets it be found again when the text changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    /// The edition.
    pub edition_id: String,
    /// The content revision of the edition the offsets were taken against.
    pub content_revision: String,
    /// The segment (verse, paragraph) within the edition.
    pub segment_id: String,
    /// The selection within the segment, in scalar values.
    pub range: ScalarRange,
    /// Token ids under the selection, in text order; empty when the text has no token layer.
    pub tokens: Vec<TokenId>,
    /// The quoted text with context, when rights permit storing it.
    pub quote: Option<TextQuoteSelector>,
}

/// Length of `text` in scalar values.
#[must_use]
pub fn scalar_len(text: &str) -> u32 {
    text.chars().count() as u32
}

/// The scalars `[start, end)` of `text`; `end` is clamped to the text.
#[must_use]
pub fn scalar_slice(text: &str, start: u32, end: u32) -> String {
    text.chars()
        .skip(start as usize)
        .take(end.saturating_sub(start) as usize)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_ids_round_trip() {
        for text in [
            "wlc:01001001003",
            "wlc:01001001003b",
            "bsb:44003016005",
            "oshb-2024:19009002001ab",
            "ns:wlc:67022021001",
        ] {
            let id = TokenId::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(id.to_string(), text);
        }
        let id = TokenId::parse("wlc:01001001003b").unwrap();
        assert_eq!(id.edition, "wlc");
        assert_eq!(id.book, BookId::Gen);
        assert_eq!((id.chapter, id.verse, id.word), (1, 1, 3));
        assert_eq!(id.suffix.as_deref(), Some("b"));
        assert_eq!(id.reference(), Reference::verse(BookId::Gen, 1, 1));
        assert_eq!(
            TokenId::parse("ns:wlc:67022021001").unwrap().edition,
            "ns:wlc"
        );
        assert_eq!(
            TokenId::parse("ns:wlc:67022021001").unwrap().book,
            BookId::Rev
        );
    }

    #[test]
    fn token_ids_reject_bad_shapes() {
        for text in [
            "",
            "wlc",
            ":01001001003",
            "wlc:0100100100",
            "wlc:010010010031",
            "wlc:01001001003B",
            "wlc:01001001003-",
            "wlc:0100100100x",
            "wlc:0100100١003",
        ] {
            assert!(
                matches!(TokenId::parse(text), Err(AnchorError::TokenIdSyntax { .. })),
                "{text:?}"
            );
        }
        assert_eq!(
            TokenId::parse("wlc:40001001001"),
            Err(AnchorError::UnknownBook { number: 40 })
        );
        assert_eq!(
            TokenId::new("wlc", BookId::Gen, 1000, 1, 1, None),
            Err(AnchorError::FieldTooLarge {
                field: "chapter".to_owned(),
                value: 1000
            })
        );
    }

    #[test]
    fn token_ids_order_by_text_position() {
        let a = TokenId::parse("wlc:01001001003").unwrap();
        let b = TokenId::parse("wlc:01001001003a").unwrap();
        let c = TokenId::parse("wlc:01001002001").unwrap();
        let d = TokenId::parse("wlc:02001001001").unwrap();
        assert!(a < b && b < c && c < d);
    }

    #[test]
    fn scalar_ranges_validate_and_measure() {
        assert!(ScalarRange::new(3, 2).is_err());
        let r = ScalarRange::new(2, 5).unwrap();
        assert_eq!(r.len(), 3);
        assert!(!r.is_empty());
        assert!(r.overlaps(&ScalarRange { start: 4, end: 9 }));
        assert!(!r.overlaps(&ScalarRange { start: 5, end: 9 }));
        assert!(ScalarRange { start: 1, end: 1 }.is_empty());
    }

    #[test]
    fn quote_selector_takes_context_in_scalars() {
        let text = "a👍🏽b אֱלֹהִ֑ים c";
        let range = ScalarRange { start: 1, end: 3 };
        let quote = TextQuoteSelector::from_text(text, range).unwrap();
        assert_eq!(quote.exact, "👍🏽");
        assert_eq!(quote.prefix, "a");
        assert_eq!(quote.suffix, "b אֱלֹהִ֑ים c");
        let long = "x".repeat(100);
        let quote =
            TextQuoteSelector::from_text(&long, ScalarRange { start: 50, end: 51 }).unwrap();
        assert_eq!(quote.prefix.len(), QUOTE_CONTEXT as usize);
        assert_eq!(quote.suffix.len(), QUOTE_CONTEXT as usize);
        assert!(matches!(
            TextQuoteSelector::from_text("abc", ScalarRange { start: 2, end: 4 }),
            Err(AnchorError::OutOfBounds {
                offset: 4,
                length: 3
            })
        ));
        assert!(TextQuoteSelector::from_text("abc", ScalarRange { start: 2, end: 1 }).is_err());
    }

    #[test]
    fn anchors_serialize_with_string_token_ids_inside() {
        let anchor = Anchor {
            edition_id: "wlc".to_owned(),
            content_revision: "4.20".to_owned(),
            segment_id: "Gen.1.1".to_owned(),
            range: ScalarRange { start: 0, end: 12 },
            tokens: vec![TokenId::parse("wlc:01001001001").unwrap()],
            quote: Some(
                TextQuoteSelector::from_text("בְּרֵאשִׁ֖ית בָּרָ֣א", ScalarRange { start: 0, end: 12 })
                    .unwrap(),
            ),
        };
        let json = serde_json::to_string(&anchor).unwrap();
        let back: Anchor = serde_json::from_str(&json).unwrap();
        assert_eq!(back, anchor);
        assert!(json.contains("\"book\":\"Gen\""));
    }
}
