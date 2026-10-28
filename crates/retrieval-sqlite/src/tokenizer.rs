//! The one normalizer shared by indexing and query parsing (specs/19, "FTS5 with one Rust
//! tokenizer"). Display text is stored byte-exact elsewhere; this module only produces search
//! keys. The mode is versioned: a change in behaviour is a new mode name and a pack rebuild.
//!
//! Mode `ol-norm-1`:
//! - the text is decomposed (NFD) so every accent, breathing, point and cantillation mark is a
//!   separate combining character;
//! - combining marks in the Greek/Latin diacritic blocks and the Hebrew points and
//!   cantillation range U+0591–U+05C7 are dropped; the Hebrew punctuation inside that range
//!   (maqqef U+05BE, paseq U+05C0, sof pasuq U+05C3, nun hafukha U+05C6) separates words;
//! - letters are lower-cased and Greek final sigma becomes medial sigma;
//! - Greek koronis and the apostrophes that mark elision (U+1FBD, U+2019, U+02BC) are dropped
//!   so `ἀλλ’` indexes as `αλλ`;
//! - a token is a maximal run of alphanumeric characters; everything else separates.
//!
//! NFC is never applied to display text (specs/03-data-model.md, OSHB's warning): the stored
//! bytes are what the source shipped, and only this derived key is normalized.

use unicode_normalization::UnicodeNormalization;

/// The normalization mode this module implements.
pub const NORMALIZATION_MODE: &str = "ol-norm-1";

/// A token produced from a string: its normalized form and the byte range of the source
/// characters it came from, which is what FTS5's highlight and snippet functions need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Normalized search key.
    pub key: String,
    /// Byte offset of the first source character.
    pub start: usize,
    /// Byte offset just past the last source character.
    pub end: usize,
}

/// Hebrew points and cantillation that disappear from the search key.
fn is_hebrew_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0591}'
            ..='\u{05BD}'
                | '\u{05BF}'
                | '\u{05C1}'
                | '\u{05C2}'
                | '\u{05C4}'
                | '\u{05C5}'
                | '\u{05C7}'
    )
}

/// Combining diacritics (Greek accents and breathings after NFD, Latin accents) that
/// disappear from the search key.
fn is_combining_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036F}' | '\u{1AB0}'..='\u{1AFF}' | '\u{1DC0}'..='\u{1DFF}' | '\u{20D0}'..='\u{20FF}' | '\u{FE20}'..='\u{FE2F}'
    )
}

/// Elision marks that are part of a Greek word in print but not of its key.
fn is_elision_mark(c: char) -> bool {
    matches!(c, '\u{1FBD}' | '\u{2019}' | '\u{02BC}' | '\u{1FBF}')
}

/// Normalizes one character into the key alphabet: `None` drops it, `Some(None)` separates,
/// `Some(Some(k))` appends `k`.
fn classify(c: char) -> Option<Option<char>> {
    if is_hebrew_mark(c) || is_combining_mark(c) || is_elision_mark(c) {
        return None;
    }
    if c.is_alphanumeric() {
        let folded = match c {
            'ς' => 'σ',
            other => other.to_lowercase().next().unwrap_or(other),
        };
        return Some(Some(folded));
    }
    Some(None)
}

/// Splits `text` into normalized tokens with byte offsets into `text`.
pub fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut current: Option<Token> = None;
    for (start, c) in text.char_indices() {
        let end = start + c.len_utf8();
        // Decompose one character at a time so the offsets stay tied to the source.
        for d in c.nfd() {
            match classify(d) {
                None => {
                    if let Some(t) = current.as_mut() {
                        t.end = end;
                    }
                }
                Some(None) => {
                    if let Some(t) = current.take() {
                        tokens.push(t);
                    }
                }
                Some(Some(k)) => match current.as_mut() {
                    Some(t) => {
                        t.key.push(k);
                        t.end = end;
                    }
                    None => {
                        current = Some(Token {
                            key: k.to_string(),
                            start,
                            end,
                        })
                    }
                },
            }
        }
    }
    if let Some(t) = current.take() {
        tokens.push(t);
    }
    tokens
}

/// The normalized form of a whole string: its tokens joined by one space. The search key of
/// a word is `normalize(word)`.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, t) in tokenize(text).iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&t.key);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greek_loses_accents_breathings_case_and_final_sigma() {
        assert_eq!(normalize("Ἀγάπη"), "αγαπη");
        assert_eq!(normalize("λόγος"), "λογοσ");
        assert_eq!(normalize("ᾠδή"), "ωδη");
        assert_eq!(normalize("Ἰησοῦς Χριστός"), "ιησουσ χριστοσ");
        // Precomposed and decomposed input give the same key.
        assert_eq!(
            normalize("\u{1F00}\u{03B3}"),
            normalize("\u{03B1}\u{0313}\u{03B3}")
        );
    }

    #[test]
    fn greek_elision_and_punctuation_separate_or_vanish() {
        assert_eq!(normalize("ἀλλ’ οὐ"), "αλλ ου");
        assert_eq!(normalize("ὁ λόγος, καὶ·"), "ο λογοσ και");
    }

    #[test]
    fn hebrew_loses_points_and_cantillation_but_keeps_letters() {
        // Genesis 1:1 with points and accents (WLC); maqqef splits a word pair.
        let pointed = "בְּרֵאשִׁ֖ית בָּרָ֣א אֱלֹהִ֑ים אֵ֥ת הַשָּׁמַ֖יִם וְאֵ֥ת הָאָֽרֶץ׃";
        assert_eq!(normalize(pointed), "בראשית ברא אלהים את השמים ואת הארץ");
        assert_eq!(normalize("עַל־פְּנֵ֣י"), "על פני");
        // Consonantal input yields the same key as pointed input.
        assert_eq!(normalize("בְּרֵאשִׁ֖ית"), normalize("בראשית"));
    }

    #[test]
    fn latin_folds_case_and_accents() {
        assert_eq!(
            normalize("The LORD's steadfast love"),
            "the lord s steadfast love"
        );
        assert_eq!(normalize("Évangile"), "evangile");
    }

    #[test]
    fn offsets_cover_the_source_bytes_including_marks() {
        let text = "ὁ λόγος";
        let tokens = tokenize(text);
        assert_eq!(tokens.len(), 2);
        assert_eq!(&text[tokens[0].start..tokens[0].end], "ὁ");
        assert_eq!(&text[tokens[1].start..tokens[1].end], "λόγος");
        let hebrew = "אֵ֥ת הַשָּׁמַ֖יִם";
        let tokens = tokenize(hebrew);
        assert_eq!(&hebrew[tokens[0].start..tokens[0].end], "אֵ֥ת");
        assert_eq!(&hebrew[tokens[1].start..tokens[1].end], "הַשָּׁמַ֖יִם");
    }

    #[test]
    fn empty_and_mark_only_input_yield_nothing() {
        assert!(tokenize("").is_empty());
        assert!(tokenize(" , ; ").is_empty());
        assert!(tokenize("\u{05B0}\u{0591}").is_empty());
    }
}
