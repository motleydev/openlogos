//! Finding an anchor again after the text under it changed.
//!
//! The order is the one specs/03-data-model.md fixes: stable token IDs first, then the exact
//! quote with its context, and never silently attaching to the nearest different passage.
//! When more than one place matches the result is [`ReanchorResult::Ambiguous`] with every
//! candidate, and when nothing matches it is [`ReanchorResult::Orphaned`]; both wait for the
//! user. The revision-mapping step (`id_lineage`) belongs to the package crate and slots in
//! between the two steps here once it exists.

use serde::{Deserialize, Serialize};

use super::{Anchor, ScalarRange, TextQuoteSelector, TokenId, scalar_len};

/// A token and where it sits in a segment's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenSpan {
    /// The token.
    pub id: TokenId,
    /// Its extent in the segment, in scalar values.
    pub range: ScalarRange,
}

/// One revision of a segment's text, with its token layout when the edition has one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    /// The content revision identifier of the edition.
    pub content_revision: String,
    /// The segment text, byte-exact.
    pub text: String,
    /// Tokens in text order; empty when the text has no token layer.
    pub tokens: Vec<TokenSpan>,
}

/// Where an anchor ended up after [`reanchor`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "value", rename_all = "snake_case")]
pub enum ReanchorResult {
    /// Same range, same text: only the content revision was updated.
    Exact(Anchor),
    /// Found once, at a different range or under changed text.
    Moved(Anchor),
    /// Found in several places; the user chooses, or the anchor stays orphaned.
    Ambiguous(Vec<Anchor>),
    /// Not found; the original anchor is kept as is for the user.
    Orphaned,
}

/// Re-targets `anchor` from revision `old` to revision `new` of the same segment.
///
/// 1. If the anchor has token IDs and every one of them is in `new.tokens`, the new range
///    spans those tokens.
/// 2. Otherwise the quote (the stored one, or one taken from `old.text`) is searched for in
///    `new.text` with its prefix and suffix; if that finds nothing, the exact text alone is
///    searched for.
///
/// One hit is [`ReanchorResult::Exact`] when the range and text are unchanged and
/// [`ReanchorResult::Moved`] otherwise; several hits are
/// [`ReanchorResult::Ambiguous`]; none is [`ReanchorResult::Orphaned`].
#[must_use]
pub fn reanchor(anchor: &Anchor, old: &Revision, new: &Revision) -> ReanchorResult {
    let old_exact = anchor
        .quote
        .as_ref()
        .map(|q| q.exact.clone())
        .or_else(|| slice(&old.text, anchor.range));

    // 1. Stable IDs.
    if !anchor.tokens.is_empty() {
        let spans: Vec<&TokenSpan> = anchor
            .tokens
            .iter()
            .filter_map(|id| new.tokens.iter().find(|span| &span.id == id))
            .collect();
        if spans.len() == anchor.tokens.len() {
            let range = ScalarRange {
                start: spans.iter().map(|s| s.range.start).min().unwrap_or(0),
                end: spans.iter().map(|s| s.range.end).max().unwrap_or(0),
            };
            return place(anchor, new, range, old_exact.as_deref());
        }
    }

    // 2. Exact quote with context, then exact quote alone.
    let quote = match &anchor.quote {
        Some(quote) => quote.clone(),
        None => match TextQuoteSelector::from_text(&old.text, anchor.range) {
            Ok(quote) => quote,
            Err(_) => return ReanchorResult::Orphaned,
        },
    };
    if quote.exact.is_empty() {
        return ReanchorResult::Orphaned;
    }
    let mut hits = find_with_context(&new.text, &quote);
    if hits.is_empty() {
        hits = find_all(&new.text, &quote.exact)
            .into_iter()
            .map(|start| ScalarRange {
                start,
                end: start + scalar_len(&quote.exact),
            })
            .collect();
    }
    match hits.as_slice() {
        [] => ReanchorResult::Orphaned,
        [range] => place(anchor, new, *range, old_exact.as_deref()),
        _ => ReanchorResult::Ambiguous(
            hits.iter()
                .map(|range| rebuild(anchor, new, *range))
                .collect(),
        ),
    }
}

/// The anchor at `range` in `new`, as exact or moved.
fn place(
    anchor: &Anchor,
    new: &Revision,
    range: ScalarRange,
    old_exact: Option<&str>,
) -> ReanchorResult {
    let rebuilt = rebuild(anchor, new, range);
    let same_text = rebuilt
        .quote
        .as_ref()
        .is_some_and(|q| Some(q.exact.as_str()) == old_exact);
    if range == anchor.range && same_text {
        ReanchorResult::Exact(rebuilt)
    } else {
        ReanchorResult::Moved(rebuilt)
    }
}

