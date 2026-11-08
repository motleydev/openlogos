//! Joining Clear's BSB target words to the BSB text pack's tokens.
//!
//! The pack's BSB is the publisher's `bsb.txt` verbatim, tokenized as whitespace fields with
//! attached punctuation (B071 decision 6: `Son,` is one token; `ground—because` is one
//! token). Clear tokenizes the BSB into words and punctuation marks with a no-space flag, so
//! both sides are the same letters cut differently, and Clear's text may be another BSB
//! revision.
//!
//! The rule, per chapter: write both sides as one stream of characters with every whitespace
//! removed (the pack's from its verse text, verse after verse; Clear's from its tokens in id
//! order, so a Psalm title Clear numbers verse 0 and the pack prints at the start of verse 1
//! is the same run of letters). Where the streams are equal, every character of one is the
//! character of the other at the same place. Where they are not, a shortest edit script
//! (Myers) between the streams pairs the characters that stay; nothing is paired by
//! similarity. A Clear word is joined when every one of its characters is paired, all inside
//! one pack token and contiguous there; its scalar range in that token is kept. Any other
//! Clear word is `text_differs` and listed; any pack token holding no joined word is listed
//! with a reason. A joined word's class says how it sits in its token:
//!
//! - `exact`: the token is the word.
//! - `punctuation`: the token is the word plus attached punctuation (`Son,`).
//! - `shares_token`: the token holds two or more Clear words (`ground—because`).
//! - `token_has_other_letters`: the token holds letters no joined Clear word covers.
//! - `other_verse`: the word's Clear verse is not the pack verse holding it (a Psalm title
//!   the pack keeps in verse 1).
//! - `in_changed_verse`: joined, in a verse whose letters differ between the two texts.
//!
//! The first that applies, from the bottom of the list up, is the word's class.

use std::collections::{BTreeMap, BTreeSet};

use openlogos_retrieval::VerseId;
use serde::Serialize;

use super::clear::TargetToken;

/// One verse of the pack: its text and its tokens' `(word, start, end)` scalar ranges.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackVerse {
    /// The verse text.
    pub text: String,
    /// Tokens: word number and scalar range in the text.
    pub tokens: Vec<(u32, u32, u32)>,
}

/// A pack's verses in order.
pub type PackText = BTreeMap<VerseId, PackVerse>;

/// Where one Clear word went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinedTarget {
    /// Index into the Clear target tokens.
    pub clear: usize,
    /// The pack verse and word, with the word's scalar range in the token; `None` when not
    /// joined.
    pub place: Option<(VerseId, u32, u32, u32)>,
    /// The class.
    pub class: &'static str,
}

/// A place in the join report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TargetPlace {
    /// The pack token, if any.
    pub token: Option<String>,
    /// Clear's id, or the pack verse for a token with no Clear word.
    pub clear: String,
    /// Clear's letters.
    pub clear_text: String,
    /// The pack token's letters.
    pub pack_text: String,
}

/// What the target join did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TargetReport {
    /// Chapters compared.
    pub chapters: u64,
    /// Chapters whose letters were equal.
    pub chapters_equal: u64,
    /// Pack verses whose letters differ from Clear's (each in `places.changed_verse`).
    pub changed_verses: u64,
    /// Clear tokens, words and punctuation.
    pub clear_tokens: u64,
    /// Clear words (tokens not excluded).
    pub clear_words: u64,
    /// Clear words joined to a pack token.
    pub words_joined: u64,
    /// Clear words not joined, each listed.
    pub words_not_joined: u64,
    /// Pack tokens.
    pub pack_tokens: u64,
    /// Pack tokens holding at least one joined word.
    pub tokens_with_words: u64,
    /// Pack tokens holding none, each listed with its reason.
    pub tokens_without_words: u64,
    /// Pack tokens holding two or more joined words.
    pub tokens_with_several_words: u64,
    /// Clear words per class.
    pub word_classes: BTreeMap<String, u64>,
    /// Pack tokens without words per reason.
    pub absent_reasons: BTreeMap<String, u64>,
    /// Every place of every class but `exact`, `punctuation` and `shares_token` (which are
    /// counted), and every token without words.
    pub places: BTreeMap<String, Vec<TargetPlace>>,
}

