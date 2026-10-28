//! Minting token ids for one verse.
//!
//! The rules, fixed here so every pipeline stage and every later revision agrees:
//!
//! 1. Ids are `edition:BBCCCVVVWWW[s]` ([`TokenId`]): the edition id, the USFM book number,
//!    chapter, verse and word, each three digits, and an optional morpheme suffix.
//! 2. Words are numbered from 1 in the reading order of the edition's own tokenization,
//!    counting orthographic words only; punctuation and milestones that are not tokens
//!    (maqqef, sof pasuq, footnote callers) take no number.
//! 3. A word that is one token has no suffix. A word the edition segments into `n >= 2`
//!    morphemes gets one id per morpheme with suffixes `a`, `b`, `c`, ... in reading order;
//!    at most 26 morphemes per word.
//! 4. Ids are minted once per (edition, content revision). A later revision never renumbers:
//!    it reuses an id only through a `same` row of the [`IdLineage`](super::IdLineage)
//!    between the two revisions, and new words get the next free numbers only where the
//!    lineage says `insert`.
//! 5. Upstream ids (OSHB `id`, MACULA `xml:id`) are kept beside the minted id as external
//!    keys; they never replace it.

use serde::{Deserialize, Serialize};

use crate::anchor::{AnchorError, TokenId};
use crate::reference::Reference;

/// Most morphemes one word may be segmented into (suffixes `a` to `z`).
pub const MAX_MORPHEMES: u8 = 26;

/// Errors from minting.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MintError {
    /// The reference names no verse.
    #[error("cannot mint tokens for a chapter-only reference {reference}")]
    ChapterOnly {
        /// The reference as given.
        reference: String,
    },
    /// A word with no morphemes.
    #[error("word {word} has zero morphemes")]
    ZeroMorphemes {
        /// 1-based word number.
        word: u32,
    },
    /// A word segmented into more than [`MAX_MORPHEMES`] morphemes.
    #[error("word {word} has {count} morphemes; at most {MAX_MORPHEMES} fit the suffix")]
    TooManyMorphemes {
        /// 1-based word number.
        word: u32,
        /// Morphemes given.
        count: u8,
    },
    /// A field that does not fit the id format (more than 999 words, chapters or verses).
    #[error(transparent)]
    Anchor(#[from] AnchorError),
}

/// The morpheme suffix for the 0-based morpheme `index` of a word: `a` for 0, `z` for 25.
///
/// `None` when the index is out of range.
#[must_use]
pub fn suffix_for(index: u8) -> Option<String> {
    (index < MAX_MORPHEMES).then(|| char::from(b'a' + index).to_string())
}

/// Mints the token ids of one verse of `edition`.
///
/// `morphemes` gives, for each orthographic word in reading order, how many morphemes the
/// edition segments it into. A word with one morpheme gets an id without suffix; a word with
/// `n >= 2` gets `n` ids with suffixes `a` to the `n`th letter. The ids come back in reading
/// order, so `TokenId`'s ordering agrees with text order.
pub fn mint_verse(
    edition: &str,
    reference: &Reference,
    morphemes: &[u8],
) -> Result<Vec<TokenId>, MintError> {
    let Some(verse) = reference.verse else {
        return Err(MintError::ChapterOnly {
            reference: reference.to_osis(),
        });
    };
    let mut ids = Vec::new();
    for (index, &count) in morphemes.iter().enumerate() {
        let word = index as u32 + 1;
        match count {
            0 => return Err(MintError::ZeroMorphemes { word }),
            1 => ids.push(TokenId::new(
                edition,
                reference.book,
                reference.chapter,
                verse,
                word,
                None,
            )?),
            n if n > MAX_MORPHEMES => {
                return Err(MintError::TooManyMorphemes { word, count: n });
            }
            n => {
                for morpheme in 0..n {
                    let suffix = suffix_for(morpheme).expect("checked against MAX_MORPHEMES");
                    ids.push(TokenId::new(
                        edition,
                        reference.book,
                        reference.chapter,
                        verse,
                        word,
                        Some(&suffix),
                    )?);
                }
            }
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::BookId;

    #[test]
    fn single_morpheme_words_have_no_suffix_and_segmented_words_count_letters() {
        // Genesis 1:1 in the WLC: 7 words; OSHB segments "בראשית" as prefix + noun and
        // "השמים" as article + noun. Counts here are illustrative of the shape.
        let ids = mint_verse(
            "wlc",
            &Reference::verse(BookId::Gen, 1, 1),
            &[2, 1, 1, 1, 2, 2, 2],
        )
        .unwrap();
        let text: Vec<String> = ids.iter().map(ToString::to_string).collect();
        assert_eq!(
            text,
            vec![
                "wlc:01001001001a",
                "wlc:01001001001b",
                "wlc:01001001002",
                "wlc:01001001003",
                "wlc:01001001004",
                "wlc:01001001005a",
                "wlc:01001001005b",
                "wlc:01001001006a",
                "wlc:01001001006b",
                "wlc:01001001007a",
                "wlc:01001001007b",
            ]
        );
        assert!(ids.windows(2).all(|w| w[0] < w[1]));
        for id in &ids {
            assert_eq!(TokenId::parse(&id.to_string()).unwrap(), *id);
        }
    }

    #[test]
    fn minting_rejects_what_the_format_cannot_hold() {
        let john = Reference::verse(BookId::John, 3, 16);
        assert_eq!(
            mint_verse("bsb", &Reference::chapter(BookId::Ps, 23), &[1]),
            Err(MintError::ChapterOnly {
                reference: "Ps.23".to_owned()
            })
        );
        assert_eq!(
            mint_verse("bsb", &john, &[1, 0]),
            Err(MintError::ZeroMorphemes { word: 2 })
        );
        assert_eq!(
            mint_verse("bsb", &john, &[27]),
            Err(MintError::TooManyMorphemes { word: 1, count: 27 })
        );
        assert!(matches!(
            mint_verse("bsb", &john, &vec![1u8; 1000]),
            Err(MintError::Anchor(AnchorError::FieldTooLarge { .. }))
        ));
        assert_eq!(
            mint_verse("bsb", &john, &[]).unwrap(),
            Vec::<TokenId>::new()
        );
        assert_eq!(suffix_for(0).as_deref(), Some("a"));
        assert_eq!(suffix_for(25).as_deref(), Some("z"));
        assert_eq!(suffix_for(26), None);
    }

    #[test]
    fn errors_serialize_with_a_kind_tag() {
        let json = serde_json::to_string(&MintError::ZeroMorphemes { word: 2 }).unwrap();
        assert_eq!(json, "{\"kind\":\"zero_morphemes\",\"word\":2}");
    }
}