/// A copy of `anchor` pointing at `range` in `new`, with tokens and quote taken from `new`.
fn rebuild(anchor: &Anchor, new: &Revision, range: ScalarRange) -> Anchor {
    let mut tokens: Vec<&TokenSpan> = new
        .tokens
        .iter()
        .filter(|span| span.range.overlaps(&range))
        .collect();
    tokens.sort_by_key(|span| span.range.start);
    Anchor {
        edition_id: anchor.edition_id.clone(),
        content_revision: new.content_revision.clone(),
        segment_id: anchor.segment_id.clone(),
        range,
        tokens: tokens.into_iter().map(|span| span.id.clone()).collect(),
        quote: TextQuoteSelector::from_text(&new.text, range).ok(),
    }
}

/// Ranges of `quote.exact` in `text` where the prefix and suffix also match.
fn find_with_context(text: &str, quote: &TextQuoteSelector) -> Vec<ScalarRange> {
    let needle = format!("{}{}{}", quote.prefix, quote.exact, quote.suffix);
    let prefix_len = scalar_len(&quote.prefix);
    let exact_len = scalar_len(&quote.exact);
    find_all(text, &needle)
        .into_iter()
        .map(|start| ScalarRange {
            start: start + prefix_len,
            end: start + prefix_len + exact_len,
        })
        .collect()
}

/// Scalar offsets of every occurrence of `needle` in `text`, overlapping ones included.
fn find_all(text: &str, needle: &str) -> Vec<u32> {
    if needle.is_empty() {
        return Vec::new();
    }
    text.char_indices()
        .enumerate()
        .filter(|(_, (byte, _))| text[*byte..].starts_with(needle))
        .map(|(scalar, _)| scalar as u32)
        .collect()
}

