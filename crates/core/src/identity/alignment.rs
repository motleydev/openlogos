//! Standoff alignment records between the tokens of two editions.
//!
//! Every alignment source (unfoldingWord `\zaln` milestones, Clear's Scripture Burrito
//! tables, a statistical aligner) normalizes to the same record: source token ids, target
//! token ids, a status, an origin, a confidence, and the tokenization hashes of both sides
//! (specs/19-search-and-retrieval.md, "Alignment"). Alignments are keyed to tokens, never to
//! Strong's numbers, so two occurrences of one lemma in a verse stay distinct. A record with
//! one empty side and status `null` says explicitly that a token has no counterpart; a token
//! in no record at all is *unaligned*, which is different (specs/03-data-model.md,
//! `span_aligns_with`: "many-to-many or explicitly null").

use serde::{Deserialize, Serialize};

use crate::anchor::TokenId;

/// What a record asserts about its two sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentStatus {
    /// The two sides correspond.
    Aligned,
    /// The sides correspond in part (an idiom, a paraphrase); both sides are non-empty.
    Partial,
    /// One side has no counterpart in the other edition; exactly one side is empty.
    Null,
    /// Reviewers disagree; the record stays visible with its origin and is not treated as
    /// settled.
    Disputed,
}

/// Where a record came from, as the first part of its `origin` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OriginKind {
    /// A named curated dataset or reviewer (`curated:<name>[@<version>]`).
    Curated,
    /// A pinned statistical aligner (`auto:<tool>@<version>`); shown with review flags.
    Auto,
    /// A user's own correction on this device (`user:<device>`); never enters packs.
    User,
}

/// One alignment record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentRecord {
    /// Tokens of the source edition.
    pub source_token_ids: Vec<TokenId>,
    /// Tokens of the target edition.
    pub target_token_ids: Vec<TokenId>,
    /// What the record asserts.
    pub status: AlignmentStatus,
    /// `curated:<name>[@<version>]`, `auto:<tool>@<version>` or `user:<device>`.
    pub origin: String,
    /// A calibrated confidence in `0..=1`, or `None` (curated records usually carry none).
    pub confidence: Option<f64>,
}

impl AlignmentRecord {
    /// The kind named by `origin`, when the string is well formed.
    #[must_use]
    pub fn origin_kind(&self) -> Option<OriginKind> {
        parse_origin(&self.origin).map(|(kind, _, _)| kind)
    }
}

/// `(kind, name, version)` of an origin string, or `None` when it is malformed.
fn parse_origin(origin: &str) -> Option<(OriginKind, &str, Option<&str>)> {
    let (kind, rest) = origin.split_once(':')?;
    let kind = match kind {
        "curated" => OriginKind::Curated,
        "auto" => OriginKind::Auto,
        "user" => OriginKind::User,
        _ => return None,
    };
    let (name, version) = match rest.split_once('@') {
        Some((name, version)) => (name, Some(version)),
        None => (rest, None),
    };
    let token_ok = |s: &str| !s.is_empty() && !s.chars().any(char::is_whitespace);
    if !token_ok(name) || version.is_some_and(|v| !token_ok(v)) {
        return None;
    }
    if kind == OriginKind::Auto && version.is_none() {
        return None;
    }
    Some((kind, name, version))
}

/// The alignment records between two editions at two content revisions.
///
/// The tokenization hashes name the exact token layers the records were made against; the
/// pipeline computes each as the lower-case hex SHA-256 of the UTF-8 lines
/// `<token id>\t<surface>\n` for every token of the edition revision in id order. The core
/// does not recompute them: it compares them with the hashes a pack declares, and a
/// mismatch on either side invalidates the whole set ([`AlignmentSet::check_tokenization`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentSet {
    /// The source edition.
    pub source_edition: String,
    /// Its content revision.
    pub source_revision: String,
    /// Hash of its token layer.
    pub source_tokenization_hash: String,
    /// The target edition.
    pub target_edition: String,
    /// Its content revision.
    pub target_revision: String,
    /// Hash of its token layer.
    pub target_tokenization_hash: String,
    /// The records, in any order.
    pub records: Vec<AlignmentRecord>,
}

