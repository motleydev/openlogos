//! Scripture citations in the notes of the 1813 translation: `Rom. viii. 32.`,
//! `Gen. xviii. 2; xxxii. 1, 28. Josh. v. 13.`, `1 Cor x. 25, 29, 32.`, `Jude 9.`,
//! `Rom. viii. 35, &c.`
//!
//! The notes write a chapter as a lower-case roman numeral and use the abbreviations of the
//! time. This module only finds where a citation starts and ends and rewrites its numbers as
//! the core parser reads them (`Gen 18:2; 32:1, 28`); the core reference parser
//! ([`openlogos_core::reference::parse`]) decides what the citation means, with the English
//! alias table plus the 1813 spellings in [`EXTRA_ALIASES`]. Nothing is guessed: what the
//! parser flags as ambiguous, what names no book of the canon table, what depends on an
//! earlier citation (`Ibid.`, `Ver. 5.`) and what the `eng` scheme has no verse for stay
//! unresolved, each with its class.

use openlogos_core::reference::{AliasTable, BookId, ReferenceSet, ReviewFlag, parse};
use openlogos_retrieval_sqlite::work_pack::verse_spans;

/// Spellings of the 1813 notes the English table does not have, as `(book, spelling)`.
pub const EXTRA_ALIASES: [(BookId, &str); 9] = [
    (BookId::Eph, "Ephes"),
    (BookId::Num, "Numb"),
    (BookId::Eccl, "Eccles"),
    (BookId::Ps, "Psal"),
    (BookId::Phil, "Philip"),
    (BookId::Jer, "Jerem"),
    (BookId::Heb, "Hebr"),
    (BookId::Col, "Coloss"),
    (BookId::Lam, "Lament"),
];

/// Words that start a citation relative to an earlier one; never resolved here.
const RELATIVE: [&str; 9] = [
    "Ibid", "Ib", "Id", "Idem", "Ver", "Vers", "Verse", "Chap", "Ch",
];

/// Books outside the 66-book canon table that the notes cite.
const NOT_IN_CANON_TABLE: [&str; 14] = [
    "Ecclus",
    "Ecclesiasticus",
    "Wisdom",
    "Wisd",
    "Tob",
    "Tobit",
    "Baruch",
    "Bar",
    "Macc",
    "Maccabees",
    "Judith",
    "Esdras",
    "Sirach",
    "Susanna",
];

/// The English alias table with [`EXTRA_ALIASES`] added.
#[must_use]
pub fn alias_table() -> AliasTable {
    let mut table = AliasTable::english();
    for (book, spelling) in EXTRA_ALIASES {
        if let Some(entry) = table.books.iter_mut().find(|e| e.book == book) {
            entry.aliases.push(spelling.to_owned());
        }
    }
    table
}

/// What became of one citation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A passage the `eng` scheme has.
    Resolved {
        /// What the parser read.
        set: ReferenceSet,
        /// Its verse-id spans.
        spans: Vec<(u32, u32)>,
        /// `and following`, `chapter only`.
        detail: Option<String>,
    },
    /// Not resolved, with the class and a detail.
    Unresolved {
        /// `relative`, `not_in_canon_table`, `unknown_name`, `ambiguous_book`,
        /// `not_in_scheme`, `parse_error`.
        class: &'static str,
        /// What was seen.
        detail: String,
    },
}

/// One citation found in a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Scalar offset of its first character in the note.
    pub start: usize,
    /// Scalar offset after its last character.
    pub end: usize,
    /// As printed.
    pub text: String,
    /// What the parser was given.
    pub normalized: String,
    /// The result.
    pub outcome: Outcome,
}

struct Scan<'a> {
    chars: &'a [char],
    pos: usize,
}

