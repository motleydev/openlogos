//! The columns of MACULA that may never enter a layer, an index or a test fixture.
//!
//! Both rights records carry a `restricted_module` flag (rights/S47-macula-greek.json,
//! rights/S47-macula-hebrew.json, reviewed by the source steward on 2026-11-08):
//!
//! - Greek: "Louw-Nida columns: the `ln` and `domain` attributes (TSV columns) are UBS MARBLE
//!   semantic-domain data \"used with permission\", not CC BY. They must be dropped from any
//!   redistributable pack and may not be indexed publicly".
//! - Hebrew: "Semantic-domain columns: the `sdbh`, `lexdomain`, `coredomain` and
//!   `contextualdomain` attributes are Semantic Dictionary of Biblical Hebrew data (c) United
//!   Bible Societies \"used with permission\", not CC BY. They must be dropped from any
//!   redistributable pack".
//!
//! The rule is enforced three times: the column lists here must name exactly what the record
//! flags (read from the record at build time, so a changed flag stops the build), the
//! columns a layer is built from are an allowlist that must not meet them, and the TSV reader
//! never copies a restricted cell (the row type has no field for one). The tests build a layer
//! from a snippet whose restricted cells hold markers and look for the markers in every
//! output byte.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::model::ImportError;

/// What the importer knows of one MACULA TSV.
#[derive(Debug, Clone, Copy)]
pub struct SourceColumns {
    /// The rights record id (`S47-macula-greek`).
    pub source_id: &'static str,
    /// The header, exactly; another header stops the import.
    pub header: &'static [&'static str],
    /// The columns a layer is built from.
    pub used: &'static [&'static str],
    /// The columns the rights record restricts: never read past the header.
    pub restricted: &'static [&'static str],
}

/// MACULA Greek over the SBLGNT, `SBLGNT/tsv/macula-greek-SBLGNT.tsv`.
pub const GREEK: SourceColumns = SourceColumns {
    source_id: "S47-macula-greek",
    header: &[
        "xml:id",
        "ref",
        "role",
        "class",
        "type",
        "english",
        "mandarin",
        "gloss",
        "text",
        "after",
        "lemma",
        "normalized",
        "strong",
        "morph",
        "person",
        "number",
        "gender",
        "case",
        "tense",
        "voice",
        "mood",
        "degree",
        "domain",
        "ln",
        "frame",
        "subjref",
        "referent",
    ],
    used: &[
        "xml:id", "ref", "class", "type", "english", "gloss", "text", "after", "lemma", "strong",
        "morph", "person", "number", "gender", "case", "tense", "voice", "mood", "degree",
    ],
    restricted: &["domain", "ln"],
};

/// MACULA Hebrew over the WLC, `WLC/tsv/macula-hebrew.tsv`.
pub const HEBREW: SourceColumns = SourceColumns {
    source_id: "S47-macula-hebrew",
    header: &[
        "xml:id",
        "ref",
        "class",
        "text",
        "transliteration",
        "after",
        "strongnumberx",
        "stronglemma",
        "sensenumber",
        "greek",
        "greekstrong",
        "gloss",
        "english",
        "mandarin",
        "stem",
        "morph",
        "lang",
        "lemma",
        "pos",
        "person",
        "gender",
        "number",
        "state",
        "type",
        "lexdomain",
        "contextualdomain",
        "coredomain",
        "sdbh",
        "extends",
        "frame",
        "subjref",
        "participantref",
    ],
    used: &[
        "xml:id",
        "ref",
        "text",
        "transliteration",
        "after",
        "strongnumberx",
        "gloss",
        "english",
        "stem",
        "morph",
        "lang",
        "lemma",
        "pos",
        "person",
        "gender",
        "number",
        "state",
        "type",
    ],
    restricted: &["lexdomain", "contextualdomain", "coredomain", "sdbh"],
};

/// The column names a record's `restricted_module` flags name in backticks.
#[must_use]
pub fn flagged_columns(record: &Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for flag in record
        .get("flags")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|f| f.get("kind").and_then(Value::as_str) == Some("restricted_module"))
    {
        let text = flag.get("text").and_then(Value::as_str).unwrap_or("");
        for (i, part) in text.split('`').enumerate() {
            if i % 2 == 1 && !part.is_empty() {
                names.insert(part.to_owned());
            }
        }
    }
    names
}

/// Checks a source's column lists against its rights record: the restricted list is exactly
/// what the record flags, every restricted column is in the header (so the reader knows the
/// cell it skips), and the allowlist meets none of them.
pub fn check(columns: &SourceColumns, record: &Value) -> Result<(), ImportError> {
    let refuse = |detail: String| ImportError::Rights {
        source_id: columns.source_id.to_owned(),
        detail,
    };
    let flagged = flagged_columns(record);
    let ours: BTreeSet<String> = columns.restricted.iter().map(|c| (*c).to_owned()).collect();
    if flagged != ours {
        return Err(refuse(format!(
            "the record restricts {flagged:?}, the importer excludes {ours:?}"
        )));
    }
    for c in columns.restricted {
        if !columns.header.contains(c) {
            return Err(refuse(format!(
                "restricted column `{c}` is not in the header"
            )));
        }
        if columns.used.contains(c) {
            return Err(refuse(format!(
                "restricted column `{c}` is in the allowlist"
            )));
        }
    }
    for c in columns.used {
        if !columns.header.contains(c) {
            return Err(refuse(format!("column `{c}` is not in the header")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str) -> Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../rights")
            .join(format!("{id}.json"));
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn the_excluded_columns_are_exactly_what_the_rights_records_flag() {
        for columns in [GREEK, HEBREW] {
            let record = record(columns.source_id);
            check(&columns, &record).unwrap();
            let flagged: Vec<String> = flagged_columns(&record).into_iter().collect();
            let mut ours: Vec<&str> = columns.restricted.to_vec();
            ours.sort_unstable();
            assert_eq!(flagged, ours, "{}", columns.source_id);
        }
    }

    #[test]
    fn a_flag_naming_another_column_or_an_allowlisted_one_stops_the_build() {
        let mut changed = record("S47-macula-greek");
        changed["flags"][0]["text"] =
            "The `ln`, `domain` and `gloss` columns are restricted.".into();
        assert!(check(&GREEK, &changed).is_err());
        let mut leaky = GREEK;
        leaky.used = &["xml:id", "ln"];
        assert!(check(&leaky, &record("S47-macula-greek")).is_err());
    }
}
