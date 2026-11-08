//! Joining MACULA's words to the text packs' tokens. Every source word ends joined to exactly
//! one token or listed; every token ends analysed or listed with a reason. Nothing is matched
//! by similarity: a join holds on position plus a stated letter rule, and each rule is a
//! class counted in the report with every place it applies.
//!
//! **Greek.** MACULA's `ref` (`MAT 1:1!3`) and the pack's token id (`sblgnt:41001001003`)
//! number the same SBLGNT words, so a word joins the token of its verse and number. The
//! letters then decide the class: `exact`; `punctuation` (the pack token carries attached
//! punctuation or brackets: `Ἀβραάμ.`, `⟦Πάντα`); `elision` (the pack prints the elision mark
//! U+02BC inside the token, MACULA in `after`); `normalization` (equal once both are NFC: John
//! 7:53–8:11); `diacritics` (equal letters, another accent or case: the two SBLGNT releases
//! differ). Letters that differ beyond that are `surface_differs`: not joined, listed.
//!
//! **Hebrew.** MACULA numbers ketiv and qere words alike and analyses the qere, the pack
//! prints the ketiv; so the join walks each verse against the OSHB markup the pack was built
//! from ([`super::ketiv`]). A written word joins its token when the letters are equal
//! (`exact`); the words of a qere join the ketiv token they are read for (`qere`), the first of
//! a run when one note reads for several (`qere_for_ketiv_run`, the others are listed); a qere
//! with nothing written joins no token (`qere_without_ketiv`, listed); a ketiv not read has no
//! analysis (`ketiv_not_read`). A verse whose words do not walk is `unaligned`: listed, not
//! joined. Prefixes and suffixes stay separate morphemes with their scalar range inside the
//! token; the article a prefix's vowel implies is a morpheme with no letters.

use std::collections::BTreeMap;

use openlogos_core::reference::BookId;
use openlogos_retrieval::VerseId;
use openlogos_retrieval_sqlite::morphology::{MorphemeRow, Reading};
use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

use super::ketiv::VerseReadings;
use super::macula::MaculaWord;
use crate::model::ImportError;

/// One token of a text pack: its word number and letters.
pub type PackToken = (u32, String);

/// The text pack's tokens by verse.
pub type PackVerses = BTreeMap<VerseId, Vec<PackToken>>;

/// A place a class applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Place {
    /// The pack token, if any.
    pub token: Option<String>,
    /// MACULA's `ref`, or the verse for a token MACULA has no word for.
    pub source: String,
    /// MACULA's letters.
    pub source_text: String,
    /// The pack token's letters.
    pub pack_text: String,
}

/// What a join did, for the report.
#[derive(Debug, Clone, Default, Serialize)]
pub struct JoinReport {
    /// `sblgnt`, `wlc`.
    pub edition: String,
    /// Words in the source.
    pub source_words: u64,
    /// Morphemes in the source (rows of the TSV).
    pub source_morphemes: u64,
    /// Morphemes with no letters of their own (an implied article).
    pub implied_morphemes: u64,
    /// Tokens in the text pack.
    pub pack_tokens: u64,
    /// Source words joined to exactly one token.
    pub words_joined: u64,
    /// Source words joined to no token, each listed.
    pub words_not_joined: u64,
    /// Pack tokens with an analysis.
    pub tokens_analyzed: u64,
    /// Pack tokens without one, each listed with its reason.
    pub tokens_without_analysis: u64,
    /// Source words per join class.
    pub word_classes: BTreeMap<String, u64>,
    /// Tokens without analysis per reason.
    pub absent_reasons: BTreeMap<String, u64>,
    /// Morphemes per language tag.
    pub morphemes_by_language: BTreeMap<String, u64>,
    /// Tokens holding two or more morphemes.
    pub segmented_tokens: u64,
    /// Every place of every class but `exact`, by class or reason.
    pub places: BTreeMap<String, Vec<Place>>,
}

