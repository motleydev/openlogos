//! `runConformance` and `conformanceFixturesJson` over `crates/conformance`, so every host
//! runs the same cases and can show the fixtures.

use openlogos_conformance as core;

/// One expected-versus-actual comparison (`conformance::CaseResult`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CaseResult {
    /// The fixture id.
    pub fixture: String,
    /// The case name within the fixture.
    pub name: String,
    /// Whether expected and actual agree.
    pub passed: bool,
    /// The expected value as JSON.
    pub expected: String,
    /// The actual value as JSON.
    pub actual: String,
}

/// The whole suite's outcome (`conformance::ConformanceReport`); a host asserts `failed == 0`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ConformanceReport {
    /// The core API version the suite ran against.
    pub api_version: String,
    /// Cases that passed.
    pub passed: u32,
    /// Cases that failed.
    pub failed: u32,
    /// Every case, in fixture order.
    pub cases: Vec<CaseResult>,
}

impl From<core::CaseResult> for CaseResult {
    fn from(case: core::CaseResult) -> Self {
        CaseResult {
            fixture: case.fixture,
            name: case.name,
            passed: case.passed,
            expected: case.expected,
            actual: case.actual,
        }
    }
}

impl From<core::ConformanceReport> for ConformanceReport {
    fn from(report: core::ConformanceReport) -> Self {
        ConformanceReport {
            api_version: report.api_version,
            passed: report.passed,
            failed: report.failed,
            cases: report.cases.into_iter().map(CaseResult::from).collect(),
        }
    }
}

/// Runs every conformance case through the core on this platform (`conformance::run_all`).
#[uniffi::export]
#[must_use]
pub fn run_conformance() -> ConformanceReport {
    core::run_all().into()
}

/// Every fixture as one JSON array (`conformance::fixtures_json`), for hosts that read the
/// fixture texts, tokens and expectations themselves.
#[uniffi::export]
#[must_use]
pub fn conformance_fixtures_json() -> String {
    core::fixtures_json().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_suite_passes_through_the_binding_types() {
        let report = run_conformance();
        assert_eq!(
            report.failed,
            0,
            "{:#?}",
            report
                .cases
                .iter()
                .filter(|c| !c.passed)
                .collect::<Vec<_>>()
        );
        assert!(report.passed > 0);
        assert_eq!(report.cases.len() as u32, report.passed);
        let fixtures: serde_json::Value =
            serde_json::from_str(&conformance_fixtures_json()).unwrap();
        assert_eq!(
            fixtures.as_array().map(Vec::len),
            Some(core::FIXTURE_FILES.len())
        );
    }
}
