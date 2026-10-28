//! Work-level locators as the engine sees them (specs/19, "Cross-resource links", items 1,
//! 2 and 4): the `locator_scheme` catalog entry, the per-edition `section_map` analogous to
//! `verse_map`, resolution of a citation to every installed edition with nearest-section
//! fallbacks labeled, the "cited, not in your library" status of a target, and the
//! packager's per-scheme resolution report.
//!
//! Citation *grammars* (how `Inst. 3.2.7` becomes `institutes:3.2.7`) live in
//! `openlogos_core::locator`; this module only sees canonical scheme ids and locator strings.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::Locator;

/// The shape of a locator scheme.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LocatorSchemeKind {
    /// Dot-separated levels (`3.2.7`).
    Hierarchical,
    /// Numbered questions with an optional answer part (`1`, `1.a`).
    QuestionAnswer,
    /// Page milestones of one named print edition (`p.6`).
    Page {
        /// The edition whose pages the locators count.
        print_edition: String,
    },
}

/// One row of the `locator_scheme` catalog: a citable coordinate system of one work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocatorScheme {
    /// Scheme id, globally unique (`wcf`, `institutes`, `thayer-1889-pages`).
    pub id: String,
    /// Work the locators address (`wcf`, `institutes`, `thayer`).
    pub work: String,
    /// Human title of the scheme (`Westminster Confession of Faith, chapter and section`).
    pub title: String,
    /// Whether this is the work's canonical scheme. A work has exactly one; edges whose
    /// locator carries no `scheme:` prefix are in it.
    pub canonical: bool,
    /// Shape.
    pub kind: LocatorSchemeKind,
    /// Level names in order (`book`, `chapter`, `section`).
    pub levels: Vec<String>,
    /// Version of this catalog entry.
    pub version: String,
    /// Version of the reference grammar that reads citations into it
    /// (`openlogos_core::locator::GRAMMAR_VERSION`).
    pub grammar_version: String,
}

/// How an edition's segment corresponds to a canonical locator; the kinds of `verse_map`
/// (specs/19, "Versification").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionMapKind {
    /// One locator, one segment.
    Equal,
    /// The locator spans several segments (one row per segment).
    Split,
    /// Several locators share one segment.
    Merge,
    /// Only part of the segment belongs to the locator (or the reverse).
    Partial,
    /// The edition has no text for the locator.
    Absent,
}

impl SectionMapKind {
    /// The kind's name as the `section_map` table stores it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            SectionMapKind::Equal => "equal",
            SectionMapKind::Split => "split",
            SectionMapKind::Merge => "merge",
            SectionMapKind::Partial => "partial",
            SectionMapKind::Absent => "absent",
        }
    }

    /// Parses the stored name.
    #[must_use]
    pub fn parse(text: &str) -> Option<SectionMapKind> {
        Some(match text {
            "equal" => SectionMapKind::Equal,
            "split" => SectionMapKind::Split,
            "merge" => SectionMapKind::Merge,
            "partial" => SectionMapKind::Partial,
            "absent" => SectionMapKind::Absent,
            _ => return None,
        })
    }
}

/// One row of an edition's `section_map`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SectionMapRow {
    /// Scheme of `locator`.
    pub scheme: String,
    /// Canonical locator (`3.2.7`, `1.a`, `p.6`).
    pub locator: String,
    /// The edition.
    pub edition: String,
    /// The edition's segment id, or `None` for an `absent` row.
    pub segment: Option<String>,
    /// Correspondence.
    pub kind: SectionMapKind,
}

/// A target to resolve: a scheme and a locator in it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SectionTarget {
    /// Scheme id.
    pub scheme: String,
    /// Canonical locator.
    pub locator: String,
}

