//! Continuous reading in the shell (B078, specs/23): a tab's passage follows the verse at the
//! top of its viewport, linked tabs follow through versification, and a navigation leaves the
//! verses it named marked until the next one.
//!
//! References are mapped with the canon table of [`openlogos_core::canon`] between the
//! schemes the host registers per resource ([`crate::commands::Shell::set_resource_schemes`]:
//! `bsb` → `eng`, `wlc` → `org`). Without a registered scheme on both sides, or with the
//! same scheme, a follower lands on the driver's reference as written, as before B078.

use openlogos_core::canon;
use openlogos_core::reference::{Reference, ReferenceRange, ReferenceSet};
use openlogos_core::versification::{MappingStatus, SchemeId};
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use crate::palette::parse_tab_reference;
use crate::shell::Tab;
use crate::view::reference_labels;

/// How a follower's reference was found from its driver's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FollowMapping {
    /// The tab moved itself (the driver), or both resources use one scheme.
    Same,
    /// A resource has no registered scheme: the driver's reference as written.
    Unregistered,
    /// Mapped one to one.
    Exact,
    /// Mapped to a verse that only partly corresponds (English Psalm 13:5 in Hebrew 13:6).
    Partial,
    /// The verse has no counterpart; the nearest earlier verse of its chapter that has one.
    Nearest,
    /// Nothing in the chapter maps: the driver's reference as written.
    Unmapped,
}

/// Where a tab is after a command moved it: what a host relabels, and for a follower scrolls
/// to. A driver comes first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabPosition {
    /// The workspace.
    pub workspace_id: String,
    /// The tab.
    pub tab_id: String,
    /// Whether this tab drove the move (`false`: it followed).
    pub driver: bool,
    /// Whether the move came from scrolling (`tab.record_scroll`): relabel in place, and
    /// scroll a follower without rebuilding it; `false` for a navigation.
    pub scrolled: bool,
    /// The tab's reference now.
    pub reference: String,
    /// The segment to bring to the top: the reference's first verse (`John.3.16`), or
    /// `None` for a chapter without verses.
    pub segment_id: Option<String>,
    /// The identity row's label (`Jn 3:16`).
    pub reference_label: Option<String>,
    /// The tab strip's label (`Jn 3`).
    pub chapter_label: Option<String>,
    /// The verses marked on the tab, if any.
    pub highlight_range: Option<String>,
    /// How the reference was found.
    pub mapping: FollowMapping,
}

impl TabPosition {
    pub(crate) fn of(
        workspace_id: &str,
        tab: &Tab,
        driver: bool,
        scrolled: bool,
        mapping: FollowMapping,
    ) -> TabPosition {
        let reference = tab.reference.clone().unwrap_or_default();
        let (reference_label, chapter_label) = reference_labels(tab.reference.as_deref());
        TabPosition {
            workspace_id: workspace_id.to_owned(),
            tab_id: tab.id.clone(),
            driver,
            scrolled,
            segment_id: first_verse(&reference),
            reference,
            reference_label,
            chapter_label,
            highlight_range: tab.highlight_range.clone(),
            mapping,
        }
    }
}

/// The first verse of a stored reference, OSIS; `None` for a chapter-only one.
fn first_verse(reference: &str) -> Option<String> {
    let set = parse_tab_reference(reference)?;
    let start = &set.ranges[0].start;
    start
        .verse
        .map(|v| Reference::verse(start.book, start.chapter, v).to_osis())
}

/// The verses a navigation marks: the reference's first range when it names verses, OSIS
/// (`John.3.16-John.3.18`); `None` for a chapter, a book or text that does not parse.
#[must_use]
pub fn highlight_of(reference: &str) -> Option<String> {
    let set = parse_tab_reference(reference)?;
    let range = &set.ranges[0];
    (range.start.verse.is_some() && range.end.verse.is_some()).then(|| {
        ReferenceRange {
            start: plain(&range.start),
            end: plain(&range.end),
        }
        .to_osis()
    })
}

