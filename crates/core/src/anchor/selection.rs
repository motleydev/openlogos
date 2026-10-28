//! Turning a selection over a paragraph of segments into one anchor per segment, so a
//! cross-verse selection in a renderer becomes stable per-verse anchors.

use serde::{Deserialize, Serialize};

use super::{Anchor, AnchorError, ScalarRange, TextQuoteSelector, TokenSpan, scalar_len};

/// One segment of a paragraph as laid out for selection: its text and token layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    /// The segment id, e.g. `John.3.16`.
    pub segment_id: String,
    /// The segment text, byte-exact.
    pub text: String,
    /// Tokens in text order, with ranges local to `text`; empty without a token layer.
    pub tokens: Vec<TokenSpan>,
}

/// Splits `range`, counted in scalar values over `segments` joined by `joiner`, into one
/// anchor per segment it touches.
///
/// Scalars of the joiner itself belong to no segment, so a selection that only covers a
/// joiner yields no anchors. Each anchor's range is local to its segment and its quote is
/// taken from the segment text.
pub fn split_selection(
    edition_id: &str,
    content_revision: &str,
    segments: &[Segment],
    joiner: &str,
    range: ScalarRange,
) -> Result<Vec<Anchor>, AnchorError> {
    if range.end < range.start {
        return Err(AnchorError::ReversedRange {
            start: range.start,
            end: range.end,
        });
    }
    let joiner_len = scalar_len(joiner);
    let total = segments.iter().map(|s| scalar_len(&s.text)).sum::<u32>()
        + joiner_len * (segments.len() as u32).saturating_sub(1);
    if range.end > total {
        return Err(AnchorError::OutOfBounds {
            offset: range.end,
            length: total,
        });
    }

    let mut anchors = Vec::new();
    let mut offset = 0u32;
    for segment in segments {
        let segment_end = offset + scalar_len(&segment.text);
        let start = range.start.max(offset);
        let end = range.end.min(segment_end);
        if start < end {
            let local = ScalarRange {
                start: start - offset,
                end: end - offset,
            };
            anchors.push(Anchor {
                edition_id: edition_id.to_owned(),
                content_revision: content_revision.to_owned(),
                segment_id: segment.segment_id.clone(),
                range: local,
                tokens: segment
                    .tokens
                    .iter()
                    .filter(|span| span.range.overlaps(&local))
                    .map(|span| span.id.clone())
                    .collect(),
                quote: TextQuoteSelector::from_text(&segment.text, local).ok(),
            });
        }
        offset = segment_end + joiner_len;
    }
    Ok(anchors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::TokenId;
    use crate::reference::BookId;

    fn segment(id: &str, verse: u32, text: &str) -> Segment {
        let mut tokens = Vec::new();
        let mut offset = 0u32;
        for (index, word) in text.split(' ').enumerate() {
            let len = scalar_len(word);
            tokens.push(TokenSpan {
                id: TokenId::new("t", BookId::John, 3, verse, index as u32 + 1, None).unwrap(),
                range: ScalarRange {
                    start: offset,
                    end: offset + len,
                },
            });
            offset += len + 1;
        }
        Segment {
            segment_id: id.to_owned(),
            text: text.to_owned(),
            tokens,
        }
    }

    fn paragraph() -> Vec<Segment> {
        vec![
            segment("John.3.16", 16, "For God so loved"),
            segment("John.3.17", 17, "For God did not"),
        ]
    }

    #[test]
    fn a_selection_across_the_boundary_yields_two_local_anchors() {
        // "For God so loved For God did not": "loved For" is 11..20.
        let anchors = split_selection(
            "t",
            "r1",
            &paragraph(),
            " ",
            ScalarRange { start: 11, end: 20 },
        )
        .unwrap();
        assert_eq!(anchors.len(), 2);
        assert_eq!(anchors[0].segment_id, "John.3.16");
        assert_eq!(anchors[0].range, ScalarRange { start: 11, end: 16 });
        assert_eq!(anchors[0].quote.as_ref().unwrap().exact, "loved");
        assert_eq!(anchors[0].tokens.len(), 1);
        assert_eq!(anchors[0].tokens[0].word, 4);
        assert_eq!(anchors[1].segment_id, "John.3.17");
        assert_eq!(anchors[1].range, ScalarRange { start: 0, end: 3 });
        assert_eq!(anchors[1].quote.as_ref().unwrap().exact, "For");
        assert_eq!(anchors[1].tokens[0].verse, 17);
        assert_eq!(anchors[1].content_revision, "r1");
    }

    #[test]
    fn a_selection_inside_one_segment_yields_one_anchor() {
        let anchors = split_selection(
            "t",
            "r1",
            &paragraph(),
            " ",
            ScalarRange { start: 21, end: 24 },
        )
        .unwrap();
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].segment_id, "John.3.17");
        assert_eq!(anchors[0].range, ScalarRange { start: 4, end: 7 });
        assert_eq!(anchors[0].quote.as_ref().unwrap().exact, "God");
    }

    #[test]
    fn the_joiner_belongs_to_no_segment() {
        let anchors = split_selection(
            "t",
            "r1",
            &paragraph(),
            " ",
            ScalarRange { start: 16, end: 17 },
        )
        .unwrap();
        assert!(anchors.is_empty());
        let anchors = split_selection(
            "t",
            "r1",
            &paragraph(),
            " ",
            ScalarRange { start: 16, end: 16 },
        )
        .unwrap();
        assert!(anchors.is_empty());
    }

    #[test]
    fn bounds_are_checked_over_the_whole_paragraph() {
        assert_eq!(
            split_selection(
                "t",
                "r1",
                &paragraph(),
                " ",
                ScalarRange { start: 0, end: 33 }
            ),
            Err(AnchorError::OutOfBounds {
                offset: 33,
                length: 32
            })
        );
        assert!(
            split_selection(
                "t",
                "r1",
                &paragraph(),
                " ",
                ScalarRange { start: 0, end: 32 }
            )
            .is_ok()
        );
        assert!(
            split_selection(
                "t",
                "r1",
                &paragraph(),
                " ",
                ScalarRange { start: 5, end: 2 }
            )
            .is_err()
        );
        assert!(
            split_selection("t", "r1", &[], " ", ScalarRange { start: 0, end: 0 })
                .unwrap()
                .is_empty()
        );
    }
}
