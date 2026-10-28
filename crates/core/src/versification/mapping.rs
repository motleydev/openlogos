//! The many-to-many mapping record of specs/03-data-model.md ("`ReferenceMapping` supports
//! many-to-many segment sets, partial spans, certainty, mapping source and notes").
//!
//! [`VerseMap::map`] answers for one verse; [`VerseMap::map_many`] answers for a set of
//! verses with one [`ReferenceMapping`] that keeps the source set, the target set, the
//! combined status, and separately the source verses an explicit `absent` row covers
//! (omitted verses, or additions seen from the other side) and the source verses no row
//! mentions at all. Unknown is never reported as identity, and omitted is never reported as
//! unknown.

use serde::{Deserialize, Serialize};

use super::{MappingStatus, SchemeId, VerseMap, VerseMapKind};
use crate::reference::Reference;

/// A mapping between a set of verses in one scheme and a set of verses in another.
///
/// `from` and `to` are the two sides of the correspondence; `status` says how well `to`
/// stands for `from` as a whole. `absent` lists the members of `from` an explicit
/// [`VerseMapKind::Absent`] row covers (the target scheme is known to have no text for
/// them), and `unknown` the members of `from` that no row mentions at all. A member is in
/// at most one of `absent` and `unknown`; the rest reached `to`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceMapping {
    /// Scheme of `from`.
    pub from_scheme: SchemeId,
    /// The source verses, in the order given.
    pub from: Vec<Reference>,
    /// Scheme of `to`.
    pub to_scheme: SchemeId,
    /// The target verses, without duplicates, in order of first appearance; empty when
    /// `status` is [`MappingStatus::Unmapped`].
    pub to: Vec<Reference>,
    /// How well `to` stands for `from`.
    pub status: MappingStatus,
    /// Source verses an explicit `absent` row covers.
    pub absent: Vec<Reference>,
    /// Source verses no row mentions.
    pub unknown: Vec<Reference>,
    /// A calibrated certainty in `0..=1`, or `None`. Compiled map rows carry none, so
    /// [`VerseMap::map_many`] leaves this `None`; a pipeline or reviewer that has one sets it.
    pub certainty: Option<f64>,
    /// Provenance of the rows used (the `source` of each distinct row), in order of use.
    pub sources: Vec<String>,
    /// Schemes traversed, e.g. `[eng, org, lxx]` for a pivoted mapping.
    pub path: Vec<SchemeId>,
    /// Free-form notes for a reader (why a mapping is partial, which tradition a row follows).
    pub notes: Vec<String>,
}

impl ReferenceMapping {
    /// Whether every source verse reached the target scheme.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.absent.is_empty() && self.unknown.is_empty() && !self.to.is_empty()
    }
}

impl VerseMap {
    /// Maps a set of verses from scheme `from` to scheme `to` as one [`ReferenceMapping`].
    ///
    /// Each verse is mapped with [`VerseMap::map`]; the targets are unioned in order of
    /// first appearance and the status is the worst single status, raised to
    /// [`MappingStatus::Partial`] when some verses reached the target scheme and others did
    /// not. A verse whose only rows towards the target are `absent` goes to `absent`; a
    /// verse with no row at all goes to `unknown`. An empty `references` is unmapped.
    #[must_use]
    pub fn map_many(
        &self,
        references: &[Reference],
        from: &SchemeId,
        to: &SchemeId,
    ) -> ReferenceMapping {
        let mut to_refs: Vec<Reference> = Vec::new();
        let mut absent = Vec::new();
        let mut unknown = Vec::new();
        let mut sources: Vec<String> = Vec::new();
        let mut path = vec![from.clone(), to.clone()];
        let mut worst: Option<MappingStatus> = None;
        let mut reached = 0usize;

        for reference in references {
            let result = self.map(reference, from, to);
            if result.path.len() > path.len() {
                path = result.path.clone();
            }
            for source in self.sources_for(reference, from, to) {
                if !sources.contains(&source) {
                    sources.push(source);
                }
            }
            if result.status == MappingStatus::Unmapped {
                if self.is_absent(reference, from, to) {
                    absent.push(reference.clone());
                } else {
                    unknown.push(reference.clone());
                }
                continue;
            }
            reached += 1;
            worst = Some(worst.map_or(result.status, |w| w.max(result.status)));
            for target in result.targets {
                if !to_refs.contains(&target) {
                    to_refs.push(target);
                }
            }
        }

        let status = match worst {
            None => MappingStatus::Unmapped,
            Some(status) if reached < references.len() => status.max(MappingStatus::Partial),
            Some(status) => status,
        };
        let mut notes = Vec::new();
        if !absent.is_empty() {
            notes.push(format!(
                "{} of {} source verses are absent from {}",
                absent.len(),
                references.len(),
                to.as_str()
            ));
        }
        if !unknown.is_empty() {
            notes.push(format!(
                "{} of {} source verses have no mapping row towards {}",
                unknown.len(),
                references.len(),
                to.as_str()
            ));
        }
        ReferenceMapping {
            from_scheme: from.clone(),
            from: references.to_vec(),
            to_scheme: to.clone(),
            to: to_refs,
            status,
            absent,
            unknown,
            certainty: None,
            sources,
            path,
            notes,
        }
    }

