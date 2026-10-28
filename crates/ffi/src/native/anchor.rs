//! `selectionToAnchors`, `anchorToUtf16` and `reanchor` over `core::anchor` and
//! `text::offsets`: a host's UTF-16 selection becomes scalar-value anchors and comes back as
//! UTF-16 for the renderer.

use openlogos_core::anchor::{self as core, TokenId, split_selection};
use openlogos_text::{OffsetError, scalar_to_utf16, utf16_to_scalar};

/// A half-open range `[start, end)` counted in Unicode scalar values (`core::anchor::ScalarRange`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ScalarRange {
    /// First scalar in the range.
    pub start: u32,
    /// One past the last scalar.
    pub end: u32,
}

/// A half-open range `[start, end)` counted in UTF-16 code units, as `NSString`, `String.utf16`
/// and Java strings count (`text::offsets::Utf16Range`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct Utf16Range {
    /// First code unit in the range.
    pub start: u32,
    /// One past the last code unit.
    pub end: u32,
}

/// A token and where it sits in a segment's text, in scalar values (`core::anchor::TokenSpan`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TokenSpan {
    /// Token id, `edition:BBCCCVVVWWW[s]`.
    pub id: String,
    /// Its extent in the segment.
    pub range: ScalarRange,
}

/// One segment (verse, paragraph) of an edition as laid out for selection (`core::anchor::Segment`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Segment {
    /// The segment id, e.g. `Gen.1.1`.
    pub segment_id: String,
    /// The segment text, byte-exact.
    pub text: String,
    /// Tokens in text order, local to `text`; empty without a token layer.
    pub tokens: Vec<TokenSpan>,
}

/// Several segments shown as one run of text, joined by `joiner`, with the edition and
/// revision the text came from. A selection over it is measured on the joined text.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Paragraph {
    /// The edition, e.g. `wlc`.
    pub edition_id: String,
    /// The content revision of the edition the segments were taken from.
    pub content_revision: String,
    /// Text between consecutive segments in the rendered run (a space, a newline, `""`).
    pub joiner: String,
    /// The segments in display order.
    pub segments: Vec<Segment>,
}

/// The selected text with context either side (`core::anchor::TextQuoteSelector`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TextQuoteSelector {
    /// The selected text, byte-exact.
    pub exact: String,
    /// Up to 32 scalars before the selection.
    pub prefix: String,
    /// Up to 32 scalars after the selection.
    pub suffix: String,
}

/// Where an annotation points (`core::anchor::Anchor`): a scalar range in one segment of one
/// edition revision, the token ids under it, and the quote that finds it again.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Anchor {
    /// The core API version that produced this value.
    pub api_version: String,
    /// The edition.
    pub edition_id: String,
    /// The content revision the offsets were taken against.
    pub content_revision: String,
    /// The segment within the edition.
    pub segment_id: String,
    /// The selection within the segment, in scalar values.
    pub range: ScalarRange,
    /// Token ids under the selection, in text order; empty without a token layer.
    pub tokens: Vec<String>,
    /// The quoted text with context, when rights permit storing it.
    pub quote: Option<TextQuoteSelector>,
}

/// One revision of a segment's text with its token layout (`core::anchor::Revision`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Revision {
    /// The content revision identifier.
    pub content_revision: String,
    /// The segment text, byte-exact.
    pub text: String,
    /// Tokens in text order; empty without a token layer.
    pub tokens: Vec<TokenSpan>,
}

/// Where an anchor ended up after [`reanchor`] (`core::anchor::ReanchorResult`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ReanchorResult {
    /// Same range, same text: only the content revision changed.
    Exact {
        /// The anchor on the new revision.
        anchor: Anchor,
    },
    /// Found once, at a different range or under changed text.
    Moved {
        /// The anchor on the new revision.
        anchor: Anchor,
    },
    /// Found in several places; the user chooses, or the anchor stays orphaned.
    Ambiguous {
        /// Every candidate, in text order.
        candidates: Vec<Anchor>,
    },
    /// Not found; the original anchor is kept as is for the user.
    Orphaned,
}

/// The unit an offset is counted in, for error reports (`text::offsets::OffsetUnit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum OffsetUnit {
    /// Unicode scalar values.
    Scalar,
    /// UTF-16 code units.
    Utf16,
    /// Extended grapheme clusters.
    Grapheme,
}