/// Why an alignment set is not usable.
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AlignmentError {
    /// A record with no tokens on either side.
    #[error("record {index} has no tokens on either side")]
    Empty {
        /// Record index.
        index: usize,
    },
    /// A `null` record with tokens on both sides, or a non-null record with an empty side.
    #[error("record {index} is {status:?} but has {sources} source and {targets} target tokens")]
    WrongShape {
        /// Record index.
        index: usize,
        /// The status.
        status: AlignmentStatus,
        /// Source tokens.
        sources: usize,
        /// Target tokens.
        targets: usize,
    },
    /// A token of another edition on one side.
    #[error("record {index} names {id} on the {side} side, which is edition {expected}")]
    ForeignToken {
        /// Record index.
        index: usize,
        /// `source` or `target`.
        side: String,
        /// The id.
        id: String,
        /// The edition expected on that side.
        expected: String,
    },
    /// An origin string that is not `curated:…`, `auto:…@…` or `user:…`.
    #[error(
        "record {index} has origin `{origin}`, not curated:<name>, auto:<tool>@<version> or user:<device>"
    )]
    BadOrigin {
        /// Record index.
        index: usize,
        /// The origin as given.
        origin: String,
    },
    /// A confidence outside `0..=1`.
    #[error("record {index} has confidence {confidence}, outside 0..=1")]
    ConfidenceOutOfRange {
        /// Record index.
        index: usize,
        /// The value.
        confidence: f64,
    },
    /// A tokenization hash that is not 64 lower-case hex digits.
    #[error("{side} tokenization hash `{hash}` is not 64 lower-case hex digits")]
    BadHash {
        /// `source` or `target`.
        side: String,
        /// The hash as given.
        hash: String,
    },
    /// The token layer on one side has changed since the records were made.
    #[error(
        "{side} edition {edition} was re-tokenized: records were made against {expected}, the pack declares {found}"
    )]
    TokenizationChanged {
        /// `source` or `target`.
        side: String,
        /// The edition.
        edition: String,
        /// Hash the records carry.
        expected: String,
        /// Hash the pack declares.
        found: String,
    },
}

