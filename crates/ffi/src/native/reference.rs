//! `parseReference`, `formatOsis` and `formatHuman` over `core::reference`.

use openlogos_core::reference::{self as core, AliasTable, BookId};

/// One verse or chapter: `book` is the OSIS id (`John`, `Ps`), `verse` is absent for a
/// chapter-only reference, `subverse` is a grain such as `a`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Reference {
    /// OSIS book id, e.g. `John`.
    pub book: String,
    /// Chapter number, 1-based.
    pub chapter: u32,
    /// Verse number, or absent for a chapter-only reference.
    pub verse: Option<u32>,
    /// Sub-verse grain such as `a`; only with a verse.
    pub subverse: Option<String>,
}

/// An inclusive range with its canonical and human forms already rendered.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReferenceRange {
    /// First reference.
    pub start: Reference,
    /// Last reference; equal to `start` for a single reference.
    pub end: Reference,
    /// Canonical form, `John.3.16-John.3.18`.
    pub osis: String,
    /// Human form with English names, `John 3:16–18`.
    pub human: String,
}

/// Why a parsed range needs a human look (`core::reference::ReviewFlag`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ReviewFlag {
    /// A chapter without verses (`Ps 23`): its extent depends on the scheme.
    ChapterOnly {
        /// Index into `ReferenceSet.ranges`.
        range: u32,
    },
    /// The abbreviation matches several books; the first candidate was used.
    AmbiguousBook {
        /// Index into `ReferenceSet.ranges`.
        range: u32,
        /// The abbreviation as written.
        alias: String,
        /// OSIS ids of every book it can mean, most likely first.
        candidates: Vec<String>,
    },
}

/// The result of parsing one citation such as `Jn 3:16, 18; Rom 8:28-30`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReferenceSet {
    /// The core API version that produced this value.
    pub api_version: String,
    /// The ranges in the order written.
    pub ranges: Vec<ReferenceRange>,
    /// Review items the parser raised; empty for a precise, unambiguous citation.
    pub flags: Vec<ReviewFlag>,
    /// Canonical form of the whole set, ranges joined by `;`.
    pub osis: String,
    /// Human form of the whole set, `John 3:16, 18; Rom 8:28–30`.
    pub human: String,
}

/// Why a citation does not parse (`core::reference::ReferenceError`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum ReferenceError {
    /// The input had no reference in it.
    #[error("empty reference")]
    Empty,
    /// A book name or abbreviation the alias table does not know, or an OSIS id that is not one.
    #[error("unknown book `{name}`")]
    UnknownBook {
        /// The name as written.
        name: String,
    },
    /// A number without a book before it.
    #[error("reference `{text}` has no book")]
    MissingBook {
        /// The item as written.
        text: String,
    },
    /// A book with no chapter after it.
    #[error("`{text}` needs a chapter number of 1 or more")]
    MissingChapter {
        /// The item as written.
        text: String,
    },
    /// A number too large to be a chapter or verse.
    #[error("`{text}` is not a usable chapter or verse number")]
    InvalidNumber {
        /// The digits as written.
        text: String,
    },
    /// A character no reference syntax uses at this position.
    #[error("unexpected `{character}` at scalar offset {offset}")]
    UnexpectedCharacter {
        /// Offset in Unicode scalar values.
        offset: u32,
        /// The character itself, as a one-scalar string.
        character: String,
    },
    /// A range whose end precedes its start.
    #[error("range runs backwards: {start} to {end}")]
    ReversedRange {
        /// Canonical form of the start.
        start: String,
        /// Canonical form of the end.
        end: String,
    },
}

impl From<core::ReferenceError> for ReferenceError {
    fn from(error: core::ReferenceError) -> Self {
        match error {
            core::ReferenceError::Empty => ReferenceError::Empty,
            core::ReferenceError::UnknownBook { name } => ReferenceError::UnknownBook { name },
            core::ReferenceError::MissingBook { text } => ReferenceError::MissingBook { text },
            core::ReferenceError::MissingChapter { text } => {
                ReferenceError::MissingChapter { text }
            }
            core::ReferenceError::InvalidNumber { text } => ReferenceError::InvalidNumber { text },
            core::ReferenceError::UnexpectedCharacter { offset, character } => {
                ReferenceError::UnexpectedCharacter {
                    offset,
                    character: character.to_string(),
                }
            }
            core::ReferenceError::ReversedRange { start, end } => {
                ReferenceError::ReversedRange { start, end }
            }
        }
    }
}

impl From<core::Reference> for Reference {
    fn from(reference: core::Reference) -> Self {
        Reference {
            book: reference.book.as_osis().to_owned(),
            chapter: reference.chapter,
            verse: reference.verse,
            subverse: reference.subverse,
        }
    }
}

impl TryFrom<Reference> for core::Reference {
    type Error = ReferenceError;