/// Why a selection, anchor or token id is unusable (`core::anchor::AnchorError` and
/// `text::offsets::OffsetError` together).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum AnchorError {
    /// Text that is not `edition:BBCCCVVVWWW[s]`.
    #[error("`{text}` is not a token id of the form edition:BBCCCVVVWWW[s]")]
    TokenIdSyntax {
        /// The text as given.
        text: String,
    },
    /// A token id `BB` field naming no known book.
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
    /// An offset past the end of the text.
    #[error("{unit:?} offset {offset} is past the end of the text ({length} {unit:?} units)")]
    OutOfBounds {
        /// The offending offset.
        offset: u32,
        /// Length of the text in the same unit.
        length: u32,
        /// The unit both numbers are counted in.
        unit: OffsetUnit,
    },
    /// A UTF-16 offset between the two halves of a surrogate pair.
    #[error("UTF-16 offset {offset} splits a surrogate pair")]
    SplitsSurrogatePair {
        /// The offending offset.
        offset: u32,
    },
    /// A scalar offset inside a grapheme cluster.
    #[error("scalar offset {offset} is not on a grapheme cluster boundary")]
    NotGraphemeBoundary {
        /// The offending offset.
        offset: u32,
    },
}

impl From<core::AnchorError> for AnchorError {
    fn from(error: core::AnchorError) -> Self {
        match error {
            core::AnchorError::TokenIdSyntax { text } => AnchorError::TokenIdSyntax { text },
            core::AnchorError::UnknownBook { number } => AnchorError::UnknownBook { number },
            core::AnchorError::FieldTooLarge { field, value } => {
                AnchorError::FieldTooLarge { field, value }
            }
            core::AnchorError::ReversedRange { start, end } => {
                AnchorError::ReversedRange { start, end }
            }
            core::AnchorError::OutOfBounds { offset, length } => AnchorError::OutOfBounds {
                offset,
                length,
                unit: OffsetUnit::Scalar,
            },
        }
    }
}

impl From<OffsetError> for AnchorError {
    fn from(error: OffsetError) -> Self {
        match error {
            OffsetError::OutOfBounds {
                offset,
                length,
                unit,
            } => AnchorError::OutOfBounds {
                offset,
                length,
                unit: match unit {
                    openlogos_text::OffsetUnit::Scalar => OffsetUnit::Scalar,
                    openlogos_text::OffsetUnit::Utf16 => OffsetUnit::Utf16,
                    openlogos_text::OffsetUnit::Grapheme => OffsetUnit::Grapheme,
                },
            },
            OffsetError::SplitsSurrogatePair { offset } => {
                AnchorError::SplitsSurrogatePair { offset }
            }
            OffsetError::NotGraphemeBoundary { offset } => {
                AnchorError::NotGraphemeBoundary { offset }
            }
            OffsetError::Reversed { start, end } => AnchorError::ReversedRange { start, end },
        }
    }
}

impl From<core::ScalarRange> for ScalarRange {
    fn from(range: core::ScalarRange) -> Self {
        ScalarRange {
            start: range.start,
            end: range.end,
        }
    }
}

impl From<ScalarRange> for core::ScalarRange {
    fn from(range: ScalarRange) -> Self {
        core::ScalarRange {
            start: range.start,
            end: range.end,
        }
    }
}

impl From<openlogos_text::Utf16Range> for Utf16Range {
    fn from(range: openlogos_text::Utf16Range) -> Self {
        Utf16Range {
            start: range.start,
            end: range.end,
        }
    }
}

impl From<Utf16Range> for openlogos_text::Utf16Range {
    fn from(range: Utf16Range) -> Self {
        openlogos_text::Utf16Range {
            start: range.start,
            end: range.end,
        }
    }
}

impl From<core::TextQuoteSelector> for TextQuoteSelector {
    fn from(quote: core::TextQuoteSelector) -> Self {
        TextQuoteSelector {
            exact: quote.exact,
            prefix: quote.prefix,
            suffix: quote.suffix,
        }
    }
}

impl From<TextQuoteSelector> for core::TextQuoteSelector {
    fn from(quote: TextQuoteSelector) -> Self {
        core::TextQuoteSelector {
            exact: quote.exact,
            prefix: quote.prefix,
            suffix: quote.suffix,
        }
    }
}

impl TryFrom<TokenSpan> for core::TokenSpan {
    type Error = AnchorError;

    fn try_from(span: TokenSpan) -> Result<Self, Self::Error> {
        Ok(core::TokenSpan {
            id: TokenId::parse(&span.id)?,
            range: span.range.into(),
        })
    }
}

fn token_spans(spans: Vec<TokenSpan>) -> Result<Vec<core::TokenSpan>, AnchorError> {
    spans.into_iter().map(core::TokenSpan::try_from).collect()
}

impl TryFrom<Segment> for core::Segment {
    type Error = AnchorError;

    fn try_from(segment: Segment) -> Result<Self, Self::Error> {
        Ok(core::Segment {
            segment_id: segment.segment_id,
            text: segment.text,
            tokens: token_spans(segment.tokens)?,
        })
    }
}

impl TryFrom<Revision> for core::Revision {
    type Error = AnchorError;