fn is_sha256_hex(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

impl AlignmentSet {
    /// Checks every record's shape, editions, origin and confidence, and both hashes' form.
    pub fn validate(&self) -> Result<(), AlignmentError> {
        for (side, hash) in [
            ("source", &self.source_tokenization_hash),
            ("target", &self.target_tokenization_hash),
        ] {
            if !is_sha256_hex(hash) {
                return Err(AlignmentError::BadHash {
                    side: side.to_owned(),
                    hash: hash.clone(),
                });
            }
        }
        for (index, record) in self.records.iter().enumerate() {
            let (source, target) = (record.source_token_ids.len(), record.target_token_ids.len());
            if source == 0 && target == 0 {
                return Err(AlignmentError::Empty { index });
            }
            let shape_ok = match record.status {
                AlignmentStatus::Null => source == 0 || target == 0,
                _ => source > 0 && target > 0,
            };
            if !shape_ok {
                return Err(AlignmentError::WrongShape {
                    index,
                    status: record.status,
                    sources: source,
                    targets: target,
                });
            }
            for (side, ids, expected) in [
                ("source", &record.source_token_ids, &self.source_edition),
                ("target", &record.target_token_ids, &self.target_edition),
            ] {
                if let Some(id) = ids.iter().find(|id| &id.edition != expected) {
                    return Err(AlignmentError::ForeignToken {
                        index,
                        side: side.to_owned(),
                        id: id.to_string(),
                        expected: expected.clone(),
                    });
                }
            }
            if parse_origin(&record.origin).is_none() {
                return Err(AlignmentError::BadOrigin {
                    index,
                    origin: record.origin.clone(),
                });
            }
            if let Some(confidence) = record.confidence
                && !(0.0..=1.0).contains(&confidence)
            {
                return Err(AlignmentError::ConfidenceOutOfRange { index, confidence });
            }
        }
        Ok(())
    }

    /// Checks the set against the token layers a pack declares; a mismatch on either side
    /// means the records are stale and must not be shown.
    pub fn check_tokenization(
        &self,
        source_hash: &str,
        target_hash: &str,
    ) -> Result<(), AlignmentError> {
        for (side, edition, expected, found) in [
            (
                "source",
                &self.source_edition,
                &self.source_tokenization_hash,
                source_hash,
            ),
            (
                "target",
                &self.target_edition,
                &self.target_tokenization_hash,
                target_hash,
            ),
        ] {
            if expected != found {
                return Err(AlignmentError::TokenizationChanged {
                    side: side.to_owned(),
                    edition: edition.clone(),
                    expected: expected.clone(),
                    found: found.to_owned(),
                });
            }
        }
        Ok(())
    }

    /// Records whose source side holds `id`, in table order.
    #[must_use]
    pub fn forward(&self, id: &TokenId) -> Vec<&AlignmentRecord> {
        self.records
            .iter()
            .filter(|r| r.source_token_ids.contains(id))
            .collect()
    }

    /// Records whose target side holds `id`, in table order.
    #[must_use]
    pub fn reverse(&self, id: &TokenId) -> Vec<&AlignmentRecord> {
        self.records
            .iter()
            .filter(|r| r.target_token_ids.contains(id))
            .collect()
    }

    /// The target tokens a source token renders as, across its non-null records.
    #[must_use]
    pub fn renderings(&self, source: &TokenId) -> Vec<TokenId> {
        let mut out: Vec<TokenId> = Vec::new();
        for record in self.forward(source) {
            for id in &record.target_token_ids {
                if !out.contains(id) {
                    out.push(id.clone());
                }
            }
        }
        out
    }

    /// Whether `id` (on `side`) appears in a `null` record: explicitly without counterpart.
    #[must_use]
    pub fn is_explicit_null(&self, id: &TokenId) -> bool {
        self.records.iter().any(|r| {
            r.status == AlignmentStatus::Null
                && (r.source_token_ids.contains(id) || r.target_token_ids.contains(id))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH_A: &str = "0000000000000000000000000000000000000000000000000000000000000001";
    const HASH_B: &str = "0000000000000000000000000000000000000000000000000000000000000002";

    fn id(text: &str) -> TokenId {
        TokenId::parse(text).unwrap()
    }

    fn ids(texts: &[&str]) -> Vec<TokenId> {
        texts.iter().map(|t| id(t)).collect()
    }

    fn record(
        source: &[&str],
        target: &[&str],
        status: AlignmentStatus,
        origin: &str,
        confidence: Option<f64>,
    ) -> AlignmentRecord {
        AlignmentRecord {
            source_token_ids: ids(source),
            target_token_ids: ids(target),
            status,
            origin: origin.to_owned(),
            confidence,
        }
    }

    /// John 3:16: ἠγάπησεν ↔ loved; Οὕτως γὰρ ↔ For … so (many-to-many); the article τὸν
    /// before κόσμον has no English counterpart; an auto record for κόσμον ↔ world.
    fn set() -> AlignmentSet {
        AlignmentSet {
            source_edition: "sblgnt".to_owned(),
            source_revision: "sblgnt-2010".to_owned(),
            source_tokenization_hash: HASH_A.to_owned(),
            target_edition: "bsb".to_owned(),
            target_revision: "bsb-2024".to_owned(),
            target_tokenization_hash: HASH_B.to_owned(),
            records: vec![
                record(
                    &["sblgnt:44003016003"],
                    &["bsb:44003016004"],
                    AlignmentStatus::Aligned,
                    "curated:clear-aligner@2024.1",
                    None,
                ),
                record(
                    &["sblgnt:44003016001", "sblgnt:44003016002"],
                    &["bsb:44003016001", "bsb:44003016003"],
                    AlignmentStatus::Aligned,
                    "curated:fixture",
                    None,
                ),
                record(
                    &["sblgnt:44003016006"],
                    &[],
                    AlignmentStatus::Null,
                    "curated:fixture",
                    None,
                ),
                record(
                    &["sblgnt:44003016007"],
                    &["bsb:44003016006"],
                    AlignmentStatus::Aligned,
                    "auto:eflomal@2.0.0",
                    Some(0.93),
                ),
            ],
        }
    }

    #[test]
    fn a_well_formed_set_validates_and_answers_lookups() {
        let s = set();
        s.validate().unwrap();
        s.check_tokenization(HASH_A, HASH_B).unwrap();
        assert_eq!(
            s.renderings(&id("sblgnt:44003016001")),
            ids(&["bsb:44003016001", "bsb:44003016003"])
        );
        assert_eq!(s.reverse(&id("bsb:44003016004")).len(), 1);
        assert!(s.is_explicit_null(&id("sblgnt:44003016006")));
        assert!(!s.is_explicit_null(&id("sblgnt:44003016009")));
        assert!(s.forward(&id("sblgnt:44003016009")).is_empty());
        assert!(s.renderings(&id("sblgnt:44003016006")).is_empty());
        assert_eq!(s.records[3].origin_kind(), Some(OriginKind::Auto));
        assert_eq!(s.records[0].origin_kind(), Some(OriginKind::Curated));
    }

    #[test]
    fn retokenizing_either_side_invalidates_the_set() {
        let s = set();
        assert_eq!(
            s.check_tokenization(HASH_B, HASH_B),
            Err(AlignmentError::TokenizationChanged {
                side: "source".to_owned(),
                edition: "sblgnt".to_owned(),
                expected: HASH_A.to_owned(),
                found: HASH_B.to_owned(),
            })
        );
        assert!(matches!(
            s.check_tokenization(HASH_A, HASH_A),
            Err(AlignmentError::TokenizationChanged { side, .. }) if side == "target"
        ));
    }

    #[test]
    fn malformed_records_are_rejected_with_their_index() {
        let mut s = set();
        s.records.push(record(
            &[],
            &[],
            AlignmentStatus::Aligned,
            "curated:x",
            None,
        ));
        assert_eq!(s.validate(), Err(AlignmentError::Empty { index: 4 }));

        let mut s = set();
        s.records[2].target_token_ids = ids(&["bsb:44003016005"]);
        assert!(matches!(
            s.validate(),
            Err(AlignmentError::WrongShape {
                index: 2,
                status: AlignmentStatus::Null,
                ..
            })
        ));
        let mut s = set();
        s.records[0].target_token_ids.clear();
        assert!(matches!(
            s.validate(),
            Err(AlignmentError::WrongShape { index: 0, .. })
        ));

        let mut s = set();
        s.records[0].target_token_ids = ids(&["kjv:44003016004"]);
        assert!(matches!(
            s.validate(),
            Err(AlignmentError::ForeignToken { index: 0, side, .. }) if side == "target"
        ));

        for bad in ["eflomal", "auto:eflomal", "model:x", "curated:", "user:a b"] {
            let mut s = set();
            s.records[1].origin = bad.to_owned();
            assert!(
                matches!(
                    s.validate(),
                    Err(AlignmentError::BadOrigin { index: 1, .. })
                ),
                "{bad}"
            );
        }
        let mut s = set();
        s.records[3].confidence = Some(1.5);
        assert!(matches!(
            s.validate(),
            Err(AlignmentError::ConfidenceOutOfRange { index: 3, .. })
        ));
        let mut s = set();
        s.target_tokenization_hash = "ABC".to_owned();
        assert!(matches!(
            s.validate(),
            Err(AlignmentError::BadHash { side, .. }) if side == "target"
        ));
    }

    #[test]
    fn sets_and_errors_serialize() {
        let s = set();
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"status\":\"null\""));
        assert!(json.contains("\"confidence\":0.93"));
        let back: AlignmentSet = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        let json = serde_json::to_string(&AlignmentError::Empty { index: 1 }).unwrap();
        assert_eq!(json, "{\"kind\":\"empty\",\"index\":1}");
    }
}
