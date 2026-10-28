//! The B006 fixture kinds: `id_lineage` tables, standoff alignment sets, and anchors carried
//! across a content revision. Fixtures write token ids as `edition:BBCCCVVVWWW[s]` strings
//! and references in OSIS form; the runners parse them into the core types, so a fixture
//! also checks that the ids and references parse.

use openlogos_core::anchor::{
    Anchor, ReanchorResult, Revision, ScalarRange, Segment, TokenId, TokenSpan, reanchor_across,
    split_selection,
};
use openlogos_core::identity::{
    AlignmentRecord, AlignmentSet, AlignmentStatus, IdLineage, LineageEntry, LineageKind,
};
use openlogos_core::versification::MappingStatus;
use openlogos_text::{grapheme_len, scalar_len, scalar_to_utf16, slice_scalars, utf16_len};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{FixtureToken, Recorder, Span};

/// One lineage row with string ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureLineageEntry {
    /// The relation.
    pub kind: LineageKind,
    /// Old ids.
    pub from: Vec<String>,
    /// New ids.
    pub to: Vec<String>,
}

/// A lineage table with string ids, as a fixture writes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureLineage {
    /// The older revision.
    pub from_revision: String,
    /// The newer revision.
    pub to_revision: String,
    /// The rows.
    pub entries: Vec<FixtureLineageEntry>,
}

/// One expected set mapping through a lineage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageCase {
    /// What the case shows.
    pub name: String,
    /// Old ids to map.
    pub tokens: Vec<String>,
    /// Expected status.
    pub status: MappingStatus,
    /// Expected successors.
    pub successors: Vec<String>,
    /// Expected deleted ids.
    pub deleted: Vec<String>,
    /// Expected unknown ids.
    pub unknown: Vec<String>,
    /// Expected `identity_kept`.
    pub identity_kept: bool,
}

/// A table that must be rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidLineageCase {
    /// What the case shows.
    pub name: String,
    /// The rows.
    pub entries: Vec<FixtureLineageEntry>,
    /// Expected error kind, as the core serializes it.
    pub error: String,
}

/// An `id_lineage` fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageFixture {
    /// Fixture id.
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License.
    pub license: String,
    /// Source.
    pub source: String,
    /// The edition.
    pub edition_id: String,
    /// The older revision.
    pub from_revision: String,
    /// The newer revision.
    pub to_revision: String,
    /// The rows.
    pub entries: Vec<FixtureLineageEntry>,
    /// Set mappings.
    pub cases: Vec<LineageCase>,
    /// Tables that must be rejected.
    #[serde(default)]
    pub invalid: Vec<InvalidLineageCase>,
}

/// One alignment record with string ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixtureAlignmentRecord {
    /// Source ids.
    pub source: Vec<String>,
    /// Target ids.
    pub target: Vec<String>,
    /// Status.
    pub status: AlignmentStatus,
    /// Origin string.
    pub origin: String,
    /// Confidence.
    pub confidence: Option<f64>,
}

/// One side of an alignment fixture: a segment with its token layer and tokenization hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignedSegment {
    /// The edition.
    pub edition_id: String,
    /// Its content revision.
    pub content_revision: String,
    /// The segment.
    pub segment_id: String,
    /// The text, byte-exact.
    pub text: String,
    /// Expected length in scalar values.
    pub scalar_len: u32,
    /// Expected length in UTF-16 units.
    pub utf16_len: u32,
    /// Expected length in grapheme clusters.
    pub grapheme_len: u32,
    /// Hash of the token layer (`<id>\t<surface>\n` lines in id order, SHA-256 hex).
    pub tokenization_hash: String,
    /// Tokens in text order, each with its surface.
    pub tokens: Vec<FixtureToken>,
}

/// Expected renderings of one source token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderingCase {
    /// What the case shows.
    pub name: String,
    /// The source id.
    pub source: String,
    /// Expected target ids.
    pub targets: Vec<String>,
}