/// A join's output: the layer's rows, the tokens without analysis, the report.
#[derive(Debug, Default)]
pub struct Joined {
    /// Morphemes in reading order.
    pub rows: Vec<MorphemeRow>,
    /// `(verse, word, reason)`.
    pub absent: Vec<(VerseId, u32, String)>,
    /// The counts and places.
    pub report: JoinReport,
}

fn verse_id(book: BookId, chapter: u32, verse: u32) -> VerseId {
    VerseId::new(
        book.number(),
        u16::try_from(chapter).unwrap_or(0),
        u16::try_from(verse).unwrap_or(0),
    )
}

fn token_id(edition: &str, verse: VerseId, word: u32) -> String {
    format!(
        "{edition}:{:02}{:03}{:03}{word:03}",
        verse.book(),
        verse.chapter(),
        verse.verse()
    )
}

fn is_elision(c: char) -> bool {
    matches!(c, '\u{02BC}' | '\u{2019}' | '\u{1FBD}' | '\u{1FBF}')
}

fn is_letter(c: char) -> bool {
    !is_elision(c) && (c.is_alphanumeric() || unicode_normalization::char::is_combining_mark(c))
}

/// The letters and marks of a token, without punctuation, brackets or elision marks.
fn letters(text: &str) -> String {
    text.chars().filter(|c| is_letter(*c)).collect()
}

/// Base letters, lower case: no accents, breathings or case.
fn base(text: &str) -> String {
    text.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// The class of a Greek word against its token, or `None` when the letters differ.
fn greek_class(source: &str, pack: &str) -> Option<&'static str> {
    if source == pack {
        return Some("exact");
    }
    let (s, p) = (letters(source), letters(pack));
    if s == p {
        let punctuation = pack.chars().filter(|c| !is_letter(*c));
        return Some(
            if punctuation.clone().any(is_elision)
                && pack.chars().filter(|c| !is_letter(*c)).all(is_elision)
            {
                "elision"
            } else {
                "punctuation"
            },
        );
    }
    if s.nfc().collect::<String>() == p.nfc().collect::<String>() {
        return Some("normalization");
    }
    if base(&s) == base(&p) {
        return Some("diacritics");
    }
    None
}

/// The scalar range of a token's letters: the token without leading and trailing
/// punctuation.
fn letter_span(pack: &str) -> (u32, u32) {
    let chars: Vec<char> = pack.chars().collect();
    let first = chars.iter().position(|c| is_letter(*c)).unwrap_or(0);
    let last = chars
        .iter()
        .rposition(|c| is_letter(*c))
        .map_or(chars.len(), |i| i + 1);
    (first as u32, last as u32)
}

/// One source word's fate.
struct Assigned<'a> {
    word: &'a MaculaWord,
    verse: VerseId,
    /// Pack word number; 0 for none.
    token: u32,
    reading: Reading,
    class: &'static str,
    /// Scalar ranges of its morphemes inside the token, when written there.
    spans: Option<Vec<(u32, u32)>>,
}

fn place(edition: &str, a: &Assigned<'_>, pack: &PackVerses) -> Place {
    Place {
        token: (a.token != 0).then(|| token_id(edition, a.verse, a.token)),
        source: a.word.reference.clone(),
        source_text: a.word.text(),
        pack_text: pack
            .get(&a.verse)
            .and_then(|tokens| tokens.iter().find(|(w, _)| *w == a.token))
            .map(|(_, t)| t.clone())
            .unwrap_or_default(),
    }
}