fn plain(r: &Reference) -> Reference {
    Reference {
        subverse: None,
        ..r.clone()
    }
}

/// Whether a tab's stored verse range already holds `verse`, so scrolling inside the passage
/// a navigation named keeps it (`John.3.16-John.3.18` holds 3:17). A chapter-only or single
/// verse reference holds only itself.
#[must_use]
pub fn holds(reference: Option<&str>, verse: &Reference) -> bool {
    let Some(set) = reference.and_then(parse_tab_reference) else {
        return false;
    };
    set.ranges.iter().any(|r| {
        r.start.verse.is_some()
            && r.end.verse.is_some()
            && plain(&r.start) <= *verse
            && *verse <= plain(&r.end)
    })
}

/// The locator scheme of a work's resource (B016): the registered scheme when it is a work
/// grammar's (`institutes`), not a versification. A host registers a work pack's
/// `locator_scheme` with [`crate::commands::Shell::set_resource_schemes`] as it registers an
/// edition's versification; `None` for a Bible edition or an unregistered resource.
#[must_use]
pub fn work_scheme<'a>(schemes: &'a BTreeMap<String, String>, content_id: &str) -> Option<&'a str> {
    static TABLE: std::sync::OnceLock<openlogos_core::locator::GrammarTable> =
        std::sync::OnceLock::new();
    let table = TABLE.get_or_init(openlogos_core::locator::GrammarTable::builtin);
    schemes
        .get(content_id)
        .map(String::as_str)
        .filter(|s| table.grammar(s).is_some())
}

/// Whether a tab of `follower` can follow a tab of `driver` (B016): both Bible resources (or
/// unregistered), or both of one work's scheme.
#[must_use]
pub fn same_family(schemes: &BTreeMap<String, String>, driver: &str, follower: &str) -> bool {
    work_scheme(schemes, driver) == work_scheme(schemes, follower)
}

fn scheme_of(schemes: &BTreeMap<String, String>, content_id: &str) -> Option<SchemeId> {
    schemes.get(content_id).map(|s| SchemeId::new(s))
}

/// One verse into another scheme: the first target, or the nearest earlier verse of the
/// chapter that maps.
fn map_verse(
    verse: &Reference,
    from: &SchemeId,
    to: &SchemeId,
) -> Option<(Reference, FollowMapping)> {
    let first = canon::first_verse(from, verse.book, verse.chapter).unwrap_or(1);
    let last = canon::last_verse(from, verse.book, verse.chapter)?;
    let mut v = verse.verse?.min(last);
    let mut nearest = v != verse.verse?;
    loop {
        let result = canon::map(&Reference::verse(verse.book, verse.chapter, v), from, to);
        if let Some(target) = result.targets.first() {
            let mapping = match (nearest, result.status) {
                (true, _) => FollowMapping::Nearest,
                (false, MappingStatus::Exact) => FollowMapping::Exact,
                (false, _) => FollowMapping::Partial,
            };
            return Some((target.clone(), mapping));
        }
        if v <= first {
            return None;
        }
        v -= 1;
        nearest = true;
    }
}