/// Expected sources of one target token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReverseCase {
    /// What the case shows.
    pub name: String,
    /// The target id.
    pub target: String,
    /// Expected source ids, across records.
    pub sources: Vec<String>,
}

/// Hashes that must invalidate the set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaleCase {
    /// Hash the pack declares for the source side.
    pub source_tokenization_hash: String,
    /// Hash the pack declares for the target side.
    pub target_tokenization_hash: String,
    /// Expected error kind.
    pub error: String,
    /// Expected side named in the error.
    pub side: String,
}

/// A record that must be rejected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvalidAlignmentCase {
    /// What the case shows.
    pub name: String,
    /// The record.
    pub record: FixtureAlignmentRecord,
    /// Expected error kind.
    pub error: String,
}

/// A standoff alignment fixture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentFixture {
    /// Fixture id.
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License.
    pub license: String,
    /// Source.
    pub source: String,
    /// The source side.
    pub source_segment: AlignedSegment,
    /// The target side.
    pub target_segment: AlignedSegment,
    /// The records.
    pub records: Vec<FixtureAlignmentRecord>,
    /// Forward lookups.
    pub renderings: Vec<RenderingCase>,
    /// Reverse lookups.
    pub reverse: Vec<ReverseCase>,
    /// Ids that must be explicitly null.
    pub explicit_null: Vec<String>,
    /// Ids that must be in no record.
    pub unaligned: Vec<String>,
    /// Number of records of `auto:` origin.
    pub auto_records: u32,
    /// The stale-hash case.
    pub stale: StaleCase,
    /// Records that must be rejected.
    #[serde(default)]
    pub invalid: Vec<InvalidAlignmentCase>,
}

/// One revision of a segment in a reanchor fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureRevision {
    /// The content revision.
    pub content_revision: String,
    /// The text, byte-exact.
    pub text: String,
    /// Expected length in scalar values.
    pub scalar_len: u32,
    /// Expected length in UTF-16 units.
    pub utf16_len: u32,
    /// Expected length in grapheme clusters.
    pub grapheme_len: u32,
    /// Tokens in text order; empty without a token layer.
    pub tokens: Vec<FixtureToken>,
}

/// An old and a new revision of one segment, with the lineage between them if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionPair {
    /// Pair id, referenced by cases.
    pub id: String,
    /// The edition.
    pub edition_id: String,
    /// The segment.
    pub segment_id: String,
    /// The older revision.
    pub old: FixtureRevision,
    /// The newer revision.
    pub new: FixtureRevision,
    /// The lineage table, when the edition ships one.
    #[serde(default)]
    pub lineage: Option<FixtureLineage>,
}

/// What a reanchoring must produce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedReanchor {
    /// `exact`, `moved`, `ambiguous` or `orphaned`.
    pub status: String,
    /// The new range in scalar values (exact, moved).
    #[serde(default)]
    pub scalar: Option<Span>,
    /// The new range in UTF-16 units (exact, moved).
    #[serde(default)]
    pub utf16: Option<Span>,
    /// The quoted text at the new range (exact, moved).
    #[serde(default)]
    pub exact: Option<String>,
    /// Token ids under the new range (exact, moved).
    #[serde(default)]
    pub tokens: Option<Vec<String>>,
    /// Candidate ranges (ambiguous).
    #[serde(default)]
    pub candidates: Option<Vec<Span>>,
}

/// One anchor to carry across.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReanchorCase {
    /// What the case shows.
    pub name: String,
    /// The pair.
    pub pair: String,
    /// The selection in the old revision, in scalar values.
    pub scalar: Span,
    /// Whether the anchor keeps its token ids.
    pub with_tokens: bool,
    /// Whether the anchor keeps its quote selector.
    pub with_quote: bool,
    /// Whether the pair's lineage is passed.
    pub use_lineage: bool,
    /// The expected result.
    pub expected: ExpectedReanchor,
}

