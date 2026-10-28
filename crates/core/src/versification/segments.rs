//! Edition segments and their native labels: how a verse reference resolves to the text an
//! edition actually prints. An edition may print one segment under a bridge label such as
//! `12-13` (USFM `\v 12-13`), print one verse as two segments, or omit a verse entirely.
//! Resolving through a [`SegmentTable`] says which, instead of assuming one segment per verse
//! (specs/03-data-model.md, "bridge labels, omitted verses").

use serde::{Deserialize, Serialize};

use super::{MappingStatus, SchemeId};
use crate::reference::Reference;

/// One printed segment of an edition and the verses, in the edition's own scheme, it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentSpan {
    /// The segment id, stable within the edition (e.g. `Num.26.3-4`).
    pub segment_id: String,
    /// The label the edition prints, e.g. `3-4` for a bridge or `3` for a plain verse.
    pub native_label: String,
    /// The verses this segment holds, in order. More than one is a bridge.
    pub verses: Vec<Reference>,
}

impl SegmentSpan {
    /// Whether the segment prints more than one verse under one label.
    #[must_use]
    pub fn is_bridge(&self) -> bool {
        self.verses.len() > 1
    }
}

/// The segments of one edition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentTable {
    /// The edition.
    pub edition_id: String,
    /// The edition's native scheme, which `verses` are counted in.
    pub scheme: SchemeId,
    /// The segments, in text order.
    pub segments: Vec<SegmentSpan>,
}

/// Where a verse is printed in an edition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentResolution {
    /// The verse asked for.
    pub reference: Reference,
    /// `exact` when the segments together hold exactly this verse; `partial` when a segment
    /// also holds other verses (a bridge); `unmapped` when the edition prints no segment for
    /// it (an omitted verse).
    pub status: MappingStatus,
    /// Ids of the segments holding the verse, in text order.
    pub segment_ids: Vec<String>,
    /// Native labels of those segments, in the same order.
    pub native_labels: Vec<String>,
    /// Whether any of the segments is a bridge.
    pub bridged: bool,
}

impl SegmentTable {
    /// The segments that print `reference`.
    #[must_use]
    pub fn resolve(&self, reference: &Reference) -> SegmentResolution {
        let hits: Vec<&SegmentSpan> = self
            .segments
            .iter()
            .filter(|s| s.verses.contains(reference))
            .collect();
        let bridged = hits.iter().any(|s| s.is_bridge());
        let status = if hits.is_empty() {
            MappingStatus::Unmapped
        } else if bridged {
            MappingStatus::Partial
        } else {
            MappingStatus::Exact
        };
        SegmentResolution {
            reference: reference.clone(),
            status,
            segment_ids: hits.iter().map(|s| s.segment_id.clone()).collect(),
            native_labels: hits.iter().map(|s| s.native_label.clone()).collect(),
            bridged,
        }
    }

    /// The verses the edition prints no segment for, out of `verses`.
    #[must_use]
    pub fn omitted<'a>(&self, verses: &'a [Reference]) -> Vec<&'a Reference> {
        verses
            .iter()
            .filter(|v| !self.segments.iter().any(|s| s.verses.contains(v)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::BookId;

    fn num(verse: u32) -> Reference {
        Reference::verse(BookId::Num, 26, verse)
    }

    fn table() -> SegmentTable {
        SegmentTable {
            edition_id: "ed".to_owned(),
            scheme: SchemeId::new("eng"),
            segments: vec![
                SegmentSpan {
                    segment_id: "Num.26.2".to_owned(),
                    native_label: "2".to_owned(),
                    verses: vec![num(2)],
                },
                SegmentSpan {
                    segment_id: "Num.26.3-4".to_owned(),
                    native_label: "3-4".to_owned(),
                    verses: vec![num(3), num(4)],
                },
                // One verse printed as two segments (a paragraph break inside the verse).
                SegmentSpan {
                    segment_id: "Num.26.5a".to_owned(),
                    native_label: "5".to_owned(),
                    verses: vec![num(5)],
                },
                SegmentSpan {
                    segment_id: "Num.26.5b".to_owned(),
                    native_label: String::new(),
                    verses: vec![num(5)],
                },
            ],
        }
    }

    #[test]
    fn plain_bridged_split_and_omitted_verses_resolve_differently() {
        let t = table();
        let plain = t.resolve(&num(2));
        assert_eq!(plain.status, MappingStatus::Exact);
        assert_eq!(plain.segment_ids, vec!["Num.26.2"]);
        assert!(!plain.bridged);

        let bridged = t.resolve(&num(4));
        assert_eq!(bridged.status, MappingStatus::Partial);
        assert_eq!(bridged.segment_ids, vec!["Num.26.3-4"]);
        assert_eq!(bridged.native_labels, vec!["3-4"]);
        assert!(bridged.bridged);

        let split = t.resolve(&num(5));
        assert_eq!(split.status, MappingStatus::Exact);
        assert_eq!(split.segment_ids, vec!["Num.26.5a", "Num.26.5b"]);

        let omitted = t.resolve(&num(6));
        assert_eq!(omitted.status, MappingStatus::Unmapped);
        assert!(omitted.segment_ids.is_empty());
        assert_eq!(
            t.omitted(&[num(1), num(2), num(3), num(6)]),
            vec![&num(1), &num(6)]
        );
    }

    #[test]
    fn tables_and_resolutions_serialize() {
        let t = table();
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"native_label\":\"3-4\""));
        let back: SegmentTable = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
        let json = serde_json::to_string(&t.resolve(&num(3))).unwrap();
        assert!(json.contains("\"status\":\"partial\""));
        assert!(json.contains("\"bridged\":true"));
    }
}
