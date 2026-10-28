//! Work locators: citable coordinates for non-Bible works and the reference grammars that
//! read citations such as `WCF 1.6`, `Inst. 3.2.7`, `Institutes III.ii.7`, `WLC Q. 1` and
//! `Thayer p. 6` (specs/19-search-and-retrieval.md, "Cross-resource links", item 1).
//!
//! A [`WorkGrammar`] is one registered scheme of one work: its names and abbreviations, the
//! levels of its hierarchy (book, chapter, section; question, part; page) and the numeral
//! styles each level accepts. Parsing produces a [`WorkCitation`] whose locators are
//! canonical: arabic numbers joined by dots in the style of CTS URNs, so `III.ii.7` and
//! `3.2.7` are the same locator. A link targets `work + canonical locator`; which edition's
//! segment that is comes from the section maps in `openlogos-retrieval`, never from here.
//!
//! The grammars are versioned with the parser ([`GRAMMAR_VERSION`]); every citation records
//! the version it was parsed under so an extracted edge can be re-parsed when a grammar
//! changes. [`parse_citation`] is the extension of [`crate::reference::parse`]: it tries the
//! work grammars first and falls back to Scripture.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::reference::{AliasTable, ReferenceError, ReferenceSet, parse as parse_reference};

/// Version of the built-in grammars and of the parsing rules in this module. Bumped whenever
/// a grammar's names, levels or numeral styles change, or the parser's reading of a form
/// changes; stored with every extracted citation.
pub const GRAMMAR_VERSION: &str = "b068-1";

/// How a level's number may be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumeralStyle {
    /// `3`, `12`.
    Arabic,
    /// `III`, `XII`.
    RomanUpper,
    /// `iii`, `xii`.
    RomanLower,
}

/// One level of a hierarchical scheme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelSpec {
    /// Level name (`book`, `chapter`, `section`, `question`, `page`).
    pub name: String,
    /// Numeral styles a citation may use at this level; the canonical form is always arabic.
    pub styles: Vec<NumeralStyle>,
}

impl LevelSpec {
    /// A level written only in arabic numerals.
    #[must_use]
    pub fn arabic(name: &str) -> LevelSpec {
        LevelSpec {
            name: name.to_owned(),
            styles: vec![NumeralStyle::Arabic],
        }
    }

    /// A level written in arabic or the given roman style.
    #[must_use]
    pub fn arabic_or(name: &str, roman: NumeralStyle) -> LevelSpec {
        LevelSpec {
            name: name.to_owned(),
            styles: vec![NumeralStyle::Arabic, roman],
        }
    }
}

/// The shape of a scheme's locators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrammarKind {
    /// Dot-separated levels (`3.2.7`); a citation may stop at any level (`3.2` is a chapter).
    Hierarchical,
    /// A numbered question with an optional part: `1` is question and answer 1, `1.a` its
    /// answer alone. Written `Q. 1`, `Q1`, `1` or `A. 1`.
    QuestionAnswer,
    /// Page milestones of one named print edition: `p.6`, written `p. 6` or `pp. 6–7`. The
    /// page marker is required, so a bare number is never read as a page.
    Page,
}

/// One registered reference grammar: a scheme of a work and the ways people cite it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkGrammar {
    /// Work id the locators belong to (`wcf`, `institutes`, `thayer`).
    pub work: String,
    /// Scheme id, globally unique; equal to the work id for the work's canonical scheme.
    pub scheme: String,
    /// Whether this is the work's canonical scheme (`false` for page milestones).
    pub canonical: bool,
    /// Names and abbreviations, display name first; compared after [`normalize_name`].
    pub names: Vec<String>,
    /// Locator shape.
    pub kind: GrammarKind,
    /// Levels in order; one level for page schemes, the question level for catechisms.
    pub levels: Vec<LevelSpec>,
    /// Grammar version this entry was written for.
    pub version: String,
}

/// One part of a locator: a number or a short tag (`p`, `a`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Part {
    /// A level number.
    Num(u32),
    /// A tag such as `p` (page) or `a` (answer).
    Tag(String),
}

impl fmt::Display for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Part::Num(n) => write!(f, "{n}"),
            Part::Tag(t) => f.write_str(t),
        }
    }
}

/// A canonical locator inside a scheme: `3.2.7`, `1.a`, `p.6`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkLocator {
    /// The parts in level order.
    pub parts: Vec<Part>,
}

impl WorkLocator {
    /// Reads a canonical locator string back: digits become numbers, anything else a tag.
    pub fn parse(text: &str) -> Result<WorkLocator, LocatorError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(LocatorError::Empty);
        }
        let mut parts = Vec::new();
        for piece in text.split('.') {
            if piece.is_empty() {
                return Err(LocatorError::InvalidNumeral {
                    text: text.to_owned(),
                });
            }
            parts.push(if piece.chars().all(|c| c.is_ascii_digit()) {
                Part::Num(piece.parse().map_err(|_| LocatorError::InvalidNumeral {
                    text: piece.to_owned(),
                })?)
            } else {
                Part::Tag(piece.to_owned())
            });
        }
        Ok(WorkLocator { parts })
    }
}