/// A reanchoring fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReanchorFixture {
    /// Fixture id.
    pub id: String,
    /// What the fixture shows.
    pub description: String,
    /// License.
    pub license: String,
    /// Source.
    pub source: String,
    /// The revision pairs.
    pub pairs: Vec<RevisionPair>,
    /// The cases.
    pub cases: Vec<ReanchorCase>,
}

fn parse_ids(ids: &[String]) -> Result<Vec<TokenId>, String> {
    ids.iter()
        .map(|id| TokenId::parse(id).map_err(|e| e.to_string()))
        .collect()
}

fn id_strings(ids: &[TokenId]) -> Vec<String> {
    ids.iter().map(ToString::to_string).collect()
}

fn lineage_entries(entries: &[FixtureLineageEntry]) -> Result<Vec<LineageEntry>, String> {
    entries
        .iter()
        .map(|e| {
            Ok(LineageEntry {
                kind: e.kind,
                from: parse_ids(&e.from)?,
                to: parse_ids(&e.to)?,
            })
        })
        .collect()
}

/// The `kind` tag of a serialized error, or its message.
fn error_kind<E: Serialize + std::fmt::Display>(error: &E) -> String {
    serde_json::to_value(error)
        .ok()
        .and_then(|v| v.get("kind").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| error.to_string())
}

/// Status, successors, deleted, unknown and identity flag of a lineage mapping.
#[derive(Serialize, PartialEq)]
struct LineageShape {
    status: MappingStatus,
    successors: Vec<String>,
    deleted: Vec<String>,
    unknown: Vec<String>,
    identity_kept: bool,
}

pub(crate) fn run_lineage(fixture: &LineageFixture, r: &mut Recorder) {
    let entries = match lineage_entries(&fixture.entries) {
        Ok(entries) => entries,
        Err(e) => {
            r.fail("rows parse", "token ids in edition:BBCCCVVVWWW[s] form", &e);
            return;
        }
    };
    let lineage = IdLineage {
        edition_id: fixture.edition_id.clone(),
        from_revision: fixture.from_revision.clone(),
        to_revision: fixture.to_revision.clone(),
        entries,
    };
    r.check(
        "the table validates",
        &Ok(()),
        &lineage.validate().map_err(|e| e.to_string()),
    );
    for case in &fixture.cases {
        let tokens = match parse_ids(&case.tokens) {
            Ok(tokens) => tokens,
            Err(e) => {
                r.fail(&format!("{}: ids parse", case.name), "token ids", &e);
                continue;
            }
        };
        let result = lineage.map_tokens(&tokens);
        r.check(
            &case.name,
            &LineageShape {
                status: case.status,
                successors: case.successors.clone(),
                deleted: case.deleted.clone(),
                unknown: case.unknown.clone(),
                identity_kept: case.identity_kept,
            },
            &LineageShape {
                status: result.status,
                successors: id_strings(&result.tokens),
                deleted: id_strings(&result.deleted),
                unknown: id_strings(&result.unknown),
                identity_kept: result.identity_kept,
            },
        );
    }
    for case in &fixture.invalid {
        let actual = match lineage_entries(&case.entries) {
            Ok(entries) => {
                let table = IdLineage {
                    entries,
                    ..lineage.clone()
                };
                match table.validate() {
                    Ok(()) => "accepted".to_owned(),
                    Err(e) => error_kind(&e),
                }
            }
            Err(e) => e,
        };
        r.check(&format!("rejects {}", case.name), &case.error, &actual);
    }
    // A table from a revision to itself is rejected whatever its rows.
    let same = IdLineage {
        to_revision: lineage.from_revision.clone(),
        ..lineage.clone()
    };
    r.check(
        "rejects a table from a revision to itself",
        &"same_revision".to_owned(),
        &same
            .validate()
            .map_or_else(|e| error_kind(&e), |()| "accepted".to_owned()),
    );
}