impl SectionTarget {
    /// Splits an edge locator string into scheme and locator: `thayer-1889-pages:p.6` names
    /// its scheme; a bare `3.2.7` is in the work's canonical scheme (`canonical`).
    #[must_use]
    pub fn from_locator(locator: &str, canonical: &str) -> SectionTarget {
        match locator.split_once(':') {
            Some((scheme, rest)) if !scheme.is_empty() => SectionTarget {
                scheme: scheme.to_owned(),
                locator: rest.to_owned(),
            },
            _ => SectionTarget {
                scheme: canonical.to_owned(),
                locator: locator.to_owned(),
            },
        }
    }
}

/// Why a resolution landed where it did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolutionBasis {
    /// The edition's own section map has the locator.
    Direct,
    /// The edition has no row for the locator but has rows for its subsections (`3.2`
    /// resolved through `3.2.1`, `3.2.2`, ...).
    Subsections {
        /// How many subsection rows were followed.
        count: u32,
    },
    /// A page citation of another print edition, carried to this edition through the
    /// canonical sections that start or continue on that page. Always labeled.
    NearestSection {
        /// The canonical locators the page maps to, sorted.
        via: Vec<String>,
        /// The print edition whose page milestones were used.
        print_edition: String,
    },
}

/// What one installed edition has for a target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SectionOutcome {
    /// The section map answers.
    Mapped {
        /// Correspondence kind.
        kind: SectionMapKind,
        /// Segment ids in the edition, sorted; empty for `absent`.
        segments: Vec<String>,
        /// How the answer was reached.
        basis: ResolutionBasis,
    },
    /// The edition's section map says nothing about the locator. Not the same as `absent`.
    Unmapped,
}

/// A target resolved in one edition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditionResolution {
    /// The edition.
    pub edition: String,
    /// Its outcome, serialized flat beside the edition (`outcome`, `kind`, `segments`, `basis`).
    #[serde(flatten)]
    pub outcome: SectionOutcome,
}

/// A target resolved in every installed edition of its work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionResolution {
    /// The target.
    pub target: SectionTarget,
    /// One entry per installed edition, in the order given.
    pub editions: Vec<EditionResolution>,
}

impl SectionResolution {
    /// Whether at least one edition has text for the target (mapped and not absent).
    #[must_use]
    pub fn has_text(&self) -> bool {
        self.editions.iter().any(|e| {
            matches!(
                &e.outcome,
                SectionOutcome::Mapped { kind, .. } if *kind != SectionMapKind::Absent
            )
        })
    }
}

/// The section maps of the installed editions of one work, merged.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SectionMap {
    /// Rows, sorted.
    pub rows: Vec<SectionMapRow>,
}

impl SectionMap {
    /// Builds a map from rows in any order.
    #[must_use]
    pub fn new(mut rows: Vec<SectionMapRow>) -> SectionMap {
        rows.sort();
        rows.dedup();
        SectionMap { rows }
    }

    fn direct(&self, scheme: &str, locator: &str, edition: &str) -> Vec<&SectionMapRow> {
        self.rows
            .iter()
            .filter(|r| r.scheme == scheme && r.locator == locator && r.edition == edition)
            .collect()
    }

    /// Mapped outcome from direct rows: `absent` wins, several rows are a split unless they
    /// say otherwise.
    fn outcome_of(rows: &[&SectionMapRow], basis: ResolutionBasis) -> Option<SectionOutcome> {
        if rows.is_empty() {
            return None;
        }
        if rows.iter().any(|r| r.kind == SectionMapKind::Absent) {
            return Some(SectionOutcome::Mapped {
                kind: SectionMapKind::Absent,
                segments: Vec::new(),
                basis,
            });
        }
        let segments: BTreeSet<String> = rows.iter().filter_map(|r| r.segment.clone()).collect();
        let kind = if rows.iter().any(|r| r.kind == SectionMapKind::Partial) {
            SectionMapKind::Partial
        } else if segments.len() > 1 {
            SectionMapKind::Split
        } else {
            rows[0].kind
        };
        Some(SectionOutcome::Mapped {
            kind,
            segments: segments.into_iter().collect(),
            basis,
        })
    }