impl Scan<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn at(&self, i: usize) -> Option<char> {
        self.chars.get(i).copied()
    }

    fn spaces(&mut self) -> usize {
        let start = self.pos;
        while self.peek() == Some(' ') {
            self.pos += 1;
        }
        self.pos - start
    }

    fn digits(&mut self) -> Option<String> {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        (self.pos > start).then(|| self.chars[start..self.pos].iter().collect())
    }

    /// A lower-case roman numeral followed by `.` or `,` (consumed when `.`): a chapter.
    fn roman_chapter(&mut self) -> Option<u32> {
        let start = self.pos;
        while self.peek().is_some_and(|c| "ivxlc".contains(c)) {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        let value = super::roman(&text);
        let ends = matches!(self.peek(), Some('.') | Some(',') | Some(';') | None)
            || (self.peek() == Some(' ')
                && self.at(self.pos + 1).is_some_and(|c| c.is_ascii_digit()));
        match value {
            Some(v) if !text.is_empty() && ends => {
                if self.peek() == Some('.') {
                    self.pos += 1;
                }
                Some(v)
            }
            _ => {
                self.pos = start;
                None
            }
        }
    }

    /// Consumes `word` if it is next.
    fn word(&mut self, word: &str) -> bool {
        let w: Vec<char> = word.chars().collect();
        if self.chars.get(self.pos..self.pos + w.len()) == Some(&w[..]) {
            self.pos += w.len();
            true
        } else {
            false
        }
    }

    fn dash(&mut self) -> bool {
        if matches!(self.peek(), Some('-' | '\u{2010}' | '–' | '—')) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// `2`, `2‐5`, `2, 5, 7‐9`, ending before `, 1 Cor` (a new item), with `&c.` noted.
    /// Returns the verse text in the parser's syntax.
    fn verses(&mut self, and_following: &mut bool) -> Option<String> {
        let first = self.digits()?;
        let mut out = first;
        loop {
            let save = self.pos;
            if self.dash() {
                self.spaces();
                // `2‐xix. 3`: a range into another chapter.
                let mark = self.pos;
                if let Some(chapter) = self.roman_chapter() {
                    self.spaces();
                    if let Some(verse) = self.digits() {
                        out.push_str(&format!("-{chapter}:{verse}"));
                        continue;
                    }
                }
                self.pos = mark;
                if let Some(end) = self.digits() {
                    out.push('-');
                    out.push_str(&end);
                    continue;
                }
                self.pos = save;
                break;
            }
            if self.peek() == Some(',') {
                self.pos += 1;
                self.spaces();
                if self.peek() == Some('&') && self.at(self.pos + 1) == Some('c') {
                    self.pos += 2;
                    if self.peek() == Some('.') {
                        self.pos += 1;
                    }
                    *and_following = true;
                    break;
                }
                let mark = self.pos;
                if let Some(next) = self.digits() {
                    // `25, 1 Cor.`: the number is the next book's ordinal.
                    let ordinal = next.len() == 1
                        && matches!(next.as_str(), "1" | "2" | "3")
                        && self.peek() == Some(' ')
                        && self.at(self.pos + 1).is_some_and(char::is_uppercase);
                    if ordinal {
                        self.pos = save;
                        break;
                    }
                    out.push_str(", ");
                    out.push_str(&next);
                    continue;
                }
                self.pos = mark;
                self.pos = save;
                break;
            }
            if self.peek() == Some(' ')
                && self.at(self.pos + 1) == Some('&')
                && self.at(self.pos + 2) == Some('c')
            {
                self.pos += 3;
                if self.peek() == Some('.') {
                    self.pos += 1;
                }
                *and_following = true;
                break;
            }
            break;
        }
        Some(out)
    }
}

fn is_word_start(chars: &[char], i: usize) -> bool {
    i == 0 || !chars[i - 1].is_alphanumeric()
}

/// A book-like name at `i`: an optional ordinal `1`–`3` and a space, then one capitalised
/// word, then an optional period. Returns `(name, end)`.
fn name_at(chars: &[char], i: usize) -> Option<(String, usize)> {
    let mut pos = i;
    let mut name = String::new();
    if matches!(chars.get(pos), Some('1' | '2' | '3')) && chars.get(pos + 1) == Some(&' ') {
        name.push(chars[pos]);
        name.push(' ');
        pos += 2;
    }
    if !chars.get(pos).is_some_and(|c| c.is_uppercase()) {
        return None;
    }
    let word_start = pos;
    while chars.get(pos).is_some_and(|c| c.is_alphabetic()) {
        pos += 1;
    }
    if pos - word_start < 2 {
        return None;
    }
    name.extend(&chars[word_start..pos]);
    if chars.get(pos) == Some(&'.') {
        pos += 1;
    }
    Some((name, pos))
}

/// Every citation in a note, in order.
#[must_use]
pub fn extract(note: &str, table: &AliasTable) -> Vec<Found> {
    let chars: Vec<char> = note.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !is_word_start(&chars, i) {
            i += 1;
            continue;
        }
        let Some((name, after_name)) = name_at(&chars, i) else {
            i += 1;
            continue;
        };
        let bare = name.trim_end_matches('.').to_owned();
        let word = bare.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ');
        let books = table.lookup(&bare);
        let single_chapter = books.len() == 1 && books[0].is_single_chapter();
        let mut scan = Scan {
            chars: &chars,
            pos: after_name,
        };
        // A bare `Ibid.` cites what the note before it cited: kept, unresolved.
        if ["Ibid", "Ib", "Idem"].contains(&word) && chars.get(after_name - 1) == Some(&'.') {
            let probe = after_name + usize::from(chars.get(after_name) == Some(&' '));
            let numbered = chars
                .get(probe)
                .is_some_and(|c| c.is_ascii_digit() || "ivxlc".contains(*c));
            if !numbered {
                found.push(Found {
                    start: i,
                    end: after_name,
                    text: chars[i..after_name].iter().collect(),
                    normalized: String::new(),
                    outcome: Outcome::Unresolved {
                        class: "relative",
                        detail: format!("`{name}` refers to an earlier citation"),
                    },
                });
                i = after_name;
                continue;
            }
        }
        if single_chapter && scan.peek() == Some(',') {
            scan.pos += 1;
        }
        if scan.spaces() == 0 {
            i += 1;
            continue;
        }
        let mut and_following = false;
        let mut normalized = format!("{bare} ");
        let mut chapter_only = false;
        if let Some(chapter) = scan.roman_chapter() {
            normalized.push_str(&chapter.to_string());
            let mark = scan.pos;
            scan.spaces();
            match scan.verses(&mut and_following) {
                Some(verses) => {
                    normalized.push(':');
                    normalized.push_str(&verses);
                }
                None => {
                    scan.pos = mark;
                    chapter_only = true;
                }
            }
            // Further chapters of the same book: `; xxxii. 1, 28`, `. viii. 50`, `, and xxx. 6`,
            // or a bare list of chapters (`Ps. xciii. xcvi.`).
            loop {
                let mark = scan.pos;
                match scan.peek() {
                    Some(';' | '.' | ',') => scan.pos += 1,
                    Some(' ') => {}
                    _ => break,
                }
                scan.spaces();
                if scan.word("and") {
                    scan.spaces();
                }
                let Some(chapter) = scan.roman_chapter() else {
                    scan.pos = mark;
                    break;
                };
                let inner = scan.pos;
                scan.spaces();
                match scan.verses(&mut and_following) {
                    Some(verses) => normalized.push_str(&format!("; {chapter}:{verses}")),
                    None => {
                        scan.pos = inner;
                        // A chapter alone must end the list or be followed by another numeral,
                        // so `i. e.` is never read as chapter 1.
                        let ends = match scan.peek() {
                            None | Some(',' | ';') => true,
                            Some(' ') => {
                                let mut probe = Scan {
                                    chars: scan.chars,
                                    pos: scan.pos,
                                };
                                probe.spaces();
                                probe.roman_chapter().is_some() || probe.peek() == Some('&')
                            }
                            _ => false,
                        };
                        if !ends {
                            scan.pos = mark;
                            break;
                        }
                        normalized.push_str(&format!("; {chapter}"));
                    }
                }
            }
        } else if single_chapter {
            // `Jude, ver. 9.`
            let mark = scan.pos;
            if !(scan.word("ver.") && scan.spaces() > 0) {
                scan.pos = mark;
            }
            match scan.verses(&mut and_following) {
                Some(verses) => normalized.push_str(&verses),
                None => {
                    i += 1;
                    continue;
                }
            }
        } else {
            i += 1;
            continue;
        }
        let end = scan.pos;
        // The printed text ends at the last character the citation used, without a final
        // period that closes the sentence.
        let mut text_end = end;
        if chars.get(text_end - 1) == Some(&'.') && !and_following {
            text_end -= 1;
        }
        let text: String = chars[i..text_end].iter().collect();
        let outcome = if RELATIVE.contains(&word) {
            Outcome::Unresolved {
                class: "relative",
                detail: format!("`{bare}` refers to an earlier citation"),
            }
        } else if NOT_IN_CANON_TABLE.contains(&word) {
            Outcome::Unresolved {
                class: "not_in_canon_table",
                detail: format!("`{bare}` is not a book of the 66-book canon table"),
            }
        } else if books.is_empty() {
            Outcome::Unresolved {
                class: "unknown_name",
                detail: format!("`{bare}` is not a book name the table knows"),
            }
        } else {
            let before: String = chars[i.saturating_sub(3)..i].iter().collect();
            let commentary = before.eq_ignore_ascii_case("in ");
            let mut outcome = resolve(&normalized, table, and_following, chapter_only);
            if commentary && let Outcome::Resolved { detail, .. } = &mut outcome {
                let remark = "cited as the subject of a commentary (`in Psalm …`)";
                *detail = Some(
                    detail
                        .take()
                        .map_or_else(|| remark.to_owned(), |d| format!("{d}; {remark}")),
                );
            }
            outcome
        };
        found.push(Found {
            start: i,
            end: text_end,
            text,
            normalized,
            outcome,
        });
        i = end.max(i + 1);
    }
    found
}