impl fmt::Display for WorkLocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                f.write_str(".")?;
            }
            write!(f, "{part}")?;
        }
        Ok(())
    }
}

/// An inclusive range of locators; a single locator has equal ends.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocatorRange {
    /// First locator.
    pub start: WorkLocator,
    /// Last locator, inclusive.
    pub end: WorkLocator,
}

impl LocatorRange {
    /// Whether the range is one locator.
    #[must_use]
    pub fn is_single(&self) -> bool {
        self.start == self.end
    }
}

/// A parsed citation of one work under one scheme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkCitation {
    /// Work id.
    pub work: String,
    /// Scheme id.
    pub scheme: String,
    /// The grammar version the citation was parsed under.
    pub grammar_version: String,
    /// The ranges in the order written.
    pub ranges: Vec<LocatorRange>,
}

impl WorkCitation {
    /// Canonical form, the scheme once and every range written out in full: `wcf:1.6`,
    /// `wcf:1.1-1.3;2.1`, `thayer-1889-pages:p.6`. [`parse_work_citation`] reads it back.
    #[must_use]
    pub fn to_canonical(&self) -> String {
        let ranges = self
            .ranges
            .iter()
            .map(|r| {
                if r.is_single() {
                    r.start.to_string()
                } else {
                    format!("{}-{}", r.start, r.end)
                }
            })
            .collect::<Vec<_>>()
            .join(";");
        format!("{}:{ranges}", self.scheme)
    }

    /// Human form with the grammar's display name: `WCF 1.6`, `WCF 1.1–3; 2.1`,
    /// `Institutes 3.2.7`, `WLC Q. 1`, `WLC A. 1`, `Thayer p. 6`, `Thayer pp. 6–7`.
    #[must_use]
    pub fn to_human(&self, table: &GrammarTable) -> String {
        let grammar = table.grammar(&self.scheme);
        let name = grammar.map_or(self.scheme.as_str(), |g| {
            g.names.first().map_or(g.scheme.as_str(), String::as_str)
        });
        let kind = grammar.map_or(GrammarKind::Hierarchical, |g| g.kind);
        let mut out = name.to_owned();
        for (i, range) in self.ranges.iter().enumerate() {
            out.push_str(if i == 0 { " " } else { "; " });
            out.push_str(&human_range(range, kind));
        }
        out
    }
}

/// `1.1–3`, `p. 6`, `pp. 6–7`, `Q. 1`, `Q. 1–3`, `A. 1`.
fn human_range(range: &LocatorRange, kind: GrammarKind) -> String {
    match kind {
        GrammarKind::Page => {
            let page = |l: &WorkLocator| l.parts.get(1).map_or_else(String::new, Part::to_string);
            if range.is_single() {
                format!("p. {}", page(&range.start))
            } else {
                format!("pp. {}–{}", page(&range.start), page(&range.end))
            }
        }
        GrammarKind::QuestionAnswer => {
            let marker = |l: &WorkLocator| match l.parts.get(1) {
                Some(Part::Tag(t)) if t == "a" => "A. ",
                _ => "Q. ",
            };
            let number =
                |l: &WorkLocator| l.parts.first().map_or_else(String::new, Part::to_string);
            if range.is_single() {
                format!("{}{}", marker(&range.start), number(&range.start))
            } else {
                format!(
                    "{}{}–{}",
                    marker(&range.start),
                    number(&range.start),
                    number(&range.end)
                )
            }
        }
        GrammarKind::Hierarchical => {
            if range.is_single() {
                return range.start.to_string();
            }
            // Elide the leading parts the end shares with the start: `3.2.7–9`.
            let shared = range
                .start
                .parts
                .iter()
                .zip(&range.end.parts)
                .take_while(|(a, b)| a == b)
                .count();
            let shared = shared.min(range.end.parts.len().saturating_sub(1));
            let tail = WorkLocator {
                parts: range.end.parts[shared..].to_vec(),
            };
            format!("{}–{}", range.start, tail)
        }
    }
}

/// A citation of either kind, as [`parse_citation`] returns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Citation {
    /// A Scripture reference.
    Scripture {
        /// The parsed reference set.
        references: ReferenceSet,
    },
    /// A work locator.
    Work {
        /// The parsed citation.
        citation: WorkCitation,
    },
}