    /// Resolves `target` in each of `installed` (edition ids). `schemes` supplies the page
    /// schemes' print editions and each work's canonical scheme.
    #[must_use]
    pub fn resolve(
        &self,
        schemes: &[LocatorScheme],
        target: &SectionTarget,
        installed: &[String],
    ) -> SectionResolution {
        let scheme = schemes.iter().find(|s| s.id == target.scheme);
        let canonical = scheme.and_then(|s| {
            schemes
                .iter()
                .find(|c| c.work == s.work && c.canonical)
                .map(|c| c.id.as_str())
        });
        let editions = installed
            .iter()
            .map(|edition| EditionResolution {
                edition: edition.clone(),
                outcome: self
                    .resolve_in(scheme, canonical, target, edition)
                    .unwrap_or(SectionOutcome::Unmapped),
            })
            .collect();
        SectionResolution {
            target: target.clone(),
            editions,
        }
    }

    fn resolve_in(
        &self,
        scheme: Option<&LocatorScheme>,
        canonical: Option<&str>,
        target: &SectionTarget,
        edition: &str,
    ) -> Option<SectionOutcome> {
        // 1. The edition's own rows.
        let direct = self.direct(&target.scheme, &target.locator, edition);
        if let Some(outcome) = Self::outcome_of(&direct, ResolutionBasis::Direct) {
            return Some(outcome);
        }
        // 2. A page of another print edition: page → that edition's segments → the canonical
        //    sections those segments carry → this edition.
        if let Some(LocatorSchemeKind::Page { print_edition }) = scheme.map(|s| &s.kind)
            && print_edition != edition
            && let Some(canonical) = canonical
        {
            let page_rows = self.direct(&target.scheme, &target.locator, print_edition);
            let page_segments: BTreeSet<&str> = page_rows
                .iter()
                .filter_map(|r| r.segment.as_deref())
                .collect();
            let via: BTreeSet<String> = self
                .rows
                .iter()
                .filter(|r| {
                    r.scheme == canonical
                        && r.edition == *print_edition
                        && r.segment
                            .as_deref()
                            .is_some_and(|s| page_segments.contains(s))
                })
                .map(|r| r.locator.clone())
                .collect();
            if !via.is_empty() {
                let rows: Vec<&SectionMapRow> = via
                    .iter()
                    .flat_map(|l| self.direct(canonical, l, edition))
                    .collect();
                let partial_page = page_rows.iter().any(|r| r.kind == SectionMapKind::Partial);
                let basis = ResolutionBasis::NearestSection {
                    via: via.into_iter().collect(),
                    print_edition: print_edition.clone(),
                };
                return Self::outcome_of(&rows, basis).map(|outcome| match outcome {
                    SectionOutcome::Mapped {
                        kind,
                        segments,
                        basis,
                    } if kind != SectionMapKind::Absent => SectionOutcome::Mapped {
                        kind: if partial_page {
                            SectionMapKind::Partial
                        } else {
                            kind
                        },
                        segments,
                        basis,
                    },
                    other => other,
                });
            }
        }
        // 3. A higher-level citation (`3.2`) through its subsections (`3.2.1`, ...).
        if matches!(
            scheme.map(|s| &s.kind),
            Some(LocatorSchemeKind::Hierarchical) | None
        ) {
            let prefix = format!("{}.", target.locator);
            let rows: Vec<&SectionMapRow> = self
                .rows
                .iter()
                .filter(|r| {
                    r.scheme == target.scheme
                        && r.edition == edition
                        && r.locator.starts_with(&prefix)
                })
                .collect();
            if !rows.is_empty() {
                let count = rows.len() as u32;
                return Self::outcome_of(&rows, ResolutionBasis::Subsections { count });
            }
        }
        None
    }
}