    fn try_from(revision: Revision) -> Result<Self, Self::Error> {
        Ok(core::Revision {
            content_revision: revision.content_revision,
            text: revision.text,
            tokens: token_spans(revision.tokens)?,
        })
    }
}

impl From<core::Anchor> for Anchor {
    fn from(anchor: core::Anchor) -> Self {
        Anchor {
            api_version: openlogos_core::api_version().to_owned(),
            edition_id: anchor.edition_id,
            content_revision: anchor.content_revision,
            segment_id: anchor.segment_id,
            range: anchor.range.into(),
            tokens: anchor.tokens.iter().map(ToString::to_string).collect(),
            quote: anchor.quote.map(TextQuoteSelector::from),
        }
    }
}

impl TryFrom<Anchor> for core::Anchor {
    type Error = AnchorError;

    fn try_from(anchor: Anchor) -> Result<Self, Self::Error> {
        Ok(core::Anchor {
            edition_id: anchor.edition_id,
            content_revision: anchor.content_revision,
            segment_id: anchor.segment_id,
            range: anchor.range.into(),
            tokens: anchor
                .tokens
                .iter()
                .map(|id| TokenId::parse(id))
                .collect::<Result<_, _>>()?,
            quote: anchor.quote.map(core::TextQuoteSelector::from),
        })
    }
}

impl From<core::ReanchorResult> for ReanchorResult {
    fn from(result: core::ReanchorResult) -> Self {
        match result {
            core::ReanchorResult::Exact(anchor) => ReanchorResult::Exact {
                anchor: anchor.into(),
            },
            core::ReanchorResult::Moved(anchor) => ReanchorResult::Moved {
                anchor: anchor.into(),
            },
            core::ReanchorResult::Ambiguous(anchors) => ReanchorResult::Ambiguous {
                candidates: anchors.into_iter().map(Anchor::from).collect(),
            },
            core::ReanchorResult::Orphaned => ReanchorResult::Orphaned,
        }
    }
}