/// The join's output.
#[derive(Debug, Default)]
pub struct TargetJoin {
    /// One per Clear word, in Clear's order.
    pub words: Vec<JoinedTarget>,
    /// Pack tokens without a word: `(verse, word, reason)`.
    pub absent: Vec<(VerseId, u32, &'static str)>,
    /// Counts and places.
    pub report: TargetReport,
}

/// Pairs equal characters of `a` and `b` along a shortest edit script (Myers, O((N+M)D)).
/// Returns, per index of `a`, the index of `b` it stays as.
#[must_use]
pub fn diff_pairs(a: &[char], b: &[char]) -> Vec<Option<usize>> {
    let mut pairs = vec![None; a.len()];
    if a.is_empty() || b.is_empty() {
        return pairs;
    }
    let (n, m) = (a.len() as isize, b.len() as isize);
    let max = n + m;
    let offset = max + 1;
    let mut v = vec![0isize; (2 * max + 3) as usize];
    // trace[d]: the furthest x per diagonal k in -(d+1)..=d+1 before step d, which is all
    // the walk back reads; O(D²) memory rather than O((N+M)D).
    let mut trace: Vec<Vec<isize>> = Vec::new();
    'outer: for d in 0..=max {
        trace.push(v[(offset - d - 1) as usize..=(offset + d + 1) as usize].to_vec());
        let mut k = -d;
        while k <= d {
            let index = (k + offset) as usize;
            let mut x = if k == -d || (k != d && v[index - 1] < v[index + 1]) {
                v[index + 1]
            } else {
                v[index - 1] + 1
            };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[index] = x;
            if x >= n && y >= m {
                break 'outer;
            }
            k += 2;
        }
    }
    // Walk back, pairing the diagonal moves.
    let (mut x, mut y) = (n, m);
    for d in (1..trace.len() as isize).rev() {
        let window = &trace[d as usize];
        let at = |k: isize| window[(k + d + 1) as usize];
        let k = x - y;
        let prev_k = if k == -d || (k != d && at(k - 1) < at(k + 1)) {
            k + 1
        } else {
            k - 1
        };
        let prev_x = at(prev_k);
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            pairs[x as usize] = Some(y as usize);
        }
        x = prev_x;
        y = prev_y;
    }
    while x > 0 && y > 0 {
        x -= 1;
        y -= 1;
        pairs[x as usize] = Some(y as usize);
    }
    pairs
}

/// One character of the pack stream: its verse, scalar offset in the verse text, and the
/// index of the token holding it.
#[derive(Debug, Clone, Copy)]
struct PackChar {
    verse: VerseId,
    offset: u32,
    token: Option<usize>,
}

fn token_id(verse: VerseId, word: u32) -> String {
    format!(
        "bsb:{:02}{:03}{:03}{word:03}",
        verse.book(),
        verse.chapter(),
        verse.verse()
    )
}