    fn try_from(reference: Reference) -> Result<Self, Self::Error> {
        let book = BookId::from_osis(&reference.book).ok_or(ReferenceError::UnknownBook {
            name: reference.book,
        })?;
        Ok(core::Reference {
            book,
            chapter: reference.chapter,
            verse: reference.verse,
            subverse: reference.subverse,
        })
    }
}

impl ReferenceRange {
    fn from_core(range: core::ReferenceRange, table: &AliasTable) -> Self {
        ReferenceRange {
            osis: range.to_osis(),
            human: range.to_human(table),
            start: range.start.into(),
            end: range.end.into(),
        }
    }
}

impl From<core::ReviewFlag> for ReviewFlag {
    fn from(flag: core::ReviewFlag) -> Self {
        match flag {
            core::ReviewFlag::ChapterOnly { range } => ReviewFlag::ChapterOnly { range },
            core::ReviewFlag::AmbiguousBook {
                range,
                alias,
                candidates,
            } => ReviewFlag::AmbiguousBook {
                range,
                alias,
                candidates: candidates
                    .into_iter()
                    .map(|book| book.as_osis().to_owned())
                    .collect(),
            },
        }
    }
}

impl ReferenceSet {
    fn from_core(set: core::ReferenceSet, table: &AliasTable) -> Self {
        ReferenceSet {
            api_version: openlogos_core::api_version().to_owned(),
            osis: set.to_osis(),
            human: set.to_human(table),
            ranges: set
                .ranges
                .into_iter()
                .map(|range| ReferenceRange::from_core(range, table))
                .collect(),
            flags: set.flags.into_iter().map(ReviewFlag::from).collect(),
        }
    }
}

/// Parses a citation with the English alias table: `Jn 3:16, 18; Rom 8:28-30`, `Ps.23`,
/// `1 Cor 13:4-7`. See `core::reference::parse` for the accepted syntaxes.
#[uniffi::export]
pub fn parse_reference(text: String) -> Result<ReferenceSet, ReferenceError> {
    let table = AliasTable::english();
    let set = core::parse(&text, &table)?;
    Ok(ReferenceSet::from_core(set, &table))
}

/// Canonical form of one reference: `John.3.16`, `Ps.23`, `Gen.1.1!a`.
#[uniffi::export]
pub fn format_osis(reference: Reference) -> Result<String, ReferenceError> {
    let reference = core::Reference::try_from(reference)?;
    Ok(reference.to_osis())
}

/// Human form of one reference with English names: `John 3:16`, `Ps 23`, `Jude 3`.
#[uniffi::export]
pub fn format_human(reference: Reference) -> Result<String, ReferenceError> {
    let reference = core::Reference::try_from(reference)?;
    Ok(reference.to_human(&AliasTable::english()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_host_test_citation() {
        let set = parse_reference("Jn 3:16, 18; Rom 8:28-30".to_owned()).unwrap();
        assert_eq!(set.api_version, openlogos_core::API_VERSION);
        assert_eq!(set.osis, "John.3.16;John.3.18;Rom.8.28-Rom.8.30");
        assert_eq!(set.human, "John 3:16, 18; Rom 8:28–30");
        assert_eq!(set.ranges.len(), 3);
        assert_eq!(set.ranges[2].start.verse, Some(28));
        assert_eq!(set.ranges[2].end.verse, Some(30));
        assert_eq!(set.ranges[2].osis, "Rom.8.28-Rom.8.30");
        assert!(set.flags.is_empty());
    }

    #[test]
    fn flags_and_errors_cross_the_boundary() {
        let set = parse_reference("Ph 1:1; Ps 23".to_owned()).unwrap();
        assert!(matches!(
            &set.flags[0],
            ReviewFlag::AmbiguousBook { range: 0, alias, candidates }
                if alias == "Ph" && candidates.contains(&"Phil".to_owned())
        ));
        assert_eq!(set.flags[1], ReviewFlag::ChapterOnly { range: 1 });
        assert_eq!(
            parse_reference("Nope 3:16".to_owned()),
            Err(ReferenceError::UnknownBook {
                name: "Nope".to_owned()
            })
        );
        assert_eq!(parse_reference(String::new()), Err(ReferenceError::Empty));
        assert!(matches!(
            parse_reference("Jn 3:16 ?".to_owned()),
            Err(ReferenceError::UnexpectedCharacter { character, .. }) if character == "?"
        ));
    }

    #[test]
    fn formats_round_trip_through_the_record() {
        let set = parse_reference("Jude 3".to_owned()).unwrap();
        let reference = set.ranges[0].start.clone();
        assert_eq!(format_osis(reference.clone()).unwrap(), "Jude.1.3");
        assert_eq!(format_human(reference).unwrap(), "Jude 3");
        let bad = Reference {
            book: "Nope".to_owned(),
            chapter: 1,
            verse: None,
            subverse: None,
        };
        assert_eq!(
            format_osis(bad),
            Err(ReferenceError::UnknownBook {
                name: "Nope".to_owned()
            })
        );
    }
}