/// Turns a UTF-16 selection over a paragraph's rendered text into one anchor per segment it
/// touches (`core::anchor::split_selection`).
///
/// `selection` is measured on the segments' texts joined by `paragraph.joiner`, which is how
/// the renderer laid them out. Each anchor's range is local to its segment and in scalar
/// values; scalars of the joiner belong to no segment, so a selection that only covers a
/// joiner yields no anchors. A UTF-16 offset inside a surrogate pair is an error.
#[uniffi::export]
pub fn selection_to_anchors(
    paragraph: Paragraph,
    selection: Utf16Range,
) -> Result<Vec<Anchor>, AnchorError> {
    let joined = paragraph
        .segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join(&paragraph.joiner);
    let range = utf16_to_scalar(&joined, selection.into())?;
    let segments = paragraph
        .segments
        .into_iter()
        .map(core::Segment::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let anchors = split_selection(
        &paragraph.edition_id,
        &paragraph.content_revision,
        &segments,
        &paragraph.joiner,
        range,
    )?;
    Ok(anchors.into_iter().map(Anchor::from).collect())
}

/// The anchor's range as UTF-16 code units of `text`, the text of the anchor's segment, for
/// a renderer to highlight (`text::offsets::scalar_to_utf16`).
#[uniffi::export]
pub fn anchor_to_utf16(anchor: Anchor, text: String) -> Result<Utf16Range, AnchorError> {
    Ok(scalar_to_utf16(&text, anchor.range.into())?.into())
}

/// Re-targets `anchor` from revision `old` to revision `new` of the same segment
/// (`core::anchor::reanchor`): stable token ids first, then the exact quote with context.
/// The error is a token id that does not parse; not finding the anchor is `Orphaned`.
#[uniffi::export]
pub fn reanchor(
    anchor: Anchor,
    old: Revision,
    new: Revision,
) -> Result<ReanchorResult, AnchorError> {
    let anchor = core::Anchor::try_from(anchor)?;
    let old = core::Revision::try_from(old)?;
    let new = core::Revision::try_from(new)?;
    Ok(core::reanchor(&anchor, &old, &new).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEN_1_1: &str = "בְּרֵאשִׁ֖ית בָּרָ֣א אֱלֹהִ֑ים אֵ֥ת הַשָּׁמַ֖יִם וְאֵ֥ת הָאָֽרֶץ׃";

    fn gen_1_1() -> Segment {
        let spans = [
            (1, 0, 12),
            (2, 13, 20),
            (3, 21, 30),
            (4, 31, 34),
            (5, 35, 46),
            (6, 47, 52),
            (7, 53, 65),
        ];
        Segment {
            segment_id: "Gen.1.1".into(),
            text: GEN_1_1.into(),
            tokens: spans
                .iter()
                .map(|&(word, start, end)| TokenSpan {
                    id: format!("wlc:01001001{word:03}"),
                    range: ScalarRange { start, end },
                })
                .collect(),
        }
    }

    fn paragraph(segments: Vec<Segment>, joiner: &str) -> Paragraph {
        Paragraph {
            edition_id: "wlc".into(),
            content_revision: "oshb-wlc-4.20".into(),
            joiner: joiner.into(),
            segments,
        }
    }

    #[test]
    fn utf16_selection_round_trips_through_an_anchor() {
        let anchors = selection_to_anchors(
            paragraph(vec![gen_1_1()], ""),
            Utf16Range { start: 21, end: 30 },
        )
        .unwrap();
        assert_eq!(anchors.len(), 1);
        let anchor = &anchors[0];
        assert_eq!(anchor.api_version, openlogos_core::API_VERSION);
        assert_eq!(anchor.segment_id, "Gen.1.1");
        assert_eq!(anchor.range, ScalarRange { start: 21, end: 30 });
        assert_eq!(anchor.tokens, vec!["wlc:01001001003"]);
        assert_eq!(anchor.quote.as_ref().unwrap().exact, "אֱלֹהִ֑ים");
        assert_eq!(
            anchor_to_utf16(anchor.clone(), GEN_1_1.into()).unwrap(),
            Utf16Range { start: 21, end: 30 }
        );
    }

    #[test]
    fn surrogate_pairs_are_counted_and_guarded() {
        // "a👍🏽b": the emoji is 2 scalars and 4 UTF-16 units.
        let segment = Segment {
            segment_id: "note".into(),
            text: "a👍🏽b".into(),
            tokens: vec![],
        };
        let anchors = selection_to_anchors(
            paragraph(vec![segment.clone()], ""),
            Utf16Range { start: 1, end: 5 },
        )
        .unwrap();
        assert_eq!(anchors[0].range, ScalarRange { start: 1, end: 3 });
        assert_eq!(
            anchor_to_utf16(anchors[0].clone(), "a👍🏽b".into()).unwrap(),
            Utf16Range { start: 1, end: 5 }
        );
        assert_eq!(
            selection_to_anchors(
                paragraph(vec![segment], ""),
                Utf16Range { start: 1, end: 2 }
            ),
            Err(AnchorError::SplitsSurrogatePair { offset: 2 })
        );
    }

    #[test]
    fn a_selection_across_segments_yields_one_anchor_each() {
        let second = Segment {
            segment_id: "Gen.1.2".into(),
            text: "וְהָאָ֗רֶץ".into(),
            tokens: vec![TokenSpan {
                id: "wlc:01001002001".into(),
                range: ScalarRange { start: 0, end: 10 },
            }],
        };
        let anchors = selection_to_anchors(
            paragraph(vec![gen_1_1(), second], " "),
            Utf16Range { start: 53, end: 70 },
        )
        .unwrap();
        assert_eq!(anchors.len(), 2);
        assert_eq!(anchors[0].tokens, vec!["wlc:01001001007"]);
        assert_eq!(anchors[1].range, ScalarRange { start: 0, end: 4 });
        assert_eq!(anchors[1].tokens, vec!["wlc:01001002001"]);
    }

    #[test]
    fn reanchoring_follows_token_ids_and_reports_bad_ids() {
        let old = gen_1_1();
        let anchors = selection_to_anchors(
            paragraph(vec![old.clone()], ""),
            Utf16Range { start: 21, end: 30 },
        )
        .unwrap();
        let old_revision = Revision {
            content_revision: "r1".into(),
            text: old.text.clone(),
            tokens: old.tokens.clone(),
        };
        // A word inserted at the front shifts every token by 4 scalars.
        let new_revision = Revision {
            content_revision: "r2".into(),
            text: format!("xxx {}", old.text),
            tokens: old
                .tokens
                .iter()
                .map(|span| TokenSpan {
                    id: span.id.clone(),
                    range: ScalarRange {
                        start: span.range.start + 4,
                        end: span.range.end + 4,
                    },
                })
                .collect(),
        };
        match reanchor(
            anchors[0].clone(),
            old_revision.clone(),
            new_revision.clone(),
        )
        .unwrap()
        {
            ReanchorResult::Moved { anchor } => {
                assert_eq!(anchor.range, ScalarRange { start: 25, end: 34 });
                assert_eq!(anchor.content_revision, "r2");
            }
            other => panic!("expected Moved, got {other:?}"),
        }
        let mut bad = anchors[0].clone();
        bad.tokens = vec!["not-a-token".into()];
        assert_eq!(
            reanchor(bad, old_revision, new_revision),
            Err(AnchorError::TokenIdSyntax {
                text: "not-a-token".into()
            })
        );
    }
}