fn check_segment(segment: &AlignedSegment, r: &mut Recorder) {
    #[derive(Serialize, PartialEq)]
    struct Lengths {
        scalar: u32,
        utf16: u32,
        grapheme: u32,
    }
    r.check(
        &format!("{}/{}: lengths", segment.edition_id, segment.segment_id),
        &Lengths {
            scalar: segment.scalar_len,
            utf16: segment.utf16_len,
            grapheme: segment.grapheme_len,
        },
        &Lengths {
            scalar: scalar_len(&segment.text),
            utf16: utf16_len(&segment.text),
            grapheme: grapheme_len(&segment.text),
        },
    );
    for token in &segment.tokens {
        let range = ScalarRange {
            start: token.start,
            end: token.end,
        };
        r.check(
            &format!("{}: token {} surface", segment.edition_id, token.id),
            &Ok(token.surface.clone().unwrap_or_default()),
            &slice_scalars(&segment.text, range),
        );
        r.check(
            &format!(
                "{}: token {} parses and is in the edition",
                segment.edition_id, token.id
            ),
            &Some(segment.edition_id.clone()),
            &TokenId::parse(&token.id).ok().map(|id| id.edition),
        );
    }
}

fn alignment_record(record: &FixtureAlignmentRecord) -> Result<AlignmentRecord, String> {
    Ok(AlignmentRecord {
        source_token_ids: parse_ids(&record.source)?,
        target_token_ids: parse_ids(&record.target)?,
        status: record.status,
        origin: record.origin.clone(),
        confidence: record.confidence,
    })
}

pub(crate) fn run_alignment(fixture: &AlignmentFixture, r: &mut Recorder) {
    check_segment(&fixture.source_segment, r);
    check_segment(&fixture.target_segment, r);
    let records: Result<Vec<AlignmentRecord>, String> =
        fixture.records.iter().map(alignment_record).collect();
    let records = match records {
        Ok(records) => records,
        Err(e) => {
            r.fail("records parse", "token ids", &e);
            return;
        }
    };
    let set = AlignmentSet {
        source_edition: fixture.source_segment.edition_id.clone(),
        source_revision: fixture.source_segment.content_revision.clone(),
        source_tokenization_hash: fixture.source_segment.tokenization_hash.clone(),
        target_edition: fixture.target_segment.edition_id.clone(),
        target_revision: fixture.target_segment.content_revision.clone(),
        target_tokenization_hash: fixture.target_segment.tokenization_hash.clone(),
        records,
    };
    r.check(
        "the set validates",
        &Ok(()),
        &set.validate().map_err(|e| e.to_string()),
    );
    r.check(
        "the set matches the token layers it was made against",
        &Ok(()),
        &set.check_tokenization(
            &fixture.source_segment.tokenization_hash,
            &fixture.target_segment.tokenization_hash,
        )
        .map_err(|e| e.to_string()),
    );
    // Every record's ids are tokens of the two segments.
    let known = |ids: &[TokenId], segment: &AlignedSegment| {
        ids.iter()
            .all(|id| segment.tokens.iter().any(|t| t.id == id.to_string()))
    };
    r.check(
        "every record names tokens of the two segments",
        &true,
        &set.records.iter().all(|rec| {
            known(&rec.source_token_ids, &fixture.source_segment)
                && known(&rec.target_token_ids, &fixture.target_segment)
        }),
    );
    for case in &fixture.renderings {
        let actual = TokenId::parse(&case.source).map(|id| id_strings(&set.renderings(&id)));
        r.check(
            &case.name,
            &Ok(case.targets.clone()),
            &actual.map_err(|e| e.to_string()),
        );
    }
    for case in &fixture.reverse {
        let actual = TokenId::parse(&case.target).map(|id| {
            let mut sources: Vec<String> = Vec::new();
            for record in set.reverse(&id) {
                for source in &record.source_token_ids {
                    let text = source.to_string();
                    if !sources.contains(&text) {
                        sources.push(text);
                    }
                }
            }
            sources
        });
        r.check(
            &case.name,
            &Ok(case.sources.clone()),
            &actual.map_err(|e| e.to_string()),
        );
    }
    for id in &fixture.explicit_null {
        r.check(
            &format!("{id} is explicitly null"),
            &Ok(true),
            &TokenId::parse(id)
                .map(|id| set.is_explicit_null(&id))
                .map_err(|e| e.to_string()),
        );
    }
    for id in &fixture.unaligned {
        r.check(
            &format!("{id} is in no record"),
            &Ok(true),
            &TokenId::parse(id)
                .map(|id| {
                    set.forward(&id).is_empty()
                        && set.reverse(&id).is_empty()
                        && !set.is_explicit_null(&id)
                })
                .map_err(|e| e.to_string()),
        );
    }
    r.check(
        "auto-origin records are counted",
        &fixture.auto_records,
        &(set
            .records
            .iter()
            .filter(|rec| rec.origin.starts_with("auto:"))
            .count() as u32),
    );
    let stale = set.check_tokenization(
        &fixture.stale.source_tokenization_hash,
        &fixture.stale.target_tokenization_hash,
    );
    let (kind, side) = match &stale {
        Ok(()) => ("accepted".to_owned(), String::new()),
        Err(e) => (
            error_kind(e),
            serde_json::to_value(e)
                .ok()
                .and_then(|v| v.get("side").and_then(Value::as_str).map(str::to_owned))
                .unwrap_or_default(),
        ),
    };
    r.check(
        "a re-tokenized side invalidates the set",
        &(fixture.stale.error.clone(), fixture.stale.side.clone()),
        &(kind, side),
    );
    for case in &fixture.invalid {
        let actual = match alignment_record(&case.record) {
            Ok(record) => {
                let mut bad = set.clone();
                bad.records.push(record);
                match bad.validate() {
                    Ok(()) => "accepted".to_owned(),
                    Err(e) => error_kind(&e),
                }
            }
            Err(e) => e,
        };
        r.check(&format!("rejects {}", case.name), &case.error, &actual);
    }
}

