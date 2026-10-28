//! Versification schemes and the compiled maps between them.
//!
//! Every text is stored in its native scheme; a [`VerseMap`] holds rows from a scheme to the
//! Paratext/Copenhagen `org` scheme (the pivot), so N schemes need N maps
//! (specs/19-search-and-retrieval.md). [`VerseMap::map`] follows a direct row when one
//! exists in either direction and otherwise pivots through `org`. The rows come from the
//! caller: the package crate loads them from a pack, the conformance crate ships the Psalm
//! 9/10 fixture table. Nothing here invents a mapping: a reference with no row is
//! [`MappingStatus::Unmapped`], never silently identity (specs/03-data-model.md).

use serde::{Deserialize, Serialize};

use crate::reference::Reference;

/// Name of a versification scheme, such as `org`, `eng`, `lxx` or `vul`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemeId(pub String);

impl SchemeId {
    /// The pivot scheme every map is compiled against.
    pub const ORG: &'static str = "org";

    /// A scheme by name.
    #[must_use]
    pub fn new(id: &str) -> SchemeId {
        SchemeId(id.to_owned())
    }

    /// The `org` pivot scheme.
    #[must_use]
    pub fn org() -> SchemeId {
        SchemeId::new(Self::ORG)
    }

    /// Whether this is the pivot scheme.
    #[must_use]
    pub fn is_org(&self) -> bool {
        self.0 == Self::ORG
    }

    /// The name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// How one source verse relates to one target verse in a map row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerseMapKind {
    /// The verses correspond one to one.
    Equal,
    /// The source verse is one of several target verses (one row per target).
    Split,
    /// The source verse is one of several that make up the target verse.
    Merge,
    /// Only part of the source corresponds to part of the target; sub-verse grains say which.
    Partial,
    /// The source verse has no counterpart in the target scheme.
    Absent,
}

/// One row of a compiled verse map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerseMapEntry {
    /// Scheme of `from`.
    pub from_scheme: SchemeId,
    /// The source verse.
    pub from: Reference,
    /// Scheme of `to`.
    pub to_scheme: SchemeId,
    /// The target verse; `None` when `kind` is [`VerseMapKind::Absent`].
    pub to: Option<Reference>,
    /// The relation between `from` and `to`.
    pub kind: VerseMapKind,
    /// Where the row comes from (edition comparison, a `.vrs` file, a reviewer), for provenance.
    pub source: String,
}

/// Confidence of a mapping result, as specs/03-data-model.md defines it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingStatus {
    /// The targets together are exactly the source verse.
    Exact,
    /// The source is only part of a target, or part of the source has no target.
    Partial,
    /// The rows disagree, so the targets are candidates for a person to choose between.
    Ambiguous,
    /// No row leads to the target scheme; `targets` is empty.
    Unmapped,
}

/// The outcome of [`VerseMap::map`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingResult {
    /// How well the targets stand for the source.
    pub status: MappingStatus,
    /// Target verses in scheme order, without duplicates; empty when unmapped.
    pub targets: Vec<Reference>,
    /// Schemes traversed, e.g. `[eng, org, lxx]` for a pivoted mapping.
    pub path: Vec<SchemeId>,
}

impl MappingResult {
    fn unmapped(path: Vec<SchemeId>) -> MappingResult {
        MappingResult {
            status: MappingStatus::Unmapped,
            targets: Vec::new(),
            path,
        }
    }
}

/// Rows between schemes, normally from each scheme to `org`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VerseMap {
    /// The rows, in any order.
    pub entries: Vec<VerseMapEntry>,
}

/// Targets and kinds found for one source verse in one direction.
struct Step {
    status: MappingStatus,
    targets: Vec<Reference>,
}

impl VerseMap {
    /// A map over these rows.
    #[must_use]
    pub fn new(entries: Vec<VerseMapEntry>) -> VerseMap {
        VerseMap { entries }
    }

    /// Maps one verse from scheme `from` to scheme `to`.
    ///
    /// Uses rows from `from` to `to`, or rows from `to` to `from` read backwards (a split
    /// read backwards is a merge), and otherwise pivots through `org`. Only verse references
    /// are mapped: a chapter-only reference is reported unmapped.
    #[must_use]
    pub fn map(&self, reference: &Reference, from: &SchemeId, to: &SchemeId) -> MappingResult {
        if reference.verse.is_none() {
            return MappingResult::unmapped(vec![from.clone(), to.clone()]);
        }
        if from == to {
            return MappingResult {
                status: MappingStatus::Exact,
                targets: vec![reference.clone()],
                path: vec![from.clone()],
            };
        }
        if let Some(step) = self.step(reference, from, to) {
            return MappingResult {
                status: step.status,
                targets: step.targets,
                path: vec![from.clone(), to.clone()],
            };
        }
        if from.is_org() || to.is_org() {
            return MappingResult::unmapped(vec![from.clone(), to.clone()]);
        }

        let org = SchemeId::org();
        let path = vec![from.clone(), org.clone(), to.clone()];
        let Some(first) = self.step(reference, from, &org) else {
            return MappingResult::unmapped(path);
        };
        let mut status = first.status;
        let mut targets: Vec<Reference> = Vec::new();
        let mut missing = false;
        for pivot in &first.targets {
            match self.step(pivot, &org, to) {
                Some(second) => {
                    status = status.max(second.status);
                    for target in second.targets {
                        if !targets.contains(&target) {
                            targets.push(target);
                        }
                    }
                }
                None => missing = true,
            }
        }
        if targets.is_empty() {
            return MappingResult::unmapped(path);
        }
        if missing {
            // Part of the source reached the target scheme and part did not.
            status = status.max(MappingStatus::Partial);
        }
        MappingResult {
            status,
            targets,
            path,
        }
    }