/// Turns assignments into rows, absences and the report.
fn finish(
    edition: &str,
    assigned: Vec<Assigned<'_>>,
    mut absent: Vec<(VerseId, u32, String)>,
    pack: &PackVerses,
) -> Result<Joined, ImportError> {
    let mut report = JoinReport {
        edition: edition.to_owned(),
        pack_tokens: pack.values().map(|t| t.len() as u64).sum(),
        ..JoinReport::default()
    };
    // Morphemes per token, and the order check: a token's morphemes are consecutive and
    // verses never go back.
    let mut per_token: BTreeMap<(VerseId, u32), u8> = BTreeMap::new();
    let mut last: Option<(VerseId, u32)> = None;
    let mut closed: std::collections::BTreeSet<(VerseId, u32)> = std::collections::BTreeSet::new();
    for a in &assigned {
        if let Some((v, _)) = last
            && a.verse < v
        {
            return Err(ImportError::Oracle(format!(
                "{} comes after a later verse",
                a.word.reference
            )));
        }
        if a.token != 0 {
            let key = (a.verse, a.token);
            if last != Some(key) && !closed.insert(key) {
                return Err(ImportError::Oracle(format!(
                    "{}: the morphemes of {} are not consecutive",
                    a.word.reference,
                    token_id(edition, a.verse, a.token)
                )));
            }
            let n = per_token.entry(key).or_default();
            *n = n
                .checked_add(a.word.morphs.len() as u8)
                .filter(|n| *n <= 26)
                .ok_or_else(|| {
                    ImportError::Oracle(format!("{}: more than 26 morphemes", a.word.reference))
                })?;
        }
        last = Some((a.verse, a.token));
    }
    let mut index: BTreeMap<(VerseId, u32), u8> = BTreeMap::new();
    let mut rows = Vec::new();
    for a in &assigned {
        report.source_words += 1;
        *report.word_classes.entry(a.class.to_owned()).or_default() += 1;
        if a.token == 0 {
            report.words_not_joined += 1;
        } else {
            report.words_joined += 1;
        }
        if a.class != "exact" {
            report
                .places
                .entry(a.class.to_owned())
                .or_default()
                .push(place(edition, a, pack));
        }
        let morphemes = if a.token == 0 {
            a.word.morphs.len() as u8
        } else {
            per_token[&(a.verse, a.token)]
        };
        for (i, m) in a.word.morphs.iter().enumerate() {
            report.source_morphemes += 1;
            if m.text.is_empty() {
                report.implied_morphemes += 1;
            }
            *report
                .morphemes_by_language
                .entry(m.language.tag().to_owned())
                .or_default() += 1;
            let morpheme = if a.token == 0 {
                i as u8
            } else {
                let next = index.entry((a.verse, a.token)).or_default();
                let this = *next;
                *next += 1;
                this
            };
            rows.push(MorphemeRow {
                verse: a.verse,
                word: a.token,
                morpheme,
                morphemes,
                word_start: i == 0,
                span: a.spans.as_ref().map(|s| s[i]),
                surface: m.text.clone(),
                reading: a.reading,
                join: a.class.to_owned(),
                external_id: m.xml_id.clone(),
                language: m.language,
                lemma: m.lemma.clone(),
                strong: m.strong.clone(),
                pos: m.pos.clone(),
                morph: m.morph.clone(),
                parsing: m.parsing.clone(),
                gloss: m.gloss.clone(),
                english: m.english.clone(),
                transliteration: m.transliteration.clone(),
            });
        }
    }
    report.segmented_tokens = per_token.values().filter(|n| **n >= 2).count() as u64;
    // Tokens no word joined and no reason covers yet.
    for (verse, tokens) in pack {
        for (word, text) in tokens {
            if per_token.contains_key(&(*verse, *word)) {
                report.tokens_analyzed += 1;
            } else if !absent.iter().any(|(v, w, _)| v == verse && w == word) {
                absent.push((*verse, *word, "not_analyzed".to_owned()));
                let _ = text;
            }
        }
    }
    absent.sort();
    for (verse, word, reason) in &absent {
        report.tokens_without_analysis += 1;
        *report.absent_reasons.entry(reason.clone()).or_default() += 1;
        report
            .places
            .entry(format!("absent:{reason}"))
            .or_default()
            .push(Place {
                token: Some(token_id(edition, *verse, *word)),
                source: String::new(),
                source_text: String::new(),
                pack_text: pack
                    .get(verse)
                    .and_then(|t| t.iter().find(|(w, _)| w == word))
                    .map(|(_, t)| t.clone())
                    .unwrap_or_default(),
            });
    }
    Ok(Joined {
        rows,
        absent,
        report,
    })
}