/// Errors from parsing a work citation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LocatorError {
    /// Nothing to parse.
    #[error("empty citation")]
    Empty,
    /// No grammar knows the leading name.
    #[error("unknown work `{name}`")]
    UnknownWork {
        /// The name as written.
        name: String,
    },
    /// The work was named but no locator followed (or a page citation lacked `p.`).
    #[error("`{text}` names {work} without a locator")]
    MissingLocator {
        /// Work id.
        work: String,
        /// The item as written.
        text: String,
    },
    /// A level number that is neither arabic nor an allowed roman numeral, or out of range.
    #[error("`{text}` is not a usable numeral")]
    InvalidNumeral {
        /// The numeral as written.
        text: String,
    },
    /// More levels than the scheme has.
    #[error("`{text}` has more levels than {scheme} ({levels})")]
    TooManyLevels {
        /// Scheme id.
        scheme: String,
        /// Levels the scheme has.
        levels: u32,
        /// The locator as written.
        text: String,
    },
    /// A character no citation syntax uses at this position.
    #[error("unexpected `{character}` at scalar offset {offset}")]
    UnexpectedCharacter {
        /// Offset in Unicode scalar values.
        offset: u32,
        /// The character.
        character: char,
    },
    /// A range whose end precedes its start.
    #[error("range runs backwards: {start} to {end}")]
    ReversedRange {
        /// Canonical start.
        start: String,
        /// Canonical end.
        end: String,
    },
}

/// Errors from [`parse_citation`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CitationError {
    /// A work was named but its locator did not parse.
    #[error("work citation: {0}")]
    Work(LocatorError),
    /// A Scripture book was named but the reference did not parse.
    #[error("scripture reference: {0}")]
    Scripture(ReferenceError),
    /// Neither a registered work nor a Scripture book.
    #[error("`{text}` names no known work or book")]
    Unrecognized {
        /// The input as written.
        text: String,
    },
}

/// The registered grammars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarTable {
    /// One entry per scheme, in lookup order.
    pub grammars: Vec<WorkGrammar>,
}

/// The key form of a work name: lower case, dots and commas removed, whitespace collapsed to
/// single spaces. `Inst.` and `inst` match; `Calvin, Institutes` and `Calvin Institutes` match.
#[must_use]
pub fn normalize_name(name: &str) -> String {
    name.to_lowercase()
        .replace(['.', ','], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

impl GrammarTable {
    /// The built-in grammars: the Westminster Confession of Faith (chapter.section), the
    /// Westminster Larger and Shorter Catechisms (question, optional answer part), Calvin's
    /// *Institutes* (book.chapter.section, shared by the Beveridge 1845 and Allen 1813
    /// translations) and page milestones of Thayer's *Greek-English Lexicon of the New
    /// Testament* (1889 print edition). All four works are in the public domain.
    #[must_use]
    pub fn builtin() -> GrammarTable {
        let v = GRAMMAR_VERSION.to_owned();
        let names = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect();
        GrammarTable {
            grammars: vec![
                WorkGrammar {
                    work: "wcf".into(),
                    scheme: "wcf".into(),
                    canonical: true,
                    names: names(&[
                        "WCF",
                        "Westminster Confession",
                        "Westminster Confession of Faith",
                        "Conf.",
                    ]),
                    kind: GrammarKind::Hierarchical,
                    levels: vec![
                        LevelSpec::arabic_or("chapter", NumeralStyle::RomanUpper),
                        LevelSpec::arabic_or("section", NumeralStyle::RomanLower),
                    ],
                    version: v.clone(),
                },
                WorkGrammar {
                    work: "wlc".into(),
                    scheme: "wlc".into(),
                    canonical: true,
                    names: names(&["WLC", "Westminster Larger Catechism", "Larger Catechism"]),
                    kind: GrammarKind::QuestionAnswer,
                    levels: vec![LevelSpec::arabic("question")],
                    version: v.clone(),
                },
                WorkGrammar {
                    work: "wsc".into(),
                    scheme: "wsc".into(),
                    canonical: true,
                    names: names(&["WSC", "Westminster Shorter Catechism", "Shorter Catechism"]),
                    kind: GrammarKind::QuestionAnswer,
                    levels: vec![LevelSpec::arabic("question")],
                    version: v.clone(),
                },
                WorkGrammar {
                    work: "institutes".into(),
                    scheme: "institutes".into(),
                    canonical: true,
                    names: names(&[
                        "Institutes",
                        "Inst.",
                        "Calvin, Institutes",
                        "Institutes of the Christian Religion",
                    ]),
                    kind: GrammarKind::Hierarchical,
                    levels: vec![
                        LevelSpec::arabic_or("book", NumeralStyle::RomanUpper),
                        LevelSpec::arabic_or("chapter", NumeralStyle::RomanLower),
                        LevelSpec::arabic("section"),
                    ],
                    version: v.clone(),
                },
                WorkGrammar {
                    work: "thayer".into(),
                    scheme: "thayer-1889-pages".into(),
                    canonical: false,
                    names: names(&[
                        "Thayer",
                        "Thayer's Lexicon",
                        "Thayer 1889",
                        "thayer-1889-pages",
                    ]),
                    kind: GrammarKind::Page,
                    levels: vec![LevelSpec::arabic("page")],
                    version: v,
                },
            ],
        }
    }

    /// The grammar of a scheme id.
    #[must_use]
    pub fn grammar(&self, scheme: &str) -> Option<&WorkGrammar> {
        self.grammars.iter().find(|g| g.scheme == scheme)
    }

    /// The grammar a written name refers to: its scheme id, or any of its names.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<&WorkGrammar> {
        let key = normalize_name(name);
        if key.is_empty() {
            return None;
        }
        self.grammars.iter().find(|g| {
            normalize_name(&g.scheme) == key || g.names.iter().any(|n| normalize_name(n) == key)
        })
    }
}

/// Scripture aliases plus work grammars: everything [`parse_citation`] needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationGrammar {
    /// Book names for Scripture references.
    pub aliases: AliasTable,
    /// Work grammars.
    pub works: GrammarTable,
}

