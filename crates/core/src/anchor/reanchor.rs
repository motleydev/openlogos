//! Finding an anchor again after the text under it changed.
//!
//! The order is the one specs/03-data-model.md fixes: stable token IDs first, then the
//! revision mapping (`id_lineage`), then the exact quote with its context, and never silently
//! attaching to the nearest different passage. When more than one place matches the result
//! is [`ReanchorResult::Ambiguous`] with every candidate, and when nothing matches it is
//! [`ReanchorResult::Orphaned`]; both wait for the user. [`reanchor`] runs without a lineage
//! table; [`reanchor_across`] takes the table a pack ships between two revisions.

use serde::{Deserialize, Serialize};

use super::{Anchor, ScalarRange, TextQuoteSelector, TokenId, scalar_len};
use crate::identity::IdLineage;
use crate::versification::MappingStatus;

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
    reanchor_across(anchor, old, new, None)
}

/// Re-targets `anchor` from revision `old` to revision `new`, using the `id_lineage` table
/// between the two revisions when the pack ships one.
///
/// 1. Stable ids, as [`reanchor`].
/// 2. Revision mapping: if `lineage` maps every one of the anchor's token ids to successors
///    (a `same`, `split` or `merge` row each) and every successor is in `new.tokens`, the
///    new range spans the successors. If the lineage says every one of the anchor's tokens
///    was deleted, the anchor is [`ReanchorResult::Orphaned`] without trying the quote: the
///    same words elsewhere would be a different passage. A lineage that mentions only some
///    of the tokens, or none, falls through.
/// 3. Exact quote with context, then exact quote alone, as [`reanchor`].
#[must_use]
pub fn reanchor_across(
    anchor: &Anchor,
    old: &Revision,
    new: &Revision,
    lineage: Option<&IdLineage>,
) -> ReanchorResult {
    let old_exact = anchor
        .quote
        .as_ref()
        .map(|q| q.exact.clone())
        .or_else(|| slice(&old.text, anchor.range));

    // 1. Stable IDs. The anchor may cover part of its tokens or run past them into
    // untokenized text (punctuation, sof pasuq), so its offsets relative to the token span
    // in the old revision are carried onto the token span in the new one.
    if !anchor.tokens.is_empty()
        && let Some(new_span) = token_span(&anchor.tokens, &new.tokens)
    {
        let range = match token_span(&anchor.tokens, &old.tokens) {
            Some(old_span) => shift_within(anchor.range, old_span, new_span, scalar_len(&new.text)),
            None => new_span,
        };
        return place(anchor, new, range, old_exact.as_deref());
    }

    // 2. Revision mapping through the lineage table.
    if !anchor.tokens.is_empty()
        && let Some(lineage) = lineage
    {
        let mapped = lineage.map_tokens(&anchor.tokens);
        if mapped.status == MappingStatus::Exact
            && let Some(new_span) = token_span(&mapped.tokens, &new.tokens)
        {
            let range = match token_span(&anchor.tokens, &old.tokens) {
                Some(old_span) => {
                    shift_within(anchor.range, old_span, new_span, scalar_len(&new.text))
                }
                None => new_span,
            };
            return place(anchor, new, range, old_exact.as_deref());
        }
        if mapped.deleted.len() == anchor.tokens.len() {
            return ReanchorResult::Orphaned;
        }
    }

    // 3. Exact quote with context, then exact quote alone.
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

/// The span from the first to the last of `ids` in `layout`, or `None` unless every id is there.
fn token_span(ids: &[TokenId], layout: &[TokenSpan]) -> Option<ScalarRange> {
    let spans: Vec<&TokenSpan> = ids
        .iter()
        .map(|id| layout.iter().find(|span| &span.id == id))
        .collect::<Option<Vec<_>>>()?;
    Some(ScalarRange {
        start: spans.iter().map(|s| s.range.start).min()?,
        end: spans.iter().map(|s| s.range.end).max()?,
    })
}

/// `range`, positioned relative to `old_span`, moved to the same position relative to
/// `new_span` and clamped to a text of `length` scalars.
fn shift_within(
    range: ScalarRange,
    old_span: ScalarRange,
    new_span: ScalarRange,
    length: u32,
) -> ScalarRange {
    let lead = i64::from(range.start) - i64::from(old_span.start);
    let trail = i64::from(old_span.end) - i64::from(range.end);
    let start = (i64::from(new_span.start) + lead).clamp(0, i64::from(length)) as u32;
    let end = (i64::from(new_span.end) - trail).clamp(i64::from(start), i64::from(length)) as u32;
    ScalarRange { start, end }
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

    /// A lineage from r1 to r2 in which the word at `index` (1-based) was split into two
    /// morphemes `a` and `b`, the word at `deleted` was deleted, and the rest are `same`.
    fn lineage(words: u32, split: u32, deleted: u32) -> IdLineage {
        use crate::identity::{LineageEntry, LineageKind};
        let id = |word: u32, suffix: Option<&str>| {
            TokenId::new("t", BookId::John, 3, 16, word, suffix).unwrap()
        };
        let mut entries = Vec::new();
        for word in 1..=words {
            entries.push(if word == split {
                LineageEntry {
                    kind: LineageKind::Split,
                    from: vec![id(word, None)],
                    to: vec![id(word, Some("a")), id(word, Some("b"))],
                }
            } else if word == deleted {
                LineageEntry {
                    kind: LineageKind::Delete,
                    from: vec![id(word, None)],
                    to: Vec::new(),
                }
            } else {
                LineageEntry {
                    kind: LineageKind::Same,
                    from: vec![id(word, None)],
                    to: vec![id(word, None)],
                }
            });
        }
        IdLineage {
            edition_id: "t".to_owned(),
            from_revision: "r1".to_owned(),
            to_revision: "r2".to_owned(),
            entries,
        }
    }

    #[test]
    fn lineage_maps_split_tokens_before_the_quote_is_tried() {
        // r1: "so loved the world"; r2 splits "loved" into "lov" + "ed" with new ids
        // 2a and 2b, and the text changes so the quote alone would fail.
        let old = revision("r1", "so loved the world", true);
        let mut new = revision("r2", "so lov ed the cosmos", false);
        let mk = |word, suffix: Option<&str>, start, end| TokenSpan {
            id: TokenId::new("t", BookId::John, 3, 16, word, suffix).unwrap(),
            range: ScalarRange { start, end },
        };
        new.tokens = vec![
            mk(1, None, 0, 2),
            mk(2, Some("a"), 3, 6),
            mk(2, Some("b"), 7, 9),
            mk(3, None, 10, 13),
            mk(4, None, 14, 20),
        ];
        let anchor = anchor_on(&old, 3, 8, true, true); // "loved"
        assert_eq!(anchor.tokens.len(), 1);
        // Without the lineage the ids are gone and the quote has changed: orphaned.
        assert_eq!(reanchor(&anchor, &old, &new), ReanchorResult::Orphaned);
        let table = lineage(4, 2, 0);
        match reanchor_across(&anchor, &old, &new, Some(&table)) {
            ReanchorResult::Moved(found) => {
                assert_eq!(found.range, ScalarRange { start: 3, end: 9 });
                assert_eq!(found.quote.unwrap().exact, "lov ed");
                assert_eq!(found.tokens.len(), 2);
                assert_eq!(found.content_revision, "r2");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn lineage_deletion_orphans_even_when_the_words_recur() {
        // r1: "a b a b" tokens 1..4; the note is on the first "a" (token 1), which r2
        // deletes while the same word still occurs later.
        let old = revision("r1", "a b a b", true);
        let mut new = revision("r2", "b a b", false);
        let mk = |word, start, end| TokenSpan {
            id: TokenId::new("t", BookId::John, 3, 16, word, None).unwrap(),
            range: ScalarRange { start, end },
        };
        new.tokens = vec![mk(2, 0, 1), mk(3, 2, 3), mk(4, 4, 5)];
        let anchor = anchor_on(&old, 0, 1, true, true);
        let table = lineage(4, 0, 1);
        assert_eq!(
            reanchor_across(&anchor, &old, &new, Some(&table)),
            ReanchorResult::Orphaned
        );
        // Without the lineage the quote step finds the later "a" with context " b" twice
        // in "a b a b"-shaped text, or once here: that single hit is a different passage,
        // which only the lineage can rule out.
        assert!(matches!(
            reanchor(&anchor, &old, &new),
            ReanchorResult::Moved(_) | ReanchorResult::Ambiguous(_)
        ));
    }

    #[test]
    fn a_partial_lineage_falls_through_to_the_quote() {
        let old = revision("r1", OLD, true);
        let new_text = format!("Indeed {OLD}");
        let new = revision("r2", &new_text, false); // no token layer in r2
        let anchor = anchor_on(&old, 11, 26, true, true); // "loved the world", 3 tokens
        // The lineage knows one of the three tokens and nothing about the others.
        let mut table = lineage(1, 0, 0);
        table.entries[0].from = vec![anchor.tokens[0].clone()];
        table.entries[0].to = vec![anchor.tokens[0].clone()];
        match reanchor_across(&anchor, &old, &new, Some(&table)) {
            ReanchorResult::Moved(found) => {
                assert_eq!(found.range, ScalarRange { start: 18, end: 33 });
                assert!(found.tokens.is_empty());
            }
            other => panic!("{other:?}"),
        }
        // Same ids present in the new layout: step 1 wins and the lineage is not consulted.
        let same = revision("r2", OLD, true);
        assert!(matches!(
            reanchor_across(&anchor, &old, &same, Some(&lineage(12, 0, 4))),
            ReanchorResult::Exact(_)
        ));
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

    #[test]
    fn stable_ids_keep_offsets_inside_and_beyond_the_tokens() {
        let old = revision("r1", "so loved the world, that", true);
        let new_text = "Indeed so loved the world, that";
        let mut new = revision("r2", new_text, false);
        new.tokens = tokenize("so loved the world, that")
            .into_iter()
            .map(|mut span| {
                span.range.start += 7;
                span.range.end += 7;
                span
            })
            .collect();
        // "oved the worl": starts inside "loved" and ends inside "world,".
        let inside = anchor_on(&old, 4, 17, true, true);
        assert_eq!(inside.tokens.len(), 3);
        match reanchor(&inside, &old, &new) {
            ReanchorResult::Moved(found) => {
                assert_eq!(found.range, ScalarRange { start: 11, end: 24 });
                assert_eq!(found.quote.unwrap().exact, "oved the worl");
            }
            other => panic!("{other:?}"),
        }
        // "world, " runs one scalar past its token into the following space.
        let beyond = anchor_on(&old, 13, 20, true, true);
        assert_eq!(beyond.tokens.len(), 1);
        match reanchor(&beyond, &old, &new) {
            ReanchorResult::Moved(found) => {
                assert_eq!(found.range, ScalarRange { start: 20, end: 27 });
                assert_eq!(found.quote.unwrap().exact, "world, ");
            }
            other => panic!("{other:?}"),
        }
        // Without an old token layout the whole token span is used.
        let no_layout = revision("r1", "so loved the world, that", false);
        match reanchor(&inside, &no_layout, &new) {
            ReanchorResult::Moved(found) => {
                assert_eq!(found.range, ScalarRange { start: 10, end: 26 })
            }
            other => panic!("{other:?}"),
        }
    }
}