/// Joins MACULA Greek to the SBLGNT pack.
pub fn join_greek(
    edition: &str,
    words: &[MaculaWord],
    pack: &PackVerses,
) -> Result<Joined, ImportError> {
    let mut assigned = Vec::with_capacity(words.len());
    for word in words {
        let verse = verse_id(word.book, word.chapter, word.verse);
        if word.morphs.len() != 1 {
            return Err(ImportError::Oracle(format!(
                "{}: a Greek word of {} rows",
                word.reference,
                word.morphs.len()
            )));
        }
        let token = pack
            .get(&verse)
            .and_then(|tokens| tokens.iter().find(|(w, _)| *w == word.number));
        let (token, class, spans) = match token {
            None => (0, "no_pack_token", None),
            Some((number, text)) => match greek_class(&word.text(), text) {
                Some(class) => (*number, class, Some(vec![letter_span(text)])),
                None => (0, "surface_differs", None),
            },
        };
        assigned.push(Assigned {
            word,
            verse,
            token,
            reading: Reading::Written,
            class,
            spans,
        });
    }
    let mut absent = Vec::new();
    for a in &assigned {
        if a.class == "surface_differs" {
            absent.push((a.verse, a.word.number, "surface_differs".to_owned()));
        }
    }
    finish(edition, assigned, absent, pack)
}

/// What the walk expects next in a Hebrew verse.
enum Expect<'a> {
    Written(u32),
    Qere { ketiv: &'a [u32], text: &'a str },
    QereOnly(&'a str),
}

/// A verse the OSHB has no words for (never in practice; the pack check below stops it).
static NO_READINGS: VerseReadings = VerseReadings {
    words: Vec::new(),
    qere: Vec::new(),
};

/// The qere read where nothing is written, after `after` main-stream words.
fn push_only<'a>(reading: &'a VerseReadings, after: u32, expect: &mut Vec<Expect<'a>>) {
    for q in reading
        .qere
        .iter()
        .filter(|q| q.ketiv.is_empty() && q.after == after)
    {
        for w in &q.words {
            expect.push(Expect::QereOnly(w));
        }
    }
}