impl CitationGrammar {
    /// English book names with the built-in work grammars.
    #[must_use]
    pub fn english() -> CitationGrammar {
        CitationGrammar {
            aliases: AliasTable::english(),
            works: GrammarTable::builtin(),
        }
    }
}

/// Parses a citation of a registered work.
///
/// Accepted: `WCF 1.6`, `WCF I.vi`, `WCF 1.1-1.3`, `WCF 1.1-3; 2.1`, `Inst. 3.2.7`,
/// `Institutes III.ii.7`, `Institutes 3.2`, `institutes:3.2.7`, `WLC Q. 1`, `WLC Q1`,
/// `WSC 1`, `WSC A. 1`, `Thayer p. 6`, `Thayer, pp. 6–7`. Separators between levels are
/// `.` or `:`; a range uses a hyphen or dash and its end may elide the leading levels it
/// shares with the start.
pub fn parse_work_citation(
    input: &str,
    table: &GrammarTable,
) -> Result<WorkCitation, LocatorError> {
    let (grammar, rest, rest_offset) = split_name(input, table)?;
    let mut scanner = Scanner {
        chars: rest.chars().collect(),
        pos: 0,
        base: rest_offset,
    };
    let mut ranges = Vec::new();
    loop {
        scanner.skip_separators();
        if scanner.at_end() {
            break;
        }
        let item_start = scanner.pos;
        let Some(start) = scanner.locator(grammar, None)? else {
            return Err(LocatorError::MissingLocator {
                work: grammar.work.clone(),
                text: scanner.rest_of_item(item_start),
            });
        };
        scanner.skip_whitespace();
        let end = if scanner.eat_dash() {
            scanner.skip_whitespace();
            match scanner.locator(grammar, Some(&start))? {
                Some(end) => end,
                None => {
                    return Err(LocatorError::MissingLocator {
                        work: grammar.work.clone(),
                        text: scanner.rest_of_item(item_start),
                    });
                }
            }
        } else {
            start.clone()
        };
        if end < start {
            return Err(LocatorError::ReversedRange {
                start: start.to_string(),
                end: end.to_string(),
            });
        }
        ranges.push(LocatorRange { start, end });
        scanner.skip_whitespace();
        if scanner.at_end() {
            break;
        }
        if !(scanner.eat(';') || scanner.eat(',')) {
            return Err(scanner.unexpected());
        }
    }
    if ranges.is_empty() {
        return Err(LocatorError::MissingLocator {
            work: grammar.work.clone(),
            text: input.trim().to_owned(),
        });
    }
    Ok(WorkCitation {
        work: grammar.work.clone(),
        scheme: grammar.scheme.clone(),
        grammar_version: grammar.version.clone(),
        ranges,
    })
}

/// Parses a citation as a work locator if a registered work is named, otherwise as a
/// Scripture reference. The scalar offsets in errors refer to `input`.
pub fn parse_citation(input: &str, grammar: &CitationGrammar) -> Result<Citation, CitationError> {
    if input.trim().is_empty() {
        return Err(CitationError::Work(LocatorError::Empty));
    }
    match parse_work_citation(input, &grammar.works) {
        Ok(citation) => Ok(Citation::Work { citation }),
        Err(LocatorError::UnknownWork { .. }) => match parse_reference(input, &grammar.aliases) {
            Ok(references) => Ok(Citation::Scripture { references }),
            Err(ReferenceError::UnknownBook { .. } | ReferenceError::MissingBook { .. }) => {
                Err(CitationError::Unrecognized {
                    text: input.trim().to_owned(),
                })
            }
            Err(e) => Err(CitationError::Scripture(e)),
        },
        Err(e) => Err(CitationError::Work(e)),
    }
}