/// The text under `range`, or `None` when the range leaves the text.
fn slice(text: &str, range: ScalarRange) -> Option<String> {
    (range.start <= range.end && range.end <= scalar_len(text))
        .then(|| super::scalar_slice(text, range.start, range.end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::BookId;

    /// Tokens for a space-separated text, one id per word.
    fn tokenize(text: &str) -> Vec<TokenSpan> {
        let mut spans = Vec::new();
        let mut offset = 0u32;
        for (index, word) in text.split(' ').enumerate() {
            let len = scalar_len(word);
            spans.push(TokenSpan {
                id: TokenId::new("t", BookId::John, 3, 16, index as u32 + 1, None).unwrap(),
                range: ScalarRange {
                    start: offset,
                    end: offset + len,
                },
            });
            offset += len + 1;
        }
        spans
    }

    fn revision(id: &str, text: &str, with_tokens: bool) -> Revision {
        Revision {
            content_revision: id.to_owned(),
            text: text.to_owned(),
            tokens: if with_tokens {
                tokenize(text)
            } else {
                Vec::new()
            },
        }
    }

    fn anchor_on(
        rev: &Revision,
        start: u32,
        end: u32,
        with_tokens: bool,
        with_quote: bool,
    ) -> Anchor {
        let range = ScalarRange { start, end };
        Anchor {
            edition_id: "t".to_owned(),
            content_revision: rev.content_revision.clone(),
            segment_id: "John.3.16".to_owned(),
            range,
            tokens: if with_tokens {
                rev.tokens
                    .iter()
                    .filter(|s| s.range.overlaps(&range))
                    .map(|s| s.id.clone())
                    .collect()
            } else {
                Vec::new()
            },
            quote: with_quote.then(|| TextQuoteSelector::from_text(&rev.text, range).unwrap()),
        }
    }

    const OLD: &str = "For God so loved the world that He gave His one and only Son";

    #[test]
    fn unchanged_text_is_exact_with_or_without_tokens() {
        let old = revision("r1", OLD, true);
        let new = revision("r2", OLD, true);
        for (tokens, quote) in [(true, true), (true, false), (false, true), (false, false)] {
            let anchor = anchor_on(&old, 11, 26, tokens, quote); // "loved the world"
            match reanchor(&anchor, &old, &new) {
                ReanchorResult::Exact(found) => {
                    assert_eq!(found.range, anchor.range);
                    assert_eq!(found.content_revision, "r2");
                    assert_eq!(found.quote.as_ref().unwrap().exact, "loved the world");
                    assert_eq!(found.tokens.len(), 3);
                }
                other => panic!("tokens={tokens} quote={quote}: {other:?}"),
            }
        }
    }

    #[test]
    fn stable_token_ids_win_over_shifted_offsets() {
        let old = revision("r1", OLD, true);
        // A word inserted at the front shifts every offset; token ids follow the words.
        let new_text = format!("Indeed {OLD}");
        let mut new = revision("r2", &new_text, false);
        new.tokens = tokenize(OLD)
            .into_iter()
            .map(|mut span| {
                span.range.start += 7;
                span.range.end += 7;
                span
            })
            .collect();
        let anchor = anchor_on(&old, 11, 26, true, true);
        match reanchor(&anchor, &old, &new) {
            ReanchorResult::Moved(found) => {
                assert_eq!(found.range, ScalarRange { start: 18, end: 33 });
                assert_eq!(found.quote.unwrap().exact, "loved the world");
                assert_eq!(found.tokens, anchor.tokens);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn missing_tokens_fall_back_to_the_quote() {
        let old = revision("r1", OLD, true);
        let new_text = format!("Indeed {OLD}");
        let new = revision("r2", &new_text, false);
        let anchor = anchor_on(&old, 11, 26, true, true);
        match reanchor(&anchor, &old, &new) {
            ReanchorResult::Moved(found) => {
                assert_eq!(found.range, ScalarRange { start: 18, end: 33 });
                assert!(found.tokens.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn context_disambiguates_a_repeated_phrase() {
        let old = revision("r1", "the world; He gave the world light", false);
        let new = revision("r2", "the world; He gave the world light", false);
        // Second "the world", distinguished by " He gave " before it.
        let anchor = anchor_on(&old, 19, 28, false, true);
        match reanchor(&anchor, &old, &new) {
            ReanchorResult::Exact(found) => assert_eq!(found.range, anchor.range),
            other => panic!("{other:?}"),
        }
        let moved = revision("r3", "Yes: the world; He gave the world light", false);
        match reanchor(&anchor, &old, &moved) {
            ReanchorResult::Moved(found) => {
                assert_eq!(found.range, ScalarRange { start: 24, end: 33 })
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn identical_contexts_are_ambiguous_not_guessed() {
        let old = revision("r1", "a b a b", false);
        let anchor = Anchor {
            quote: Some(TextQuoteSelector {
                exact: "a".to_owned(),
                prefix: String::new(),
                suffix: " b".to_owned(),
            }),
            ..anchor_on(&old, 0, 1, false, false)
        };
        let new = revision("r2", "a b a b", false);
        match reanchor(&anchor, &old, &new) {
            ReanchorResult::Ambiguous(candidates) => {
                let ranges: Vec<ScalarRange> = candidates.iter().map(|c| c.range).collect();
                assert_eq!(
                    ranges,
                    vec![
                        ScalarRange { start: 0, end: 1 },
                        ScalarRange { start: 4, end: 5 }
                    ]
                );
                assert!(candidates.iter().all(|c| c.content_revision == "r2"));
            }
            other => panic!("{other:?}"),
        }
        // Overlapping occurrences count too.
        let old = revision("r1", "aaa", false);
        let anchor = anchor_on(&old, 0, 2, false, true);
        let new = revision("r2", "aaa", false);
        // The quote is "aa" with suffix "a"; in "aaaa" that matches at 0 and at 1.
        let edited = revision("r3", "aaaa", false);
        assert!(matches!(
            reanchor(&anchor, &old, &new),
            ReanchorResult::Exact(_)
        ));
        assert!(matches!(
            reanchor(&anchor, &old, &edited),
            ReanchorResult::Ambiguous(_)
        ));
    }

    #[test]
    fn removed_or_rewritten_text_is_orphaned_not_moved_to_a_neighbour() {
        let old = revision("r1", OLD, true);
        let anchor = anchor_on(&old, 11, 26, true, true); // "loved the world"
        // Rewritten with a fresh token layer: the pipeline mints new ids for new words, so
        // none of the anchor's ids survive and the quote is not there either.
        let rewritten = revision(
            "r2",
            "For God so cherished the cosmos that He gave His Son",
            false,
        );
        assert_eq!(
            reanchor(&anchor, &old, &rewritten),
            ReanchorResult::Orphaned
        );
        // A change just outside the selection leaves it exact: same range, same text.
        let near = revision(
            "r3",
            "For God so loved the worlds that He gave His one and only Son",
            false,
        );
        match reanchor(&anchor, &old, &near) {
            ReanchorResult::Exact(found) => assert_eq!(found.range, anchor.range),
            other => panic!("{other:?}"),
        }
        let gone = revision("r4", "", false);
        assert_eq!(reanchor(&anchor, &old, &gone), ReanchorResult::Orphaned);
    }

    #[test]
    fn an_anchor_with_no_quote_and_a_stale_range_is_orphaned() {
        let old = revision("r1", "short", false);
        let anchor = Anchor {
            range: ScalarRange { start: 3, end: 40 },
            ..anchor_on(&old, 0, 1, false, false)
        };
        assert_eq!(
            reanchor(&anchor, &old, &revision("r2", "short", false)),
            ReanchorResult::Orphaned
        );
        let empty = anchor_on(&old, 2, 2, false, true);
        assert_eq!(
            reanchor(&empty, &old, &revision("r2", "short", false)),
            ReanchorResult::Orphaned
        );
    }

    #[test]
    fn results_serialize_with_a_status_tag() {
        let old = revision("r1", OLD, false);
        let anchor = anchor_on(&old, 11, 26, false, true);
        let json =
            serde_json::to_string(&reanchor(&anchor, &old, &revision("r2", OLD, false))).unwrap();
        assert!(json.starts_with("{\"status\":\"exact\",\"value\":{"));
        assert_eq!(
            serde_json::to_string(&ReanchorResult::Orphaned).unwrap(),
            "{\"status\":\"orphaned\"}"
        );
    }
}