fn to_revision(revision: &FixtureRevision) -> Result<Revision, String> {
    let tokens = revision
        .tokens
        .iter()
        .map(|t| {
            TokenId::parse(&t.id).map(|id| TokenSpan {
                id,
                range: ScalarRange {
                    start: t.start,
                    end: t.end,
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(Revision {
        content_revision: revision.content_revision.clone(),
        text: revision.text.clone(),
        tokens,
    })
}

fn check_revision(name: &str, revision: &FixtureRevision, r: &mut Recorder) {
    #[derive(Serialize, PartialEq)]
    struct Lengths {
        scalar: u32,
        utf16: u32,
        grapheme: u32,
    }
    r.check(
        &format!("{name}: lengths"),
        &Lengths {
            scalar: revision.scalar_len,
            utf16: revision.utf16_len,
            grapheme: revision.grapheme_len,
        },
        &Lengths {
            scalar: scalar_len(&revision.text),
            utf16: utf16_len(&revision.text),
            grapheme: grapheme_len(&revision.text),
        },
    );
}

/// The fields of a reanchoring result a fixture pins down.
#[derive(Serialize, PartialEq)]
struct ReanchorShape {
    status: String,
    scalar: Option<Span>,
    utf16: Option<Span>,
    exact: Option<String>,
    tokens: Option<Vec<String>>,
    candidates: Option<Vec<Span>>,
    content_revision: Option<String>,
}

fn shape_of(result: &ReanchorResult, new: &Revision) -> ReanchorShape {
    let span = |a: &Anchor| Span {
        start: a.range.start,
        end: a.range.end,
    };
    let found = |status: &str, a: &Anchor| ReanchorShape {
        status: status.to_owned(),
        scalar: Some(span(a)),
        utf16: scalar_to_utf16(&new.text, a.range).ok().map(|u| Span {
            start: u.start,
            end: u.end,
        }),
        exact: a.quote.as_ref().map(|q| q.exact.clone()),
        tokens: Some(id_strings(&a.tokens)),
        candidates: None,
        content_revision: Some(a.content_revision.clone()),
    };
    match result {
        ReanchorResult::Exact(a) => found("exact", a),
        ReanchorResult::Moved(a) => found("moved", a),
        ReanchorResult::Ambiguous(candidates) => ReanchorShape {
            status: "ambiguous".to_owned(),
            scalar: None,
            utf16: None,
            exact: None,
            tokens: None,
            candidates: Some(candidates.iter().map(span).collect()),
            content_revision: candidates.first().map(|a| a.content_revision.clone()),
        },
        ReanchorResult::Orphaned => ReanchorShape {
            status: "orphaned".to_owned(),
            scalar: None,
            utf16: None,
            exact: None,
            tokens: None,
            candidates: None,
            content_revision: None,
        },
    }
}

pub(crate) fn run_reanchor(fixture: &ReanchorFixture, r: &mut Recorder) {
    for pair in &fixture.pairs {
        check_revision(&format!("{}/old", pair.id), &pair.old, r);
        check_revision(&format!("{}/new", pair.id), &pair.new, r);
    }
    for case in &fixture.cases {
        let name = |what: &str| format!("{}: {what}", case.name);
        let Some(pair) = fixture.pairs.iter().find(|p| p.id == case.pair) else {
            r.fail(&name("pair exists"), &case.pair, "no such pair");
            continue;
        };
        let (old, new) = match (to_revision(&pair.old), to_revision(&pair.new)) {
            (Ok(old), Ok(new)) => (old, new),
            (Err(e), _) | (_, Err(e)) => {
                r.fail(&name("revisions parse"), "token ids", &e);
                continue;
            }
        };
        let lineage = match (&pair.lineage, case.use_lineage) {
            (Some(table), true) => match lineage_entries(&table.entries) {
                Ok(entries) => Some(IdLineage {
                    edition_id: pair.edition_id.clone(),
                    from_revision: table.from_revision.clone(),
                    to_revision: table.to_revision.clone(),
                    entries,
                }),
                Err(e) => {
                    r.fail(&name("lineage parses"), "token ids", &e);
                    continue;
                }
            },
            _ => None,
        };
        if let Some(table) = &lineage {
            r.check(
                &name("lineage validates"),
                &Ok(()),
                &table.validate().map_err(|e| e.to_string()),
            );
        }
        let segment = Segment {
            segment_id: pair.segment_id.clone(),
            text: old.text.clone(),
            tokens: old.tokens.clone(),
        };
        let anchors = split_selection(
            &pair.edition_id,
            &old.content_revision,
            std::slice::from_ref(&segment),
            "",
            case.scalar.into(),
        );
        let mut anchor = match anchors.as_deref() {
            Ok([anchor]) => anchor.clone(),
            other => {
                r.fail(
                    &name("anchor from the selection"),
                    "exactly one anchor",
                    &format!("{other:?}"),
                );
                continue;
            }
        };
        if !case.with_tokens {
            anchor.tokens.clear();
        }
        if !case.with_quote {
            anchor.quote = None;
        }
        let result = reanchor_across(&anchor, &old, &new, lineage.as_ref());
        let expected = ReanchorShape {
            status: case.expected.status.clone(),
            scalar: case.expected.scalar,
            utf16: case.expected.utf16,
            exact: case.expected.exact.clone(),
            tokens: case.expected.tokens.clone(),
            candidates: case.expected.candidates.clone(),
            content_revision: match case.expected.status.as_str() {
                "orphaned" => None,
                _ => Some(new.content_revision.clone()),
            },
        };
        r.check(&case.name, &expected, &shape_of(&result, &new));
    }
}