    /// One hop: rows from `from` to `to`, or rows from `to` to `from` read backwards.
    /// `None` when no row mentions the verse in this direction.
    fn step(&self, reference: &Reference, from: &SchemeId, to: &SchemeId) -> Option<Step> {
        let mut kinds: Vec<(VerseMapKind, Option<Reference>)> = self
            .entries
            .iter()
            .filter(|e| &e.from_scheme == from && &e.to_scheme == to && &e.from == reference)
            .map(|e| (e.kind, e.to.clone()))
            .collect();
        if kinds.is_empty() {
            kinds = self
                .entries
                .iter()
                .filter(|e| {
                    &e.from_scheme == to && &e.to_scheme == from && e.to.as_ref() == Some(reference)
                })
                .map(|e| (invert(e.kind), Some(e.from.clone())))
                .collect();
        }
        if kinds.is_empty() {
            return None;
        }

        let mut targets: Vec<Reference> = Vec::new();
        let mut equal_targets = 0usize;
        let mut status = MappingStatus::Exact;
        for (kind, target) in kinds {
            match kind {
                VerseMapKind::Absent => {}
                VerseMapKind::Equal => equal_targets += 1,
                VerseMapKind::Split => {}
                VerseMapKind::Merge | VerseMapKind::Partial => {
                    status = status.max(MappingStatus::Partial);
                }
            }
            if let Some(target) = target
                && !targets.contains(&target)
            {
                targets.push(target);
            }
        }
        if targets.is_empty() {
            return Some(Step {
                status: MappingStatus::Unmapped,
                targets,
            });
        }
        // Two rows each claiming to be the one equal counterpart contradict each other.
        if equal_targets > 1 {
            status = MappingStatus::Ambiguous;
        }
        Some(Step { status, targets })
    }
}