/// Joins MACULA Hebrew to the WLC pack, walking each verse against the OSHB markup.
pub fn join_hebrew(
    edition: &str,
    words: &[MaculaWord],
    pack: &PackVerses,
    readings: &BTreeMap<VerseId, VerseReadings>,
) -> Result<Joined, ImportError> {
    // The source's words by verse, in file order.
    let mut by_verse: BTreeMap<VerseId, Vec<&MaculaWord>> = BTreeMap::new();
    for word in words {
        by_verse
            .entry(verse_id(word.book, word.chapter, word.verse))
            .or_default()
            .push(word);
    }
    let mut assigned = Vec::with_capacity(words.len());
    let mut absent = Vec::new();
    for (verse, source) in &by_verse {
        let tokens = pack.get(verse).map(Vec::as_slice).unwrap_or_default();
        let reading = readings.get(verse).unwrap_or(&NO_READINGS);
        // The OSHB main stream is the pack's text: same count, same letters, or the pack was
        // not built from these bytes.
        if reading.words.len() != tokens.len()
            || reading
                .words
                .iter()
                .zip(tokens)
                .any(|(w, (_, t))| w.text != *t)
        {
            return Err(ImportError::Oracle(format!(
                "verse {}: the OSHB words are not the WLC pack's tokens",
                verse.0
            )));
        }
        let mut expect: Vec<Expect<'_>> = Vec::new();
        push_only(reading, 0, &mut expect);
        for (i, w) in reading.words.iter().enumerate() {
            let n = i as u32 + 1;
            if !w.ketiv {
                expect.push(Expect::Written(n));
            } else if let Some(q) = reading.qere.iter().find(|q| q.ketiv.first() == Some(&n)) {
                if q.words.is_empty() {
                    absent.push((*verse, n, "ketiv_not_read".to_owned()));
                }
                for text in &q.words {
                    expect.push(Expect::Qere {
                        ketiv: &q.ketiv,
                        text,
                    });
                }
                for other in q.ketiv.iter().skip(1) {
                    absent.push((*verse, *other, "ketiv_read_with_previous_token".to_owned()));
                }
            }
            push_only(reading, n, &mut expect);
        }
        let walks = expect.len() == source.len()
            && expect.iter().zip(source).all(|(e, w)| match e {
                Expect::Written(n) => tokens[*n as usize - 1].1 == w.text(),
                Expect::Qere { text, .. } | Expect::QereOnly(text) => *text == w.text(),
            });
        if !walks {
            for w in source {
                assigned.push(Assigned {
                    word: w,
                    verse: *verse,
                    token: 0,
                    reading: Reading::Written,
                    class: "unaligned",
                    spans: None,
                });
            }
            for (n, _) in tokens {
                if !absent.iter().any(|(v, w, _)| v == verse && w == n) {
                    absent.push((*verse, *n, "unaligned".to_owned()));
                }
            }
            continue;
        }
        for (e, w) in expect.iter().zip(source) {
            let (token, reading, class, spans) = match e {
                Expect::Written(n) => {
                    let mut at = 0u32;
                    let spans = w
                        .morphs
                        .iter()
                        .map(|m| {
                            let start = at;
                            at += m.text.chars().count() as u32;
                            (start, at)
                        })
                        .collect();
                    (*n, Reading::Written, "exact", Some(spans))
                }
                Expect::Qere { ketiv, .. } => (
                    ketiv[0],
                    Reading::Qere,
                    if ketiv.len() > 1 {
                        "qere_for_ketiv_run"
                    } else {
                        "qere"
                    },
                    None,
                ),
                Expect::QereOnly(_) => (0, Reading::QereWithoutKetiv, "qere_without_ketiv", None),
            };
            assigned.push(Assigned {
                word: w,
                verse: *verse,
                token,
                reading,
                class,
                spans,
            });
        }
    }
    for verse in pack.keys().filter(|v| !by_verse.contains_key(v)) {
        for (n, _) in &pack[verse] {
            absent.push((*verse, *n, "verse_not_in_source".to_owned()));
        }
    }
    finish(edition, assigned, absent, pack)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greek_classes_follow_the_letters() {
        assert_eq!(greek_class("λόγος", "λόγος"), Some("exact"));
        assert_eq!(greek_class("Ἀβραάμ", "Ἀβραάμ."), Some("punctuation"));
        assert_eq!(greek_class("Πάντα", "⟦Πάντα"), Some("punctuation"));
        assert_eq!(greek_class("κατ", "κατʼ"), Some("elision"));
        assert_eq!(greek_class("δι", "[διʼ"), Some("punctuation"));
        assert_eq!(greek_class("πάλιν", "πα\u{301}λιν"), Some("normalization"));
        assert_eq!(greek_class("ὁδηγοί", "ὁδηγοὶ"), Some("diacritics"));
        assert_eq!(greek_class("ἰδὼν", "Ἰδὼν"), Some("diacritics"));
        assert_eq!(greek_class("λόγος", "λόγου"), None);
        assert_eq!(letter_span("(καὶ"), (1, 4));
        assert_eq!(letter_span("κατʼ"), (0, 3));
    }
}