/// Finds the longest leading run of words that names a grammar. `wcf:1.6` splits at the
/// colon. Returns the grammar, the rest of the input and the rest's scalar offset.
fn split_name<'t>(
    input: &str,
    table: &'t GrammarTable,
) -> Result<(&'t WorkGrammar, String, u32), LocatorError> {
    let chars: Vec<char> = input.chars().collect();
    let trimmed_start = chars
        .iter()
        .position(|c| !c.is_whitespace())
        .ok_or(LocatorError::Empty)?;
    // Word boundaries: the scalar offset after each word.
    let mut boundaries: Vec<usize> = Vec::new();
    let mut i = trimmed_start;
    while i < chars.len() {
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != ':' {
            i += 1;
        }
        boundaries.push(i);
        if i < chars.len() && chars[i] == ':' && boundaries.len() == 1 {
            // `scheme:locator`.
            let name: String = chars[trimmed_start..i].iter().collect();
            if let Some(g) = table.lookup(&name) {
                let rest: String = chars[i + 1..].iter().collect();
                return Ok((g, rest, (i + 1) as u32));
            }
        }
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if boundaries.len() == 6 {
            break;
        }
    }
    for end in boundaries.iter().rev() {
        let name: String = chars[trimmed_start..*end].iter().collect();
        if let Some(g) = table.lookup(&name) {
            let rest: String = chars[*end..].iter().collect();
            return Ok((g, rest, *end as u32));
        }
    }
    let first: String = chars[trimmed_start..boundaries.first().copied().unwrap_or(chars.len())]
        .iter()
        .collect();
    Err(LocatorError::UnknownWork {
        name: first.trim_end_matches([',', '.']).to_owned(),
    })
}

/// Value of a roman numeral in the given case, 1 to 3999; `None` if not well formed.
fn roman_value(text: &str, upper: bool) -> Option<u32> {
    let digit = |c: char| -> Option<u32> {
        let c = if upper {
            c.to_ascii_uppercase()
        } else {
            c.to_ascii_lowercase()
        };
        Some(match c {
            'I' | 'i' => 1,
            'V' | 'v' => 5,
            'X' | 'x' => 10,
            'L' | 'l' => 50,
            'C' | 'c' => 100,
            'D' | 'd' => 500,
            'M' | 'm' => 1000,
            _ => return None,
        })
    };
    if text.is_empty()
        || !text.chars().all(|c| {
            if upper {
                c.is_ascii_uppercase()
            } else {
                c.is_ascii_lowercase()
            }
        })
    {
        return None;
    }
    let values: Vec<u32> = text.chars().map(digit).collect::<Option<_>>()?;
    let mut total = 0i64;
    for (i, v) in values.iter().enumerate() {
        if values.get(i + 1).is_some_and(|next| next > v) {
            total -= i64::from(*v);
        } else {
            total += i64::from(*v);
        }
    }
    let total = u32::try_from(total).ok()?;
    // Reject forms like `IIII` or `VX` by requiring the canonical spelling to round-trip.
    (total > 0 && roman_text(total, upper) == text).then_some(total)
}

/// Canonical roman spelling of `n` (1 to 3999), in the given case.
fn roman_text(n: u32, upper: bool) -> String {
    const TABLE: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut n = n;
    let mut out = String::new();
    for (value, text) in TABLE {
        while n >= value {
            out.push_str(text);
            n -= value;
        }
    }
    if upper { out } else { out.to_lowercase() }
}

/// Largest level number accepted (pages of a thick lexicon fit; CTS-style sections do).
const MAX_NUMBER: u32 = 9999;

/// Cursor over the locator text after the work name.
struct Scanner {
    chars: Vec<char>,
    pos: usize,
    /// Scalar offset of `chars[0]` in the original input, for error messages.
    base: u32,
}

impl Scanner {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    /// Whitespace and a comma directly after the name (`Thayer, p. 6`).
    fn skip_separators(&mut self) {
        self.skip_whitespace();
        if self.eat(',') {
            self.skip_whitespace();
        }
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn eat_dash(&mut self) -> bool {
        self.eat('-') || self.eat('–') || self.eat('—')
    }

    /// Eats one of the markers (case-insensitive), with an optional trailing period.
    fn eat_marker(&mut self, markers: &[&str]) -> bool {
        for marker in markers {
            let n = marker.chars().count();
            let candidate: String = self.chars[self.pos..].iter().take(n).collect();
            if candidate.eq_ignore_ascii_case(marker)
                && !self
                    .chars
                    .get(self.pos + n)
                    .is_some_and(|c| c.is_alphabetic())
            {
                self.pos += n;
                self.eat('.');
                return true;
            }
        }
        false
    }

    /// A run of letters or digits forming one level: `3`, `III`, `ii`.
    fn numeral_text(&mut self) -> String {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_alphanumeric()) {
            self.pos += 1;
        }
        self.chars[start..self.pos].iter().collect()
    }