/// Why a cited target shows as "cited, not in your library".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotInLibraryReason {
    /// The work is known to the catalog but no edition is installed.
    NotInstalled,
    /// An edition is installed but the user has no license for its text.
    Restricted,
    /// The work is installed but no section map resolves the locator.
    Unresolved,
    /// No scheme is registered for the work.
    UnknownWork,
}

/// The label every unresolved, restricted or not-installed target displays under.
pub const CITED_NOT_IN_LIBRARY: &str = "cited, not in your library";

/// How a cited target can be shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TargetStatus {
    /// The target resolves in at least one installed, licensed edition.
    InLibrary {
        /// The resolution per installed edition.
        resolution: SectionResolution,
    },
    /// Shown as [`CITED_NOT_IN_LIBRARY`] with the locator and the citing context; never
    /// dropped (specs/19 item 4).
    NotInLibrary {
        /// Why.
        reason: NotInLibraryReason,
        /// What the installed editions said, when any are installed.
        resolution: Option<SectionResolution>,
    },
}

impl TargetStatus {
    /// Display label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            TargetStatus::InLibrary { .. } => "in your library",
            TargetStatus::NotInLibrary { .. } => CITED_NOT_IN_LIBRARY,
        }
    }
}

/// A cited target with its citing context and display status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitedTarget {
    /// The target locator as the edge carries it.
    pub target: Locator,
    /// Where the citation stands.
    pub cited_by: Locator,
    /// Relation type.
    pub rel: String,
    /// The citing text around the citation, when the pack carries it.
    pub context: Option<String>,
    /// Status.
    pub status: TargetStatus,
}

/// One citation's fate in the packager (specs/19 item 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionOutcome {
    /// Parsed and found in a section map (or a Scripture reference that parsed).
    Resolved,
    /// Parsed into a registered scheme but no section map has the locator.
    UnresolvedKnownWork,
    /// The grammar could not read it.
    Unparsed,
}

/// Counts for one scheme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemeRates {
    /// Scheme id, or `unknown` for citations no grammar read and no hint named.
    pub scheme: String,
    /// Resolved citations.
    pub resolved: u64,
    /// Parsed but not found.
    pub unresolved_known_work: u64,
    /// Not parsed.
    pub unparsed: u64,
}

impl SchemeRates {
    /// All citations counted.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.resolved + self.unresolved_known_work + self.unparsed
    }

    /// Resolved share, 0 to 1 (1 when nothing was counted).
    #[must_use]
    pub fn resolved_rate(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            1.0
        } else {
            self.resolved as f64 / total as f64
        }
    }
}

/// A configured minimum resolved rate for one scheme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemeMinimum {
    /// Scheme id.
    pub scheme: String,
    /// Minimum resolved share, 0 to 1.
    pub minimum_resolved_rate: f64,
}

/// Schema version of the serialized report (`schemas/resolution-report.schema.json`).
pub const RESOLUTION_REPORT_SCHEMA_VERSION: &str = "1.0";

/// The packager's resolution report for one pack: per-scheme counts, the configured
/// minimums and whether every minimum was met.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolutionReport {
    /// [`RESOLUTION_REPORT_SCHEMA_VERSION`].
    pub schema_version: String,
    /// The pack reported on.
    pub pack_id: String,
    /// The grammar version citations were parsed with.
    pub grammar_version: String,
    /// Per scheme, sorted by scheme id.
    pub schemes: Vec<SchemeRates>,
    /// Minimums applied, sorted by scheme id.
    pub minimums: Vec<SchemeMinimum>,
    /// One line per scheme below its minimum.
    pub failures: Vec<String>,
    /// Whether no scheme is below its minimum.
    pub passed: bool,
}

impl ResolutionReport {
    /// An empty report.
    #[must_use]
    pub fn new(pack_id: &str, grammar_version: &str) -> ResolutionReport {
        ResolutionReport {
            schema_version: RESOLUTION_REPORT_SCHEMA_VERSION.to_owned(),
            pack_id: pack_id.to_owned(),
            grammar_version: grammar_version.to_owned(),
            schemes: Vec::new(),
            minimums: Vec::new(),
            failures: Vec::new(),
            passed: true,
        }
    }

