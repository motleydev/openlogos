//! `mapVerse` over `core::versification`, with the conformance crate's fixture table as the
//! default verse map until packages carry their own (specs/19).

use openlogos_conformance::FixtureRow;
use openlogos_core::versification::{self as core, SchemeId, VerseMap};

use super::reference::{Reference, ReferenceError};

/// Confidence of a mapping (`core::versification::MappingStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MappingStatus {
    /// The targets together are exactly the source verse.
    Exact,
    /// The source is only part of a target, or part of the source has no target.
    Partial,
    /// The rows disagree; the targets are candidates for a person to choose between.
    Ambiguous,
    /// No row leads to the target scheme; `targets` is empty.
    Unmapped,
}

impl From<core::MappingStatus> for MappingStatus {
    fn from(status: core::MappingStatus) -> Self {
        match status {
            core::MappingStatus::Exact => MappingStatus::Exact,
            core::MappingStatus::Partial => MappingStatus::Partial,
            core::MappingStatus::Ambiguous => MappingStatus::Ambiguous,
            core::MappingStatus::Unmapped => MappingStatus::Unmapped,
        }
    }
}

/// The outcome of [`map_verse`].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MappingResult {
    /// The core API version that produced this value.
    pub api_version: String,
    /// How well the targets stand for the source.
    pub status: MappingStatus,
    /// Target verses in scheme order, without duplicates; empty when unmapped.
    pub targets: Vec<Reference>,
    /// Scheme names traversed, e.g. `[eng, org, lxx]` for a pivoted mapping.
    pub path: Vec<String>,
}

impl From<core::MappingResult> for MappingResult {
    fn from(result: core::MappingResult) -> Self {
        MappingResult {
            api_version: openlogos_core::api_version().to_owned(),
            status: result.status.into(),
            targets: result.targets.into_iter().map(Reference::from).collect(),
            path: result
                .path
                .into_iter()
                .map(|scheme| scheme.as_str().to_owned())
                .collect(),
        }
    }
}

/// How one source verse relates to one target verse in a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum VerseMapRowKind {
    /// One to one.
    Equal,
    /// The source verse is one of several target verses (one row per target).
    Split,
    /// The source verse is one of several that make up the target verse.
    Merge,
    /// Only part of the source corresponds to part of the target.
    Partial,
    /// The source verse has no counterpart in the target scheme.
    Absent,
}

impl From<VerseMapRowKind> for core::VerseMapKind {
    fn from(kind: VerseMapRowKind) -> Self {
        match kind {
            VerseMapRowKind::Equal => core::VerseMapKind::Equal,
            VerseMapRowKind::Split => core::VerseMapKind::Split,
            VerseMapRowKind::Merge => core::VerseMapKind::Merge,
            VerseMapRowKind::Partial => core::VerseMapKind::Partial,
            VerseMapRowKind::Absent => core::VerseMapKind::Absent,
        }
    }
}

/// One row of a verse map as a host supplies it: references as OSIS strings, with
/// same-chapter ranges (`Ps.9.1-Ps.9.20`) expanded verse by verse. Same shape as the rows
/// of the `psalm-9-10-numbering` conformance fixture.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VerseMapRow {
    /// Scheme of `from`, e.g. `eng`.
    pub from_scheme: String,
    /// Source verse or same-chapter range.
    pub from: String,
    /// Scheme of `to`, normally `org`.
    pub to_scheme: String,
    /// Target verse or range of the same length; absent for an `Absent` row.
    pub to: Option<String>,
    /// The relation.
    pub kind: VerseMapRowKind,
    /// Where the row comes from, for provenance.
    pub source: String,
}

impl From<VerseMapRow> for FixtureRow {
    fn from(row: VerseMapRow) -> Self {
        FixtureRow {
            from_scheme: row.from_scheme,
            from: row.from,
            to_scheme: row.to_scheme,
            to: row.to,
            kind: row.kind.into(),
            source: row.source,
        }
    }
}

/// Why a verse cannot be mapped at all (a mapping with no row is a result, not an error).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum VersificationError {
    /// The reference names a book that is not an OSIS id.
    #[error("unknown book `{name}`")]
    UnknownBook {
        /// The book as given.
        name: String,
    },
    /// A supplied row does not expand: not a verse, or ranges of different lengths.
    #[error("invalid verse map: {message}")]
    InvalidVerseMap {
        /// Which row and why.
        message: String,
    },
}