/// The join. `clear` is the target table in file order (verse and word order).
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn join(pack: &PackText, clear: &[TargetToken]) -> TargetJoin {
    let mut out = TargetJoin::default();
    let report = &mut out.report;
    report.clear_tokens = clear.len() as u64;
    report.pack_tokens = pack.values().map(|v| v.tokens.len() as u64).sum();

    // Chapters of both sides.
    let mut chapters: BTreeSet<(u8, u16)> = pack.keys().map(|v| (v.book(), v.chapter())).collect();
    let mut clear_by_chapter: BTreeMap<(u8, u16), Vec<usize>> = BTreeMap::new();
    for (i, t) in clear.iter().enumerate() {
        let key = (t.book.number(), t.chapter);
        chapters.insert(key);
        clear_by_chapter.entry(key).or_default().push(i);
    }
    // Per pack token (verse, token index): the Clear words joined to it.
    let mut held: BTreeMap<(VerseId, usize), Vec<usize>> = BTreeMap::new();
    let mut words: Vec<JoinedTarget> = Vec::new();
    let mut changed: BTreeSet<VerseId> = BTreeSet::new();
    let empty = Vec::new();
    for (book, chapter) in chapters {
        report.chapters += 1;
        let first = VerseId::new(book, chapter, 0);
        let last = VerseId::new(book, chapter, 999);
        let mut b: Vec<char> = Vec::new();
        let mut owners: Vec<PackChar> = Vec::new();
        for (verse, pv) in pack.range(first..=last) {
            let mut token = 0usize;
            for (offset, c) in pv.text.chars().enumerate() {
                let offset = offset as u32;
                while token < pv.tokens.len() && pv.tokens[token].2 <= offset {
                    token += 1;
                }
                if c.is_whitespace() {
                    continue;
                }
                let inside = pv
                    .tokens
                    .get(token)
                    .filter(|t| t.1 <= offset && offset < t.2)
                    .map(|_| token);
                b.push(c);
                owners.push(PackChar {
                    verse: *verse,
                    offset,
                    token: inside,
                });
            }
        }
        let indexes = clear_by_chapter.get(&(book, chapter)).unwrap_or(&empty);
        let mut a: Vec<char> = Vec::new();
        let mut a_owner: Vec<usize> = Vec::new();
        for i in indexes {
            for c in clear[*i].text.chars() {
                a.push(c);
                a_owner.push(*i);
            }
        }
        let pairs: Vec<Option<usize>> = if a == b {
            report.chapters_equal += 1;
            (0..a.len()).map(Some).collect()
        } else {
            let pairs = diff_pairs(&a, &b);
            // A verse is changed when it holds a character with no partner, or when a Clear
            // character with no partner falls between characters of it.
            let mut paired_b = vec![false; b.len()];
            for p in pairs.iter().flatten() {
                paired_b[*p] = true;
            }
            for (j, paired) in paired_b.iter().enumerate() {
                if !paired && owners[j].token.is_some() {
                    changed.insert(owners[j].verse);
                }
            }
            let mut next_b = 0usize;
            for p in &pairs {
                match p {
                    Some(j) => next_b = j + 1,
                    None => {
                        if let Some(o) = owners.get(next_b).or_else(|| owners.last()) {
                            changed.insert(o.verse);
                        }
                    }
                }
            }
            pairs
        };
        // Each Clear word of the chapter.
        let mut start = 0usize;
        while start < a.len() {
            let i = a_owner[start];
            let mut end = start;
            while end < a.len() && a_owner[end] == i {
                end += 1;
            }
            let token = &clear[i];
            if !token.exclude {
                let mapped: Vec<Option<usize>> = pairs[start..end].to_vec();
                let place = if mapped.iter().all(Option::is_some) {
                    let js: Vec<usize> = mapped.into_iter().flatten().collect();
                    let contiguous = js.windows(2).all(|w| w[1] == w[0] + 1);
                    let first = owners[js[0]];
                    let same = js.iter().all(|j| {
                        owners[*j].verse == first.verse && owners[*j].token == first.token
                    });
                    match (contiguous && same, first.token) {
                        (true, Some(t)) => {
                            let pv = &pack[&first.verse];
                            let (word, token_start, _) = pv.tokens[t];
                            let last = owners[*js.last().unwrap_or(&js[0])];
                            held.entry((first.verse, t)).or_default().push(words.len());
                            Some((
                                first.verse,
                                word,
                                first.offset - token_start,
                                last.offset + 1 - token_start,
                            ))
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                words.push(JoinedTarget {
                    clear: i,
                    place,
                    class: if place.is_some() {
                        "exact"
                    } else {
                        "text_differs"
                    },
                });
            }
            start = end;
        }
        // Clear words of a chapter with no letters (none expected) still count.
        for i in indexes {
            if clear[*i].text.is_empty() && !clear[*i].exclude {
                words.push(JoinedTarget {
                    clear: *i,
                    place: None,
                    class: "text_differs",
                });
            }
        }
    }

    // Classes, now that every token knows its words.
    for ((verse, t), list) in &held {
        let pv = &pack[verse];
        let (_, start, end) = pv.tokens[*t];
        let text: Vec<char> = pv.text.chars().collect();
        let token_text: String = text[start as usize..end as usize].iter().collect();
        let mut covered = vec![false; (end - start) as usize];
        for w in list {
            if let Some((_, _, s, e)) = words[*w].place {
                for c in &mut covered[s as usize..e as usize] {
                    *c = true;
                }
            }
        }
        let other_letters = token_text
            .chars()
            .zip(&covered)
            .any(|(c, cov)| !cov && c.is_alphanumeric());
        for w in list {
            let joined = &mut words[*w];
            let clear_token = &clear[joined.clear];
            let clear_verse = VerseId::new(
                clear_token.book.number(),
                clear_token.chapter,
                clear_token.verse,
            );
            joined.class = if changed.contains(verse) {
                "in_changed_verse"
            } else if clear_verse != *verse {
                "other_verse"
            } else if other_letters {
                "token_has_other_letters"
            } else if list.len() > 1 {
                "shares_token"
            } else if token_text == clear_token.text {
                "exact"
            } else {
                "punctuation"
            };
        }
    }

    // Tokens without words.
    let mut verses_with_words: BTreeSet<VerseId> = BTreeSet::new();
    for (verse, _) in held.keys() {
        verses_with_words.insert(*verse);
    }
    for (verse, pv) in pack {
        let text: Vec<char> = pv.text.chars().collect();
        for (t, (word, start, end)) in pv.tokens.iter().enumerate() {
            if held.contains_key(&(*verse, t)) {
                continue;
            }
            let reason = if verses_with_words.contains(verse) {
                "not_in_clear_target"
            } else {
                "verse_not_in_clear_target"
            };
            out.absent.push((*verse, *word, reason));
            *report.absent_reasons.entry(reason.to_owned()).or_default() += 1;
            report
                .places
                .entry(reason.to_owned())
                .or_default()
                .push(TargetPlace {
                    token: Some(token_id(*verse, *word)),
                    clear: crate::alignment::osis(*verse),
                    clear_text: String::new(),
                    pack_text: text[*start as usize..*end as usize].iter().collect(),
                });
        }
    }
    report.tokens_without_words = out.absent.len() as u64;
    report.tokens_with_words = held.len() as u64;
    report.tokens_with_several_words = held.values().filter(|l| l.len() > 1).count() as u64;
    report.changed_verses = changed.len() as u64;
    for verse in &changed {
        let pv = &pack[verse];
        report
            .places
            .entry("changed_verse".to_owned())
            .or_default()
            .push(TargetPlace {
                token: None,
                clear: crate::alignment::osis(*verse),
                clear_text: String::new(),
                pack_text: pv.text.clone(),
            });
    }
    for w in &words {
        report.clear_words += 1;
        if w.place.is_some() {
            report.words_joined += 1;
        } else {
            report.words_not_joined += 1;
        }
        *report.word_classes.entry(w.class.to_owned()).or_default() += 1;
        if matches!(w.class, "exact" | "punctuation" | "shares_token") {
            continue;
        }
        let token = w.place.map(|(v, word, _, _)| (v, word));
        let pack_text = token
            .and_then(|(v, word)| {
                let pv = pack.get(&v)?;
                let (_, s, e) = pv.tokens.iter().find(|t| t.0 == word)?;
                Some(
                    pv.text
                        .chars()
                        .skip(*s as usize)
                        .take((e - s) as usize)
                        .collect(),
                )
            })
            .unwrap_or_default();
        report
            .places
            .entry(w.class.to_owned())
            .or_default()
            .push(TargetPlace {
                token: token.map(|(v, word)| token_id(v, word)),
                clear: clear[w.clear].id.clone(),
                clear_text: clear[w.clear].text.clone(),
                pack_text,
            });
    }
    out.words = words;
    out
}

#[cfg(test)]
mod tests {
    use openlogos_core::reference::BookId;

    use super::*;

    fn verse(text: &str) -> PackVerse {
        // Whitespace fields holding a letter or digit, as B071 decision 6.
        let chars: Vec<char> = text.chars().collect();
        let mut tokens = Vec::new();
        let mut i = 0;
        let mut word = 0;
        while i < chars.len() {
            if chars[i].is_whitespace() {
                i += 1;
                continue;
            }
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            if chars[start..i].iter().any(|c| c.is_alphanumeric()) {
                word += 1;
                tokens.push((word, start as u32, i as u32));
            }
        }
        PackVerse {
            text: text.to_owned(),
            tokens,
        }
    }

    fn clear(book: &str, chapter: u16, verse: u16, words: &[&str]) -> Vec<TargetToken> {
        let book = BookId::from_osis(book).unwrap();
        words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let (text, skip) = w.strip_suffix('+').map_or((*w, false), |t| (t, true));
                let exclude = !text.chars().any(char::is_alphanumeric);
                TargetToken {
                    id: format!("{:02}{chapter:03}{verse:03}{:03}", 0, i + 1),
                    book,
                    chapter,
                    verse,
                    text: text.to_owned(),
                    skip_space_after: skip,
                    exclude,
                }
            })
            .collect()
    }

    #[test]
    fn myers_pairs_only_equal_characters() {
        let a: Vec<char> = "abcabba".chars().collect();
        let b: Vec<char> = "cbabac".chars().collect();
        let pairs = diff_pairs(&a, &b);
        let kept: Vec<(usize, usize)> = pairs
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.map(|j| (i, j)))
            .collect();
        assert_eq!(
            kept.len(),
            4,
            "the LCS of the classic example has 4 characters"
        );
        for (i, j) in &kept {
            assert_eq!(a[*i], b[*j]);
        }
        assert!(kept.windows(2).all(|w| w[0].0 < w[1].0 && w[0].1 < w[1].1));
        let same: Vec<char> = "same".chars().collect();
        assert_eq!(
            diff_pairs(&same, &same),
            vec![Some(0), Some(1), Some(2), Some(3)]
        );
        assert_eq!(diff_pairs(&[], &same), Vec::<Option<usize>>::new());
    }

    #[test]
    fn words_join_tokens_with_classes_and_differences_are_listed() {
        let v1 = VerseId::new(1, 3, 19);
        let v2 = VerseId::new(1, 35, 8);
        let mut pack = PackText::new();
        pack.insert(v1, verse("to the ground—because out of it, you"));
        pack.insert(v2, verse("named it Allon-bacuth."));
        let mut targets = clear(
            "Gen",
            3,
            19,
            &[
                "to", "the", "ground+", "—", "because", "out", "of", "it+", ",", "you",
            ],
        );
        targets.extend(clear("Gen", 35, 8, &["named", "it", "Allon-bachuth+", "."]));
        let joined = join(&pack, &targets);
        let class_of = |text: &str| {
            joined
                .words
                .iter()
                .find(|w| targets[w.clear].text == text)
                .map(|w| (w.class, w.place))
                .unwrap()
        };
        assert_eq!(class_of("to").0, "exact");
        assert_eq!(class_of("ground"), ("shares_token", Some((v1, 3, 0, 6))));
        assert_eq!(class_of("because"), ("shares_token", Some((v1, 3, 7, 14))));
        assert_eq!(class_of("it").0, "punctuation");
        // Gen 35:8 differs by one letter: the changed word is listed, its neighbours joined.
        assert_eq!(class_of("Allon-bachuth"), ("text_differs", None));
        assert_eq!(class_of("named").0, "in_changed_verse");
        assert_eq!(joined.report.changed_verses, 1);
        assert_eq!(joined.absent, vec![(v2, 3, "not_in_clear_target")]);
        assert_eq!(joined.report.words_not_joined, 1);
    }

    #[test]
    fn a_title_clear_numbers_zero_joins_the_pack_verse_holding_it() {
        let v1 = VerseId::new(19, 11, 1);
        let mut pack = PackText::new();
        pack.insert(v1, verse("Of David. In the LORD I take refuge."));
        let mut targets = clear("Ps", 11, 0, &["Of", "David+", "."]);
        targets.extend(clear(
            "Ps",
            11,
            1,
            &["In", "the", "LORD", "I", "take", "refuge+", "."],
        ));
        let joined = join(&pack, &targets);
        assert_eq!(joined.report.word_classes["other_verse"], 2);
        assert_eq!(joined.report.word_classes["exact"], 5);
        assert_eq!(joined.report.word_classes["punctuation"], 1);
        assert!(joined.absent.is_empty());
        // A verse Clear does not have is listed whole.
        pack.insert(VerseId::new(19, 11, 2), verse("Extra words"));
        let joined = join(&pack, &targets);
        assert_eq!(joined.report.absent_reasons["verse_not_in_clear_target"], 2);
    }
}