/// A stored reference of resource `from` in resource `to`'s scheme, with how it was found,
/// through the registered schemes ([`crate::shell::Session::resource_schemes`]).
/// A chapter maps by its first verse, and stays a chapter when that lands on the first verse
/// of a chapter (`Ps.3` → `Ps.3`, `Joel.3` → `Joel.4`, `Mal.4` → `Mal.3.19`).
#[must_use]
pub fn map_reference(
    schemes: &BTreeMap<String, String>,
    reference: &str,
    from: &str,
    to: &str,
) -> (String, FollowMapping) {
    let (Some(source), Some(target)) = (scheme_of(schemes, from), scheme_of(schemes, to)) else {
        return (reference.to_owned(), FollowMapping::Unregistered);
    };
    if source == target {
        return (reference.to_owned(), FollowMapping::Same);
    }
    let Some(set) = parse_tab_reference(reference) else {
        return (reference.to_owned(), FollowMapping::Unmapped);
    };
    let range = &set.ranges[0];
    let start = &range.start;
    let Some(start_verse) = start
        .verse
        .or_else(|| canon::first_verse(&source, start.book, start.chapter))
    else {
        return (reference.to_owned(), FollowMapping::Unmapped);
    };
    let Some((mapped, mapping)) = map_verse(
        &Reference::verse(start.book, start.chapter, start_verse),
        &source,
        &target,
    ) else {
        return (reference.to_owned(), FollowMapping::Unmapped);
    };
    if start.verse.is_none() {
        let opens_chapter =
            canon::first_verse(&target, mapped.book, mapped.chapter) == mapped.verse;
        let out = if opens_chapter {
            Reference::chapter(mapped.book, mapped.chapter).to_osis()
        } else {
            mapped.to_osis()
        };
        return (out, mapping);
    }
    let end = match (&range.end.verse, range.is_single()) {
        (Some(_), false) => map_verse(&plain(&range.end), &source, &target).map(|(r, _)| r),
        _ => None,
    };
    let out = match end {
        Some(end) if end > mapped => ReferenceSet {
            ranges: vec![ReferenceRange { start: mapped, end }],
            flags: Vec::new(),
        }
        .to_osis(),
        _ => mapped.to_osis(),
    };
    (out, mapping)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schemes() -> BTreeMap<String, String> {
        [("bsb", "eng"), ("sblgnt", "org"), ("wlc", "org")]
            .into_iter()
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
            .collect()
    }

    #[test]
    fn references_map_between_registered_schemes() {
        let s = schemes();
        let m = |r: &str, from: &str, to: &str| map_reference(&s, r, from, to);
        use FollowMapping::*;
        assert_eq!(m("Joel.2.28", "bsb", "wlc"), ("Joel.3.1".into(), Exact));
        assert_eq!(m("Joel.3", "bsb", "wlc"), ("Joel.4".into(), Exact));
        assert_eq!(m("Mal.4", "bsb", "wlc"), ("Mal.3.19".into(), Exact));
        assert_eq!(m("Ps.3", "bsb", "wlc"), ("Ps.3".into(), Exact));
        assert_eq!(m("Ps.3.1", "bsb", "wlc"), ("Ps.3.2".into(), Exact));
        assert_eq!(m("Ps.13.5", "bsb", "wlc"), ("Ps.13.6".into(), Partial));
        assert_eq!(
            m("Joel.2.27-Joel.2.29", "bsb", "wlc"),
            ("Joel.2.27-Joel.3.2".into(), Exact)
        );
        assert_eq!(
            m("Acts.19.41", "bsb", "sblgnt"),
            ("Acts.19.40".into(), Nearest)
        );
        assert_eq!(m("John.3.16", "sblgnt", "wlc"), ("John.3.16".into(), Same));
        assert_eq!(
            m("John.3.16", "bsb", "lexicon"),
            ("John.3.16".into(), Unregistered)
        );
        assert_eq!(m("Gen.1.99", "bsb", "wlc"), ("Gen.1.31".into(), Nearest));
        assert_eq!(m("Gen.51.1", "bsb", "wlc"), ("Gen.51.1".into(), Unmapped));
    }

    #[test]
    fn highlights_and_holding() {
        assert_eq!(
            highlight_of("John 3:16-18").as_deref(),
            Some("John.3.16-John.3.18")
        );
        assert_eq!(highlight_of("John.3.16").as_deref(), Some("John.3.16"));
        assert_eq!(highlight_of("John.3"), None);
        let v = |n| Reference::verse(openlogos_core::reference::BookId::John, 3, n);
        assert!(holds(Some("John.3.16-John.3.18"), &v(17)));
        assert!(!holds(Some("John.3.16-John.3.18"), &v(19)));
        assert!(holds(Some("John.3.16"), &v(16)));
        assert!(!holds(Some("John.3"), &v(16)));
    }
}