impl From<ReferenceError> for VersificationError {
    fn from(error: ReferenceError) -> Self {
        match error {
            ReferenceError::UnknownBook { name } => VersificationError::UnknownBook { name },
            other => VersificationError::InvalidVerseMap {
                message: other.to_string(),
            },
        }
    }
}

/// Maps one verse from `from_scheme` to `to_scheme` (`core::versification::VerseMap::map`).
///
/// `rows` is the verse map; an empty list means the conformance crate's fixture table
/// (Psalms 9, 10 and 13:5–6 between `org`, `eng` and `lxx`), which is the only table the
/// core ships until packages carry their own. A reference with no row is `Unmapped`.
#[uniffi::export]
pub fn map_verse(
    reference: Reference,
    from_scheme: String,
    to_scheme: String,
    rows: Vec<VerseMapRow>,
) -> Result<MappingResult, VersificationError> {
    let reference = openlogos_core::reference::Reference::try_from(reference)?;
    let map = if rows.is_empty() {
        fixture_verse_map()?
    } else {
        verse_map(rows)?
    };
    Ok(map
        .map(
            &reference,
            &SchemeId::new(&from_scheme),
            &SchemeId::new(&to_scheme),
        )
        .into())
}

fn verse_map(rows: Vec<VerseMapRow>) -> Result<VerseMap, VersificationError> {
    let rows: Vec<FixtureRow> = rows.into_iter().map(FixtureRow::from).collect();
    openlogos_conformance::verse_map_from_rows(&rows)
        .map_err(|message| VersificationError::InvalidVerseMap { message })
}

fn fixture_verse_map() -> Result<VerseMap, VersificationError> {
    openlogos_conformance::verse_map_from_rows(&openlogos_conformance::fixture_rows())
        .map_err(|message| VersificationError::InvalidVerseMap { message })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::parse_reference;

    fn reference(text: &str) -> Reference {
        parse_reference(text.to_owned()).unwrap().ranges[0]
            .start
            .clone()
    }

    #[test]
    fn maps_through_the_fixture_table() {
        let result = map_verse(reference("Ps 9:1"), "eng".into(), "org".into(), vec![]).unwrap();
        assert_eq!(result.status, MappingStatus::Exact);
        assert_eq!(result.targets[0].verse, Some(2));
        assert_eq!(result.path, vec!["eng", "org"]);

        let result = map_verse(reference("Ps 10:1"), "eng".into(), "lxx".into(), vec![]).unwrap();
        assert_eq!(result.status, MappingStatus::Exact);
        assert_eq!(result.targets[0].chapter, 9);
        assert_eq!(result.targets[0].verse, Some(22));
        assert_eq!(result.path, vec!["eng", "org", "lxx"]);

        let result = map_verse(reference("Gen 1:1"), "eng".into(), "org".into(), vec![]).unwrap();
        assert_eq!(result.status, MappingStatus::Unmapped);
        assert!(result.targets.is_empty());
    }

    #[test]
    fn maps_through_supplied_rows_and_rejects_bad_ones() {
        let row = VerseMapRow {
            from_scheme: "test".into(),
            from: "Gen.1.1-Gen.1.3".into(),
            to_scheme: "org".into(),
            to: Some("Gen.1.2-Gen.1.4".into()),
            kind: VerseMapRowKind::Equal,
            source: "unit test".into(),
        };
        let result = map_verse(
            reference("Gen 1:3"),
            "test".into(),
            "org".into(),
            vec![row.clone()],
        )
        .unwrap();
        assert_eq!(result.targets[0].verse, Some(4));

        let bad = VerseMapRow {
            to: Some("Gen.1.2".into()),
            ..row
        };
        assert!(matches!(
            map_verse(reference("Gen 1:3"), "test".into(), "org".into(), vec![bad]),
            Err(VersificationError::InvalidVerseMap { .. })
        ));
        let unknown = Reference {
            book: "Nope".into(),
            chapter: 1,
            verse: Some(1),
            subverse: None,
        };
        assert_eq!(
            map_verse(unknown, "eng".into(), "org".into(), vec![]),
            Err(VersificationError::UnknownBook {
                name: "Nope".into()
            })
        );
    }
}