    /// One level number in a style the level allows.
    fn level_number(&mut self, level: &LevelSpec) -> Result<Option<u32>, LocatorError> {
        let start = self.pos;
        let text = self.numeral_text();
        if text.is_empty() {
            return Ok(None);
        }
        let value = if text.chars().all(|c| c.is_ascii_digit()) {
            level
                .styles
                .contains(&NumeralStyle::Arabic)
                .then(|| text.parse::<u32>().ok())
                .flatten()
        } else if level.styles.contains(&NumeralStyle::RomanUpper)
            && let Some(v) = roman_value(&text, true)
        {
            Some(v)
        } else if level.styles.contains(&NumeralStyle::RomanLower)
            && let Some(v) = roman_value(&text, false)
        {
            Some(v)
        } else {
            None
        };
        match value {
            Some(v) if (1..=MAX_NUMBER).contains(&v) => Ok(Some(v)),
            _ => {
                self.pos = start;
                Err(LocatorError::InvalidNumeral { text })
            }
        }
    }

    /// A locator of the grammar's kind. For a range end, `start` supplies the leading
    /// levels an abbreviated end elides.
    fn locator(
        &mut self,
        grammar: &WorkGrammar,
        start: Option<&WorkLocator>,
    ) -> Result<Option<WorkLocator>, LocatorError> {
        match grammar.kind {
            GrammarKind::Page => {
                let marked = self.eat_marker(&["pages", "page", "pp", "p"]);
                self.skip_whitespace();
                if !marked && start.is_none() {
                    return Ok(None);
                }
                let Some(n) = self.level_number(&grammar.levels[0])? else {
                    return Ok(None);
                };
                Ok(Some(WorkLocator {
                    parts: vec![Part::Tag("p".into()), Part::Num(n)],
                }))
            }
            GrammarKind::QuestionAnswer => {
                let answer = self.eat_marker(&["answer", "a"]);
                let question = !answer && self.eat_marker(&["question", "q"]);
                self.skip_whitespace();
                let Some(n) = self.level_number(&grammar.levels[0])? else {
                    return Ok(None);
                };
                let mut parts = vec![Part::Num(n)];
                let inherited_answer = !question
                    && !answer
                    && start
                        .is_some_and(|s| matches!(s.parts.get(1), Some(Part::Tag(t)) if t == "a"));
                if answer || inherited_answer {
                    parts.push(Part::Tag("a".into()));
                } else if self.eat('.') {
                    // `1.a` in canonical form.
                    let tag = self.numeral_text();
                    match tag.as_str() {
                        "a" => parts.push(Part::Tag("a".into())),
                        _ => return Err(LocatorError::InvalidNumeral { text: tag }),
                    }
                }
                Ok(Some(WorkLocator { parts }))
            }
            GrammarKind::Hierarchical => {
                let item_start = self.pos;
                let mut numbers: Vec<u32> = Vec::new();
                // The end of a range may start at a deeper level than the first: `3.2.7-9`
                // reads `9` against the section level.
                loop {
                    let level_index = numbers.len();
                    if level_index >= grammar.levels.len() {
                        return Err(LocatorError::TooManyLevels {
                            scheme: grammar.scheme.clone(),
                            levels: grammar.levels.len() as u32,
                            text: self.rest_of_item(item_start),
                        });
                    }
                    // Try the level the count says; for an abbreviated end, the text may be
                    // a deeper level's numeral style, so try later levels too.
                    let mut taken = None;
                    let candidates: Vec<usize> = if start.is_some() {
                        (level_index..grammar.levels.len()).collect()
                    } else {
                        vec![level_index]
                    };
                    let save = self.pos;
                    let mut last_err = None;
                    for li in candidates {
                        self.pos = save;
                        match self.level_number(&grammar.levels[li]) {
                            Ok(Some(n)) => {
                                taken = Some(n);
                                break;
                            }
                            Ok(None) => break,
                            Err(e) => last_err = Some(e),
                        }
                    }
                    match taken {
                        Some(n) => numbers.push(n),
                        None => {
                            if let Some(e) = last_err {
                                return Err(e);
                            }
                            break;
                        }
                    }
                    if (self.peek() == Some('.') || self.peek() == Some(':'))
                        && self
                            .chars
                            .get(self.pos + 1)
                            .is_some_and(|c| c.is_ascii_alphanumeric())
                    {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                if numbers.is_empty() {
                    return Ok(None);
                }
                let parts: Vec<Part> = match start {
                    Some(s) if numbers.len() < s.parts.len() => {
                        let keep = s.parts.len() - numbers.len();
                        s.parts[..keep]
                            .iter()
                            .cloned()
                            .chain(numbers.into_iter().map(Part::Num))
                            .collect()
                    }
                    _ => numbers.into_iter().map(Part::Num).collect(),
                };
                Ok(Some(WorkLocator { parts }))
            }
        }
    }

    fn rest_of_item(&self, from: usize) -> String {
        self.chars[from..]
            .iter()
            .take_while(|c| **c != ';' && **c != ',')
            .collect::<String>()
            .trim()
            .to_owned()
    }

    fn unexpected(&self) -> LocatorError {
        LocatorError::UnexpectedCharacter {
            offset: self.base + self.pos as u32,
            character: self.peek().unwrap_or('\0'),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> GrammarTable {
        GrammarTable::builtin()
    }

    #[test]
    fn parses_the_three_schemes_and_the_catechism_form() {
        let t = table();
        let cases: &[(&str, &str, &str)] = &[
            ("WCF 1.6", "wcf:1.6", "WCF 1.6"),
            ("WCF I.vi", "wcf:1.6", "WCF 1.6"),
            ("WCF 1:6", "wcf:1.6", "WCF 1.6"),
            (
                "Westminster Confession of Faith 25.2",
                "wcf:25.2",
                "WCF 25.2",
            ),
            ("WCF 1.1-1.3", "wcf:1.1-1.3", "WCF 1.1–3"),
            ("WCF 1.1–3", "wcf:1.1-1.3", "WCF 1.1–3"),
            ("WCF 1.1-3; 2.1", "wcf:1.1-1.3;2.1", "WCF 1.1–3; 2.1"),
            ("WCF 1", "wcf:1", "WCF 1"),
            ("wcf:1.6", "wcf:1.6", "WCF 1.6"),
            ("Inst. 3.2.7", "institutes:3.2.7", "Institutes 3.2.7"),
            ("Inst 3.2.7", "institutes:3.2.7", "Institutes 3.2.7"),
            (
                "Institutes III.ii.7",
                "institutes:3.2.7",
                "Institutes 3.2.7",
            ),
            (
                "Institutes III.ii.7-9",
                "institutes:3.2.7-3.2.9",
                "Institutes 3.2.7–9",
            ),
            (
                "Institutes 3.2.7-3.3.1",
                "institutes:3.2.7-3.3.1",
                "Institutes 3.2.7–3.1",
            ),
            (
                "Calvin, Institutes 1.1.1",
                "institutes:1.1.1",
                "Institutes 1.1.1",
            ),
            ("Institutes 3.2", "institutes:3.2", "Institutes 3.2"),
            ("institutes:3.2.7", "institutes:3.2.7", "Institutes 3.2.7"),
            ("WLC Q. 1", "wlc:1", "WLC Q. 1"),
            ("WLC Q1", "wlc:1", "WLC Q. 1"),
            ("WLC Question 1", "wlc:1", "WLC Q. 1"),
            ("WSC 1", "wsc:1", "WSC Q. 1"),
            ("WSC A. 1", "wsc:1.a", "WSC A. 1"),
            ("WSC Q. 1-3", "wsc:1-3", "WSC Q. 1–3"),
            ("wsc:1.a", "wsc:1.a", "WSC A. 1"),
            ("Thayer p. 6", "thayer-1889-pages:p.6", "Thayer p. 6"),
            ("Thayer, p. 6", "thayer-1889-pages:p.6", "Thayer p. 6"),
            ("Thayer p.6", "thayer-1889-pages:p.6", "Thayer p. 6"),
            (
                "Thayer pp. 6-7",
                "thayer-1889-pages:p.6-p.7",
                "Thayer pp. 6–7",
            ),
            (
                "Thayer pp. 6–7",
                "thayer-1889-pages:p.6-p.7",
                "Thayer pp. 6–7",
            ),
            (
                "Thayer page 700",
                "thayer-1889-pages:p.700",
                "Thayer p. 700",
            ),
            (
                "thayer-1889-pages:p.6",
                "thayer-1889-pages:p.6",
                "Thayer p. 6",
            ),
        ];
        for (input, canonical, human) in cases {
            let c = parse_work_citation(input, &t).unwrap_or_else(|e| panic!("{input}: {e}"));
            assert_eq!(c.to_canonical(), *canonical, "input {input:?}");
            assert_eq!(c.to_human(&t), *human, "input {input:?}");
            assert_eq!(c.grammar_version, GRAMMAR_VERSION);
            // Canonical and human forms read back to the same ranges.
            let back = parse_work_citation(canonical, &t).unwrap();
            assert_eq!(back.ranges, c.ranges, "canonical {canonical:?}");
            let back = parse_work_citation(human, &t).unwrap();
            assert_eq!(back.ranges, c.ranges, "human {human:?}");
        }
    }

    #[test]
    fn rejects_what_it_cannot_read() {
        let t = table();
        assert!(matches!(
            parse_work_citation("", &t),
            Err(LocatorError::Empty)
        ));
        assert!(matches!(
            parse_work_citation("Hodge 1.6", &t),
            Err(LocatorError::UnknownWork { name }) if name == "Hodge"
        ));
        assert!(matches!(
            parse_work_citation("WCF", &t),
            Err(LocatorError::MissingLocator { .. })
        ));
        // A page citation needs its marker: a bare number is never a page.
        assert!(matches!(
            parse_work_citation("Thayer 6", &t),
            Err(LocatorError::MissingLocator { .. })
        ));
        assert!(matches!(
            parse_work_citation("Inst. 3.2.7.1", &t),
            Err(LocatorError::TooManyLevels { levels: 3, .. })
        ));
        // Lower-case roman at the book level is not a style the Institutes allow.
        assert!(matches!(
            parse_work_citation("Institutes iii.2.7", &t),
            Err(LocatorError::InvalidNumeral { .. })
        ));
        assert!(matches!(
            parse_work_citation("WCF IIII.1", &t),
            Err(LocatorError::InvalidNumeral { .. })
        ));
        assert!(matches!(
            parse_work_citation("WCF 1.6 Exod", &t),
            Err(LocatorError::UnexpectedCharacter { .. })
        ));
        assert!(matches!(
            parse_work_citation("WCF 2.1-1.6", &t),
            Err(LocatorError::ReversedRange { .. })
        ));
        assert!(matches!(
            parse_work_citation("WCF 0.1", &t),
            Err(LocatorError::InvalidNumeral { .. })
        ));
    }

    #[test]
    fn unified_parse_prefers_works_then_scripture() {
        let g = CitationGrammar::english();
        assert!(matches!(
            parse_citation("WCF 1.6", &g),
            Ok(Citation::Work { citation }) if citation.to_canonical() == "wcf:1.6"
        ));
        assert!(matches!(
            parse_citation("John 3:16", &g),
            Ok(Citation::Scripture { references }) if references.to_osis() == "John.3.16"
        ));
        assert!(matches!(
            parse_citation("Hodge 1.6", &g),
            Err(CitationError::Unrecognized { .. })
        ));
        assert!(matches!(
            parse_citation("Inst. 3.2.7.1", &g),
            Err(CitationError::Work(LocatorError::TooManyLevels { .. }))
        ));
        assert!(matches!(
            parse_citation("Gen 2:1-1:1", &g),
            Err(CitationError::Scripture(
                ReferenceError::ReversedRange { .. }
            ))
        ));
        assert!(matches!(
            parse_citation("  ", &g),
            Err(CitationError::Work(LocatorError::Empty))
        ));
    }

    #[test]
    fn roman_numerals_round_trip_in_both_cases() {
        for n in 1..=3999u32 {
            assert_eq!(roman_value(&roman_text(n, true), true), Some(n));
            assert_eq!(roman_value(&roman_text(n, false), false), Some(n));
        }
        assert_eq!(roman_value("IIII", true), None);
        assert_eq!(roman_value("VX", true), None);
        assert_eq!(roman_value("Ii", true), None);
        assert_eq!(roman_value("", true), None);
    }

    #[test]
    fn locators_order_and_serialize() {
        let a = WorkLocator::parse("3.2.7").unwrap();
        let b = WorkLocator::parse("3.2.10").unwrap();
        assert!(a < b);
        assert_eq!(a.to_string(), "3.2.7");
        assert_eq!(WorkLocator::parse("p.6").unwrap().to_string(), "p.6");
        assert_eq!(
            WorkLocator::parse("1.a").unwrap().parts,
            vec![Part::Num(1), Part::Tag("a".into())]
        );
        assert!(WorkLocator::parse("3..7").is_err());
        let c = parse_work_citation("Institutes III.ii.7", &table()).unwrap();
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"parts\":[3,2,7]"));
        let back: WorkCitation = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
        let err = parse_work_citation("Hodge 1", &table()).unwrap_err();
        assert!(
            serde_json::to_string(&err)
                .unwrap()
                .contains("unknown_work")
        );
    }

    #[test]
    fn every_builtin_name_resolves_to_its_grammar() {
        let t = table();
        for g in &t.grammars {
            assert_eq!(t.lookup(&g.scheme).map(|x| &x.scheme), Some(&g.scheme));
            for name in &g.names {
                assert_eq!(t.lookup(name).map(|x| &x.scheme), Some(&g.scheme), "{name}");
            }
            assert_eq!(g.version, GRAMMAR_VERSION);
            assert_eq!(g.canonical, g.work == g.scheme);
        }
        assert!(t.lookup("").is_none());
        assert!(t.lookup("Hodge").is_none());
    }
}