/// The kind of a row read from target to source.
fn invert(kind: VerseMapKind) -> VerseMapKind {
    match kind {
        VerseMapKind::Split => VerseMapKind::Merge,
        VerseMapKind::Merge => VerseMapKind::Split,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::BookId;

    fn ps(chapter: u32, verse: u32) -> Reference {
        Reference::verse(BookId::Ps, chapter, verse)
    }

    fn row(
        from_scheme: &str,
        from: Reference,
        to_scheme: &str,
        to: Option<Reference>,
        kind: VerseMapKind,
    ) -> VerseMapEntry {
        VerseMapEntry {
            from_scheme: SchemeId::new(from_scheme),
            from,
            to_scheme: SchemeId::new(to_scheme),
            to,
            kind,
            source: "synthetic unit-test row".to_owned(),
        }
    }

    /// A small table shaped like the real Psalm 9/10 data: English verse n is Hebrew verse
    /// n+1 in Psalm 9, Greek Psalm 9:22 onwards is Hebrew Psalm 10, plus synthetic rows for
    /// the merge, partial, absent and ambiguous cases.
    fn table() -> VerseMap {
        let mut entries = vec![
            row("eng", ps(9, 0), "org", Some(ps(9, 1)), VerseMapKind::Equal),
            row("eng", ps(9, 1), "org", Some(ps(9, 2)), VerseMapKind::Equal),
            row(
                "eng",
                ps(10, 1),
                "org",
                Some(ps(10, 1)),
                VerseMapKind::Equal,
            ),
            row("lxx", ps(9, 2), "org", Some(ps(9, 2)), VerseMapKind::Equal),
            row(
                "lxx",
                ps(9, 22),
                "org",
                Some(ps(10, 1)),
                VerseMapKind::Equal,
            ),
            // English 13:5 and 13:6 together are Hebrew 13:6.
            row(
                "eng",
                ps(13, 5),
                "org",
                Some(ps(13, 6)),
                VerseMapKind::Merge,
            ),
            row(
                "eng",
                ps(13, 6),
                "org",
                Some(ps(13, 6)),
                VerseMapKind::Merge,
            ),
            // A verse with no counterpart.
            row("eng", ps(99, 1), "org", None, VerseMapKind::Absent),
            // Contradictory rows.
            row(
                "eng",
                ps(98, 1),
                "org",
                Some(ps(98, 1)),
                VerseMapKind::Equal,
            ),
            row(
                "eng",
                ps(98, 1),
                "org",
                Some(ps(98, 2)),
                VerseMapKind::Equal,
            ),
            // A partial correspondence with sub-verse grains.
            row(
                "eng",
                ps(97, 1),
                "org",
                Some(ps(97, 1).with_subverse("b")),
                VerseMapKind::Partial,
            ),
        ];
        // One target of a split pivots on, the other does not.
        entries.push(row(
            "eng",
            ps(96, 1),
            "org",
            Some(ps(96, 1)),
            VerseMapKind::Split,
        ));
        entries.push(row(
            "eng",
            ps(96, 1),
            "org",
            Some(ps(96, 2)),
            VerseMapKind::Split,
        ));
        entries.push(row(
            "lxx",
            ps(96, 1),
            "org",
            Some(ps(96, 1)),
            VerseMapKind::Equal,
        ));
        VerseMap::new(entries)
    }

    fn s(id: &str) -> SchemeId {
        SchemeId::new(id)
    }

    #[test]
    fn same_scheme_is_identity() {
        let result = table().map(&ps(9, 1), &s("eng"), &s("eng"));
        assert_eq!(result.status, MappingStatus::Exact);
        assert_eq!(result.targets, vec![ps(9, 1)]);
        assert_eq!(result.path, vec![s("eng")]);
    }

    #[test]
    fn direct_rows_map_both_ways() {
        let t = table();
        let forward = t.map(&ps(9, 1), &s("eng"), &s("org"));
        assert_eq!(forward.status, MappingStatus::Exact);
        assert_eq!(forward.targets, vec![ps(9, 2)]);
        let backward = t.map(&ps(9, 2), &s("org"), &s("eng"));
        assert_eq!(backward.status, MappingStatus::Exact);
        assert_eq!(backward.targets, vec![ps(9, 1)]);
        assert_eq!(backward.path, vec![s("org"), s("eng")]);
    }

    #[test]
    fn pivots_through_org_when_no_direct_row_exists() {
        let t = table();
        let result = t.map(&ps(9, 1), &s("eng"), &s("lxx"));
        assert_eq!(result.status, MappingStatus::Exact);
        assert_eq!(result.targets, vec![ps(9, 2)]);
        assert_eq!(result.path, vec![s("eng"), s("org"), s("lxx")]);
        let result = t.map(&ps(9, 22), &s("lxx"), &s("eng"));
        assert_eq!(result.targets, vec![ps(10, 1)]);
    }

    #[test]
    fn merge_is_partial_forwards_and_exact_backwards() {
        let t = table();
        let forward = t.map(&ps(13, 5), &s("eng"), &s("org"));
        assert_eq!(forward.status, MappingStatus::Partial);
        assert_eq!(forward.targets, vec![ps(13, 6)]);
        let backward = t.map(&ps(13, 6), &s("org"), &s("eng"));
        assert_eq!(backward.status, MappingStatus::Exact);
        assert_eq!(backward.targets, vec![ps(13, 5), ps(13, 6)]);
    }

    #[test]
    fn partial_kind_keeps_the_subverse_grain() {
        let result = table().map(&ps(97, 1), &s("eng"), &s("org"));
        assert_eq!(result.status, MappingStatus::Partial);
        assert_eq!(result.targets, vec![ps(97, 1).with_subverse("b")]);
    }

    #[test]
    fn absent_missing_and_chapter_only_are_unmapped() {
        let t = table();
        for (reference, from, to) in [
            (ps(99, 1), "eng", "org"),
            (ps(11, 1), "eng", "org"),
            (ps(9, 1), "eng", "vul"),
            (ps(9, 1), "org", "vul"),
            (Reference::chapter(BookId::Ps, 9), "eng", "org"),
        ] {
            let result = t.map(&reference, &s(from), &s(to));
            assert_eq!(result.status, MappingStatus::Unmapped, "{reference}");
            assert!(result.targets.is_empty());
        }
        assert_eq!(
            t.map(&ps(9, 1), &s("eng"), &s("vul")).path,
            vec![s("eng"), s("org"), s("vul")]
        );
    }

    #[test]
    fn contradictory_rows_are_ambiguous() {
        let result = table().map(&ps(98, 1), &s("eng"), &s("org"));
        assert_eq!(result.status, MappingStatus::Ambiguous);
        assert_eq!(result.targets, vec![ps(98, 1), ps(98, 2)]);
    }

    #[test]
    fn a_split_that_only_partly_reaches_the_target_scheme_is_partial() {
        let result = table().map(&ps(96, 1), &s("eng"), &s("lxx"));
        assert_eq!(result.status, MappingStatus::Partial);
        assert_eq!(result.targets, vec![ps(96, 1)]);
    }

    #[test]
    fn rows_and_results_serialize() {
        let t = table();
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"kind\":\"merge\""));
        let back: VerseMap = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
        let result = t.map(&ps(9, 1), &s("eng"), &s("lxx"));
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"status\":\"exact\""));
        assert!(json.contains("\"path\":[\"eng\",\"org\",\"lxx\"]"));
    }
}
