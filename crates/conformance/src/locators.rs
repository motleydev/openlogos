//! Fixtures for work locators (B068): citation parsing through the built-in grammars of
//! `openlogos_core::locator`, and section-map resolution through
//! `openlogos_retrieval::SectionMap`. Every host runs them through [`crate::run_all`].

use openlogos_core::locator::{
    Citation, CitationGrammar, GrammarTable, parse_citation, parse_work_citation,
};
use openlogos_retrieval::{LocatorScheme, SectionMap, SectionMapRow, SectionTarget};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Recorder;

/// One work citation and its expected parse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkCitationCase {
    /// The citation as written.
    pub input: String,
    /// Expected work id.
    pub work: String,
    /// Expected scheme id.
    pub scheme: String,
    /// Expected canonical form (`wcf:1.6`).
    pub canonical: String,
    /// Expected human form (`WCF 1.6`).
    pub human: String,
}

/// One citation the grammars must reject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkCitationErrorCase {
    /// The input.
    pub input: String,
    /// Expected `LocatorError` kind, as the core serializes it.
    pub error: String,
}

/// One input for the unified parser, which tries works before Scripture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnifiedCase {
    /// The input.
    pub input: String,
    /// `work`, `scripture` or `error`.
    pub kind: String,
    /// Expected canonical form for `work` and `scripture`.
    #[serde(default)]
    pub canonical: Option<String>,
    /// Expected `CitationError` kind for `error`.
    #[serde(default)]
    pub error: Option<String>,
}

/// A work-citation fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkCitationFixture {
    /// Fixture id.
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License.
    pub license: String,
    /// Source.
    pub source: String,
    /// Grammar table to parse with; only `builtin` exists.
    pub grammar_table: String,
    /// The grammar version the expectations were written against.
    pub grammar_version: String,
    /// Citations that parse.
    pub cases: Vec<WorkCitationCase>,
    /// Citations that fail.
    pub errors: Vec<WorkCitationErrorCase>,
    /// Inputs for the unified parser.
    #[serde(default)]
    pub unified: Vec<UnifiedCase>,
}

/// One resolution case: a citation, the installed editions, and what each must say.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionMapCase {
    /// What the case shows.
    pub name: String,
    /// The citation, parsed with the fixture's grammar table; its first range's start is
    /// the target.
    pub citation: String,
    /// Installed edition ids, in the order the resolution lists them.
    pub installed: Vec<String>,
    /// Expected `EditionResolution` records as the retrieval crate serializes them.
    pub editions: Value,
}

/// A section-map fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionMapFixture {
    /// Fixture id.
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License.
    pub license: String,
    /// Source.
    pub source: String,
    /// Grammar table to parse citations with; only `builtin` exists.
    pub grammar_table: String,
    /// Locator schemes the rows use.
    pub schemes: Vec<LocatorScheme>,
    /// Section-map rows of every edition.
    pub rows: Vec<SectionMapRow>,
    /// The cases.
    pub cases: Vec<SectionMapCase>,
}

#[derive(Serialize, PartialEq)]
struct WorkShape {
    work: String,
    scheme: String,
    canonical: String,
    human: String,
}

fn error_kind<E: Serialize + std::fmt::Display>(error: &E) -> String {
    serde_json::to_value(error)
        .ok()
        .and_then(|v| v.get("kind").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| error.to_string())
}

pub(crate) fn run_work_citations(fixture: &WorkCitationFixture, r: &mut Recorder) {
    if fixture.grammar_table != "builtin" {
        r.fail("grammar table exists", "builtin", &fixture.grammar_table);
        return;
    }
    let table = GrammarTable::builtin();
    r.check(
        "grammar version",
        &fixture.grammar_version,
        &openlogos_core::locator::GRAMMAR_VERSION.to_owned(),
    );
    for case in &fixture.cases {
        let actual = parse_work_citation(&case.input, &table).map(|c| WorkShape {
            work: c.work.clone(),
            scheme: c.scheme.clone(),
            canonical: c.to_canonical(),
            human: c.to_human(&table),
        });
        r.check(
            &format!("parses `{}`", case.input),
            &Ok(WorkShape {
                work: case.work.clone(),
                scheme: case.scheme.clone(),
                canonical: case.canonical.clone(),
                human: case.human.clone(),
            }),
            &actual.map_err(|e| e.to_string()),
        );
        let reparsed = parse_work_citation(&case.canonical, &table).map(|c| c.to_canonical());
        r.check(
            &format!("canonical form of `{}` is stable", case.input),
            &Ok(case.canonical.clone()),
            &reparsed.map_err(|e| e.to_string()),
        );
        let from_human = parse_work_citation(&case.human, &table).map(|c| c.to_canonical());
        r.check(
            &format!("human form of `{}` reads back", case.input),
            &Ok(case.canonical.clone()),
            &from_human.map_err(|e| e.to_string()),
        );
    }
    for case in &fixture.errors {
        let actual = match parse_work_citation(&case.input, &table) {
            Ok(c) => format!("parsed as {}", c.to_canonical()),
            Err(error) => error_kind(&error),
        };
        r.check(&format!("rejects `{}`", case.input), &case.error, &actual);
    }
    let grammar = CitationGrammar::english();
    for case in &fixture.unified {
        let (kind, detail) = match parse_citation(&case.input, &grammar) {
            Ok(Citation::Work { citation }) => ("work".to_owned(), citation.to_canonical()),
            Ok(Citation::Scripture { references }) => {
                ("scripture".to_owned(), references.to_osis())
            }
            Err(error) => ("error".to_owned(), error_kind(&error)),
        };
        let expected_detail = match case.kind.as_str() {
            "error" => case.error.clone().unwrap_or_default(),
            _ => case.canonical.clone().unwrap_or_default(),
        };
        r.check(
            &format!("unified parse of `{}`", case.input),
            &(case.kind.clone(), expected_detail),
            &(kind, detail),
        );
    }
}

pub(crate) fn run_section_maps(fixture: &SectionMapFixture, r: &mut Recorder) {
    if fixture.grammar_table != "builtin" {
        r.fail("grammar table exists", "builtin", &fixture.grammar_table);
        return;
    }
    let table = GrammarTable::builtin();
    let map = SectionMap::new(fixture.rows.clone());
    for case in &fixture.cases {
        let citation = match parse_work_citation(&case.citation, &table) {
            Ok(c) => c,
            Err(e) => {
                r.fail(&case.name, "a citation that parses", &e.to_string());
                continue;
            }
        };
        let Some(range) = citation.ranges.first() else {
            r.fail(&case.name, "a citation with a range", "none");
            continue;
        };
        let target = SectionTarget {
            scheme: citation.scheme.clone(),
            locator: range.start.to_string(),
        };
        let resolution = map.resolve(&fixture.schemes, &target, &case.installed);
        let actual = serde_json::to_value(&resolution.editions).unwrap_or(Value::Null);
        r.check(&case.name, &case.editions, &actual);
    }
}