    /// Counts one citation under a scheme.
    pub fn record(&mut self, scheme: &str, outcome: ResolutionOutcome) {
        let index = match self
            .schemes
            .binary_search_by(|s| s.scheme.as_str().cmp(scheme))
        {
            Ok(i) => i,
            Err(i) => {
                self.schemes.insert(
                    i,
                    SchemeRates {
                        scheme: scheme.to_owned(),
                        resolved: 0,
                        unresolved_known_work: 0,
                        unparsed: 0,
                    },
                );
                i
            }
        };
        let rates = &mut self.schemes[index];
        match outcome {
            ResolutionOutcome::Resolved => rates.resolved += 1,
            ResolutionOutcome::UnresolvedKnownWork => rates.unresolved_known_work += 1,
            ResolutionOutcome::Unparsed => rates.unparsed += 1,
        }
    }

    /// The rates of one scheme.
    #[must_use]
    pub fn scheme(&self, scheme: &str) -> Option<&SchemeRates> {
        self.schemes.iter().find(|s| s.scheme == scheme)
    }

    /// Applies the configured minimums: a scheme with a minimum and no citations passes.
    pub fn apply_minimums(&mut self, minimums: &[SchemeMinimum]) {
        let mut minimums = minimums.to_vec();
        minimums.sort_by(|a, b| a.scheme.cmp(&b.scheme));
        self.failures.clear();
        for m in &minimums {
            if let Some(rates) = self.scheme(&m.scheme)
                && rates.resolved_rate() < m.minimum_resolved_rate
            {
                self.failures.push(format!(
                    "{}: resolved {} of {} ({:.1}%), minimum {:.1}%",
                    m.scheme,
                    rates.resolved,
                    rates.total(),
                    rates.resolved_rate() * 100.0,
                    m.minimum_resolved_rate * 100.0
                ));
            }
        }
        self.minimums = minimums;
        self.passed = self.failures.is_empty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schemes() -> Vec<LocatorScheme> {
        let s = |id: &str, work: &str, canonical: bool, kind: LocatorSchemeKind| LocatorScheme {
            id: id.into(),
            work: work.into(),
            title: id.into(),
            canonical,
            kind,
            levels: vec![],
            version: "1".into(),
            grammar_version: "b068-1".into(),
        };
        vec![
            s(
                "institutes",
                "institutes",
                true,
                LocatorSchemeKind::Hierarchical,
            ),
            s("thayer", "thayer", true, LocatorSchemeKind::Hierarchical),
            s(
                "thayer-1889-pages",
                "thayer",
                false,
                LocatorSchemeKind::Page {
                    print_edition: "thayer-1889".into(),
                },
            ),
        ]
    }

    fn row(
        scheme: &str,
        locator: &str,
        edition: &str,
        segment: Option<&str>,
        kind: SectionMapKind,
    ) -> SectionMapRow {
        SectionMapRow {
            scheme: scheme.into(),
            locator: locator.into(),
            edition: edition.into(),
            segment: segment.map(str::to_owned),
            kind,
        }
    }

    fn target(scheme: &str, locator: &str) -> SectionTarget {
        SectionTarget {
            scheme: scheme.into(),
            locator: locator.into(),
        }
    }

    #[test]
    fn resolves_each_kind_directly() {
        use SectionMapKind::*;
        let map = SectionMap::new(vec![
            row("institutes", "3.2.7", "beveridge", Some("b-3-2-7"), Equal),
            row("institutes", "3.2.7", "allen", Some("a-3-2-7a"), Split),
            row("institutes", "3.2.7", "allen", Some("a-3-2-7b"), Split),
            row("institutes", "3.2.8", "allen", Some("a-3-2-8-9"), Merge),
            row("institutes", "3.2.9", "allen", Some("a-3-2-8-9"), Merge),
            row("institutes", "3.2.10", "allen", Some("a-3-2-10"), Partial),
            row("institutes", "4.20.32", "allen", None, Absent),
        ]);
        let installed = vec!["allen".to_owned(), "beveridge".to_owned()];
        let r = map.resolve(&schemes(), &target("institutes", "3.2.7"), &installed);
        assert_eq!(
            r.editions[0].outcome,
            SectionOutcome::Mapped {
                kind: Split,
                segments: vec!["a-3-2-7a".into(), "a-3-2-7b".into()],
                basis: ResolutionBasis::Direct
            }
        );
        assert_eq!(
            r.editions[1].outcome,
            SectionOutcome::Mapped {
                kind: Equal,
                segments: vec!["b-3-2-7".into()],
                basis: ResolutionBasis::Direct
            }
        );
        assert!(r.has_text());
        let merge = map.resolve(&schemes(), &target("institutes", "3.2.9"), &installed);
        assert!(matches!(
            &merge.editions[0].outcome,
            SectionOutcome::Mapped { kind: Merge, .. }
        ));
        assert_eq!(merge.editions[1].outcome, SectionOutcome::Unmapped);
        let partial = map.resolve(&schemes(), &target("institutes", "3.2.10"), &installed);
        assert!(matches!(
            &partial.editions[0].outcome,
            SectionOutcome::Mapped { kind: Partial, .. }
        ));
        let absent = map.resolve(&schemes(), &target("institutes", "4.20.32"), &installed);
        assert_eq!(
            absent.editions[0].outcome,
            SectionOutcome::Mapped {
                kind: Absent,
                segments: vec![],
                basis: ResolutionBasis::Direct
            }
        );
        assert!(!absent.has_text());
        // A chapter citation resolves through its sections and says so.
        let chapter = map.resolve(&schemes(), &target("institutes", "3.2"), &installed);
        assert!(matches!(
            &chapter.editions[1].outcome,
            SectionOutcome::Mapped {
                kind: Equal,
                basis: ResolutionBasis::Subsections { count: 1 },
                ..
            }
        ));
        assert!(matches!(
            &chapter.editions[0].outcome,
            SectionOutcome::Mapped {
                kind: Partial,
                basis: ResolutionBasis::Subsections { count: 5 },
                ..
            }
        ));
    }

    #[test]
    fn page_citations_fall_back_to_the_nearest_section_and_say_so() {
        use SectionMapKind::*;
        let map = SectionMap::new(vec![
            // The 1889 print edition: entries and the pages they sit on.
            row("thayer", "agapao", "thayer-1889", Some("e-0040"), Equal),
            row("thayer", "agape", "thayer-1889", Some("e-0041"), Equal),
            row("thayer", "agapetos", "thayer-1889", Some("e-0042"), Equal),
            row(
                "thayer-1889-pages",
                "p.6",
                "thayer-1889",
                Some("e-0040"),
                Partial,
            ),
            row(
                "thayer-1889-pages",
                "p.6",
                "thayer-1889",
                Some("e-0041"),
                Equal,
            ),
            row(
                "thayer-1889-pages",
                "p.7",
                "thayer-1889",
                Some("e-0042"),
                Equal,
            ),
            // A later digitization keyed by headword only.
            row("thayer", "agapao", "thayer-ccel", Some("c-agapao"), Equal),
            row("thayer", "agape", "thayer-ccel", Some("c-agape"), Equal),
        ]);
        let both = vec!["thayer-1889".to_owned(), "thayer-ccel".to_owned()];
        let r = map.resolve(&schemes(), &target("thayer-1889-pages", "p.6"), &both);
        assert_eq!(
            r.editions[0].outcome,
            SectionOutcome::Mapped {
                kind: Partial,
                segments: vec!["e-0040".into(), "e-0041".into()],
                basis: ResolutionBasis::Direct
            }
        );
        assert_eq!(
            r.editions[1].outcome,
            SectionOutcome::Mapped {
                kind: Partial,
                segments: vec!["c-agapao".into(), "c-agape".into()],
                basis: ResolutionBasis::NearestSection {
                    via: vec!["agapao".into(), "agape".into()],
                    print_edition: "thayer-1889".into()
                }
            }
        );
        // Only the other edition installed: the fallback is the whole answer and is labeled.
        let only_ccel = vec!["thayer-ccel".to_owned()];
        let r = map.resolve(&schemes(), &target("thayer-1889-pages", "p.7"), &only_ccel);
        assert_eq!(r.editions[0].outcome, SectionOutcome::Unmapped);
        let r = map.resolve(&schemes(), &target("thayer-1889-pages", "p.6"), &only_ccel);
        assert!(matches!(
            &r.editions[0].outcome,
            SectionOutcome::Mapped {
                basis: ResolutionBasis::NearestSection { .. },
                ..
            }
        ));
        assert_eq!(
            SectionTarget::from_locator("thayer-1889-pages:p.6", "thayer"),
            target("thayer-1889-pages", "p.6")
        );
        assert_eq!(
            SectionTarget::from_locator("agape", "thayer"),
            target("thayer", "agape")
        );
    }

    #[test]
    fn the_report_counts_per_scheme_and_applies_minimums() {
        let mut report = ResolutionReport::new("commentary-x", "b068-1");
        for _ in 0..9 {
            report.record("wcf", ResolutionOutcome::Resolved);
        }
        report.record("wcf", ResolutionOutcome::UnresolvedKnownWork);
        report.record("institutes", ResolutionOutcome::Resolved);
        report.record("institutes", ResolutionOutcome::Unparsed);
        report.record("unknown", ResolutionOutcome::Unparsed);
        assert_eq!(
            report
                .schemes
                .iter()
                .map(|s| s.scheme.as_str())
                .collect::<Vec<_>>(),
            vec!["institutes", "unknown", "wcf"]
        );
        assert!((report.scheme("wcf").unwrap().resolved_rate() - 0.9).abs() < 1e-9);
        report.apply_minimums(&[
            SchemeMinimum {
                scheme: "wcf".into(),
                minimum_resolved_rate: 0.95,
            },
            SchemeMinimum {
                scheme: "institutes".into(),
                minimum_resolved_rate: 0.5,
            },
            SchemeMinimum {
                scheme: "thayer-1889-pages".into(),
                minimum_resolved_rate: 1.0,
            },
        ]);
        assert!(!report.passed);
        assert_eq!(
            report.failures,
            vec!["wcf: resolved 9 of 10 (90.0%), minimum 95.0%"]
        );
        report.apply_minimums(&[SchemeMinimum {
            scheme: "wcf".into(),
            minimum_resolved_rate: 0.9,
        }]);
        assert!(report.passed);
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"schema_version\":\"1.0\""));
        let back: ResolutionReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, report);
    }

    #[test]
    fn target_status_labels() {
        let status = TargetStatus::NotInLibrary {
            reason: NotInLibraryReason::NotInstalled,
            resolution: None,
        };
        assert_eq!(status.label(), CITED_NOT_IN_LIBRARY);
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"status\":\"not_in_library\""));
        assert!(json.contains("\"reason\":\"not_installed\""));
        assert_eq!(
            SectionMapKind::parse("partial"),
            Some(SectionMapKind::Partial)
        );
        assert_eq!(SectionMapKind::parse("nope"), None);
        for k in [
            SectionMapKind::Equal,
            SectionMapKind::Split,
            SectionMapKind::Merge,
            SectionMapKind::Partial,
            SectionMapKind::Absent,
        ] {
            assert_eq!(SectionMapKind::parse(k.name()), Some(k));
        }
    }
}