    /// Whether an explicit `absent` row covers `reference` on the way from `from` to `to`,
    /// directly or on either hop of the pivot through `org`.
    fn is_absent(&self, reference: &Reference, from: &SchemeId, to: &SchemeId) -> bool {
        if self.has_absent_row(reference, from, to) {
            return true;
        }
        if from.is_org() || to.is_org() {
            return false;
        }
        let org = SchemeId::org();
        if self.has_absent_row(reference, from, &org) {
            return true;
        }
        // The first hop may succeed and every pivot verse be absent from the target.
        let first = self.map(reference, from, &org);
        !first.targets.is_empty()
            && first
                .targets
                .iter()
                .all(|pivot| self.has_absent_row(pivot, &org, to))
    }

    /// Whether a row `from -> to` marks `reference` absent, in either stored direction.
    fn has_absent_row(&self, reference: &Reference, from: &SchemeId, to: &SchemeId) -> bool {
        self.entries.iter().any(|e| {
            e.kind == VerseMapKind::Absent
                && ((&e.from_scheme == from && &e.to_scheme == to && &e.from == reference)
                    || (&e.from_scheme == to
                        && &e.to_scheme == from
                        && e.to.as_ref() == Some(reference)))
        })
    }

    /// The `source` of every row that mentions `reference` between `from` and `to`, or
    /// between either and `org`, in table order.
    fn sources_for(&self, reference: &Reference, from: &SchemeId, to: &SchemeId) -> Vec<String> {
        let org = SchemeId::org();
        let mut out = Vec::new();
        for e in &self.entries {
            let between = |a: &SchemeId, b: &SchemeId| {
                (&e.from_scheme == a && &e.to_scheme == b)
                    || (&e.from_scheme == b && &e.to_scheme == a)
            };
            let mentions = &e.from == reference || e.to.as_ref() == Some(reference);
            if mentions
                && (between(from, to) || between(from, &org) || between(&org, to))
                && !out.contains(&e.source)
            {
                out.push(e.source.clone());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::BookId;
    use crate::versification::VerseMapEntry;

    fn r(book: BookId, chapter: u32, verse: u32) -> Reference {
        Reference::verse(book, chapter, verse)
    }

    fn row(
        from_scheme: &str,
        from: Reference,
        to_scheme: &str,
        to: Option<Reference>,
        kind: VerseMapKind,
        source: &str,
    ) -> VerseMapEntry {
        VerseMapEntry {
            from_scheme: SchemeId::new(from_scheme),
            from,
            to_scheme: SchemeId::new(to_scheme),
            to,
            kind,
            source: source.to_owned(),
        }
    }

    fn s(id: &str) -> SchemeId {
        SchemeId::new(id)
    }

    /// Hebrew Malachi 3:19-24 is English Malachi 4:1-6; a synthetic edition `ed` omits
    /// English 4:5 and knows nothing about 4:6.
    fn table() -> VerseMap {
        let mut entries = Vec::new();
        for i in 0..6 {
            entries.push(row(
                "eng",
                r(BookId::Mal, 4, 1 + i),
                "org",
                Some(r(BookId::Mal, 3, 19 + i)),
                VerseMapKind::Equal,
                "Hebrew Malachi has three chapters",
            ));
        }
        for i in 0..4 {
            entries.push(row(
                "ed",
                r(BookId::Mal, 4, 1 + i),
                "org",
                Some(r(BookId::Mal, 3, 19 + i)),
                VerseMapKind::Equal,
                "edition follows English chapters",
            ));
        }
        entries.push(row(
            "org",
            r(BookId::Mal, 3, 23),
            "ed",
            None,
            VerseMapKind::Absent,
            "edition omits this verse",
        ));
        // English 13:5 and 13:6 are Hebrew 13:6.
        entries.push(row(
            "eng",
            r(BookId::Ps, 13, 5),
            "org",
            Some(r(BookId::Ps, 13, 6)),
            VerseMapKind::Merge,
            "Psalm 13 merge",
        ));
        entries.push(row(
            "eng",
            r(BookId::Ps, 13, 6),
            "org",
            Some(r(BookId::Ps, 13, 6)),
            VerseMapKind::Merge,
            "Psalm 13 merge",
        ));
        VerseMap::new(entries)
    }

    #[test]
    fn a_chapter_span_maps_many_to_many_and_exact() {
        let refs: Vec<Reference> = (1..=6).map(|v| r(BookId::Mal, 4, v)).collect();
        let m = table().map_many(&refs, &s("eng"), &s("org"));
        assert_eq!(m.status, MappingStatus::Exact);
        assert_eq!(m.from.len(), 6);
        assert_eq!(
            m.to,
            (19..=24).map(|v| r(BookId::Mal, 3, v)).collect::<Vec<_>>()
        );
        assert!(m.is_complete());
        assert_eq!(
            m.sources,
            vec!["Hebrew Malachi has three chapters".to_owned()]
        );
        assert_eq!(m.path, vec![s("eng"), s("org")]);
        assert!(m.notes.is_empty());
        assert_eq!(m.certainty, None);
    }

    #[test]
    fn merged_verses_keep_the_worst_single_status() {
        let t = table();
        let both = t.map_many(
            &[r(BookId::Ps, 13, 5), r(BookId::Ps, 13, 6)],
            &s("eng"),
            &s("org"),
        );
        // Each row is a merge, so the single-verse status is partial; the union of
        // targets is one verse. The record keeps the worst single status, which is honest:
        // the rows do not say the two together are the whole of 13:6.
        assert_eq!(both.to, vec![r(BookId::Ps, 13, 6)]);
        assert_eq!(both.status, MappingStatus::Partial);
        let one = t.map_many(&[r(BookId::Ps, 13, 5)], &s("eng"), &s("org"));
        assert_eq!(one.status, MappingStatus::Partial);
        let back = t.map_many(&[r(BookId::Ps, 13, 6)], &s("org"), &s("eng"));
        assert_eq!(back.status, MappingStatus::Exact);
        assert_eq!(back.to, vec![r(BookId::Ps, 13, 5), r(BookId::Ps, 13, 6)]);
    }

    #[test]
    fn omitted_is_absent_and_missing_is_unknown() {
        let t = table();
        let refs: Vec<Reference> = (19..=24).map(|v| r(BookId::Mal, 3, v)).collect();
        let m = t.map_many(&refs, &s("org"), &s("ed"));
        assert_eq!(m.status, MappingStatus::Partial);
        assert_eq!(m.to.len(), 4);
        assert_eq!(m.absent, vec![r(BookId::Mal, 3, 23)]);
        assert_eq!(m.unknown, vec![r(BookId::Mal, 3, 24)]);
        assert!(!m.is_complete());
        assert_eq!(m.notes.len(), 2);
        assert!(m.sources.contains(&"edition omits this verse".to_owned()));

        // Pivoted: English 4:5 reaches org 3:23, which the edition marks absent.
        let via = t.map_many(&[r(BookId::Mal, 4, 5)], &s("eng"), &s("ed"));
        assert_eq!(via.status, MappingStatus::Unmapped);
        assert_eq!(via.absent, vec![r(BookId::Mal, 4, 5)]);
        assert!(via.unknown.is_empty());
        assert_eq!(via.path, vec![s("eng"), s("org"), s("ed")]);
        let nothing = t.map_many(&[r(BookId::Mal, 4, 6)], &s("eng"), &s("ed"));
        assert_eq!(nothing.unknown, vec![r(BookId::Mal, 4, 6)]);
        assert!(nothing.absent.is_empty());
    }

    #[test]
    fn an_empty_set_is_unmapped_and_serializes() {
        let m = table().map_many(&[], &s("eng"), &s("org"));
        assert_eq!(m.status, MappingStatus::Unmapped);
        assert!(m.to.is_empty() && m.from.is_empty());
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"status\":\"unmapped\""));
        assert!(json.contains("\"certainty\":null"));
        let back: ReferenceMapping = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }
}