fn resolve(
    normalized: &str,
    table: &AliasTable,
    and_following: bool,
    chapter_only: bool,
) -> Outcome {
    match parse(normalized, table) {
        Err(error) => Outcome::Unresolved {
            class: "parse_error",
            detail: error.to_string(),
        },
        Ok(set) => {
            if set
                .flags
                .iter()
                .any(|f| matches!(f, ReviewFlag::AmbiguousBook { .. }))
            {
                return Outcome::Unresolved {
                    class: "ambiguous_book",
                    detail: format!(
                        "the abbreviation fits more than one book: {}",
                        set.to_osis()
                    ),
                };
            }
            match verse_spans(&set) {
                None => Outcome::Unresolved {
                    class: "not_in_scheme",
                    detail: format!("`eng` has no such chapter or verse: {}", set.to_osis()),
                },
                Some(spans) => {
                    let mut notes = Vec::new();
                    if and_following {
                        notes.push("&c. (and following): linked to the verses printed");
                    }
                    if chapter_only {
                        notes.push("a whole chapter");
                    }
                    Outcome::Resolved {
                        set,
                        spans,
                        detail: (!notes.is_empty()).then(|| notes.join("; ")),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn osis(note: &str) -> Vec<String> {
        let table = alias_table();
        extract(note, &table)
            .into_iter()
            .map(|f| match f.outcome {
                Outcome::Resolved { set, .. } => set.to_osis(),
                Outcome::Unresolved { class, .. } => format!("!{class}"),
            })
            .collect()
    }

    fn texts(note: &str) -> Vec<String> {
        extract(note, &alias_table())
            .into_iter()
            .map(|f| f.text)
            .collect()
    }

    #[test]
    fn the_notes_forms_resolve() {
        assert_eq!(osis("Rom. viii. 32."), ["Rom.8.32"]);
        assert_eq!(osis("Ephes. i. 5-7, 13."), ["Eph.1.5-Eph.1.7;Eph.1.13"]);
        assert_eq!(osis("2 Sam. xii. 13‐16."), ["2Sam.12.13-2Sam.12.16"]);
        assert_eq!(
            osis("1 Cor x. 25, 29, 32."),
            ["1Cor.10.25;1Cor.10.29;1Cor.10.32"]
        );
        assert_eq!(osis("Jude 9."), ["Jude.1.9"]);
        assert_eq!(osis("Psalm cxix. 105."), ["Ps.119.105"]);
        assert_eq!(
            osis("Gen. xviii. 2; xxxii. 1, 28. Josh. v. 13."),
            ["Gen.18.2;Gen.32.1;Gen.32.28", "Josh.5.13"]
        );
        assert_eq!(
            osis("Rom. iv. 25. 1 Cor. xv. 3, 17."),
            ["Rom.4.25", "1Cor.15.3;1Cor.15.17"]
        );
        assert_eq!(
            osis("Rom. iv. 25, 1 Cor. xv. 3."),
            ["Rom.4.25", "1Cor.15.3"]
        );
        assert_eq!(osis("Psalm li."), ["Ps.51"]);
        assert_eq!(osis("John vii. 18. viii. 50."), ["John.7.18;John.8.50"]);
        assert_eq!(osis("Deut. x. 16, and xxx. 6."), ["Deut.10.16;Deut.30.6"]);
        assert_eq!(osis("Ps. xciii. xcvi., &c."), ["Ps.93;Ps.96"]);
        assert_eq!(
            osis("Daniel xii. 1. Jude, ver. 9."),
            ["Dan.12.1", "Jude.1.9"]
        );
        assert_eq!(osis("Rom. v. 20. i. e. the law."), ["Rom.5.20"]);
        assert_eq!(osis("Isaiah xi. 4. Psalm ii. 9."), ["Isa.11.4", "Ps.2.9"]);
    }

    #[test]
    fn printed_text_and_offsets_are_the_notes_own() {
        let found = extract("See Rom. viii. 35, &c. and more.", &alias_table());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "Rom. viii. 35, &c.");
        assert_eq!(found[0].start, 4);
        assert_eq!(found[0].normalized, "Rom 8:35");
        assert!(
            matches!(&found[0].outcome, Outcome::Resolved { detail: Some(d), .. } if d.contains("&c."))
        );
        assert_eq!(
            texts("1 Peter iv. 17. Jer. xxv. 29, marg. read."),
            ["1 Peter iv. 17", "Jer. xxv. 29"]
        );
    }

    #[test]
    fn what_cannot_be_read_without_a_guess_stays_unresolved_with_its_class() {
        assert_eq!(osis("Ibid. v. 3."), ["!relative"]);
        assert_eq!(osis("Jude 9. Ibid."), ["Jude.1.9", "!relative"]);
        assert_eq!(osis("Ecclus. xv. 14."), ["!not_in_canon_table"]);
        assert_eq!(osis("Like viii. 5-15."), ["!unknown_name"]);
        assert_eq!(osis("Ez. xxxiii. 14, 15."), ["!ambiguous_book"]);
        assert_eq!(osis("Psalm cli. 1."), ["!not_in_scheme"]);
        assert_eq!(osis("Rom. viii. 99."), ["!not_in_scheme"]);
        // Prose without a chapter numeral is not a citation.
        assert!(osis("Seneca, Præf. lib. 1. Quæst. Nat.").is_empty());
        assert!(osis("It is not uncommon, among persons of a certain class.").is_empty());
    }
}
