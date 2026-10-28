//! Optional semantic retrieval and the storage boundary of
//! docs/adr/0002-embedded-retrieval-stack.md. The five traits declared here (TextStore,
//! TextIndex, TokenEngine, RelationStore and VectorIndex) are the seams between the core's
//! query logic and the embedded engine: SQLite as canonical store, FTS5 for keyword search,
//! the bitmap token engine, a typed edge table for relations and sqlite-vec brute force over
//! quantized vectors. Hybrid retrieval fuses BM25 and vector k-nearest-neighbour results by
//! reciprocal rank fusion, and every result is labeled with the model and version that
//! produced it so similarity is never presented as a documented relation
//! (specs/19-search-and-retrieval.md).
//!
//! Everything in this crate is engine-neutral: no SQL, no file formats, no handle types.
//! `openlogos-retrieval-sqlite` (B064) is the first implementation; a later engine implements
//! the same traits and the conformance suite runs against both.

use serde::{Deserialize, Serialize};

/// Errors every engine reports through the trait boundary.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum RetrievalError {
    /// The query names a feature layer or unit that the installed packs do not carry. Never
    /// approximated (specs/19, "layer unavailable").
    #[error("layer unavailable: {0}")]
    LayerUnavailable(String),
    /// No installed pack serves this edition.
    #[error("unknown edition `{0}`")]
    UnknownEdition(String),
    /// The query is malformed (bad slot index, empty pattern, unsupported combination).
    #[error("invalid query: {0}")]
    InvalidQuery(String),
    /// The engine's storage layer failed; the message is engine-specific.
    #[error("storage: {0}")]
    Storage(String),
}

/// Result alias for the retrieval boundary.
pub type Result<T> = core::result::Result<T, RetrievalError>;

/// A verse in the `BBCCCVVV` numeric form that token ids use (`edition:BBCCCVVVWWW[s]`):
/// USFM book number × 1 000 000 + chapter × 1000 + verse. Sorts canonically as an integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VerseId(pub u32);

impl VerseId {
    /// Builds a verse id from its parts; chapter and verse must each be below 1000.
    #[must_use]
    pub fn new(book: u8, chapter: u16, verse: u16) -> VerseId {
        VerseId(u32::from(book) * 1_000_000 + u32::from(chapter % 1000) * 1000 + u32::from(verse % 1000))
    }

    /// USFM book number (Genesis 1, Matthew 41).
    #[must_use]
    pub fn book(self) -> u8 {
        (self.0 / 1_000_000) as u8
    }

    /// Chapter number.
    #[must_use]
    pub fn chapter(self) -> u16 {
        ((self.0 / 1000) % 1000) as u16
    }

    /// Verse number.
    #[must_use]
    pub fn verse(self) -> u16 {
        (self.0 % 1000) as u16
    }

    /// The `BBCCC` chapter key (book × 1000 + chapter).
    #[must_use]
    pub fn chapter_key(self) -> u32 {
        self.0 / 1000
    }
}

/// Global token position inside one edition: 0-based, dense, in reading order. Text-Fabric
/// calls the same thing a slot (specs/19 names that model as the reference).
pub type TokenPos = u32;

/// What one installed edition carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditionInfo {
    /// Edition id, the prefix of its token ids (`sblgnt`, `wlc`, `bsb`).
    pub edition: String,
    /// Versification scheme the text is stored in (`org`, `eng`, `lxx`).
    pub scheme: String,
    /// Verses stored.
    pub verses: u64,
    /// Tokens stored; 0 when the pack has no token layer.
    pub tokens: u64,
    /// Feature columns the token layer carries (`lemma`, `case`, ...), sorted.
    pub features: Vec<String>,
    /// Container kinds available for unit scoping beyond verse/chapter/book.
    pub containers: Vec<String>,
}

/// One verse mapped into another scheme (specs/19, `verse_map`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerseMapping {
    /// Target scheme.
    pub scheme: String,
    /// Target verse.
    pub verse: VerseId,
    /// Kind of correspondence, as `openlogos_core::versification::VerseMapKind` spells it.
    pub kind: String,
}

/// Canonical storage for passages and pack metadata.
///
/// ADR 0002 chooses SQLite: rusqlite on native targets, sqlite-wasm with the `opfs-sahpool`
/// VFS (SQLite 3.54 or later) on the web. Corpus packs are read-only and attached on demand;
/// user data lives in a separate mutable database.
pub trait TextStore {
    /// Every edition the open packs serve.
    fn editions(&self) -> Result<Vec<EditionInfo>>;
    /// The byte-exact display text of one verse, or `None` when the edition lacks it.
    fn verse_text(&self, edition: &str, verse: VerseId) -> Result<Option<String>>;
    /// All verses of one chapter in order.
    fn chapter_text(&self, edition: &str, book: u8, chapter: u16) -> Result<Vec<(VerseId, String)>>;
    /// Where a verse of this edition lands in another scheme, from the pack's `verse_map`.
    /// An empty result means unmapped, never identity.
    fn map_verse(&self, edition: &str, verse: VerseId, to_scheme: &str) -> Result<Vec<VerseMapping>>;
}

/// A keyword, phrase or Boolean query (specs/17 syntax, engine-specific parsing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextQuery {
    /// The query text; phrases in double quotes, `AND`, `OR`, `NOT` as operators.
    pub text: String,
    /// Editions to search; empty means every open edition.
    pub editions: Vec<String>,
    /// Restrict to one USFM book number.
    pub book: Option<u8>,
    /// Maximum hits.
    pub limit: usize,
}

/// One full-text hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextHit {
    /// Edition of the hit.
    pub edition: String,
    /// Verse of the hit.
    pub verse: VerseId,
    /// BM25 score (lower is better in FTS5's convention; engines must document theirs).
    pub score: f64,
}

/// Keyword, phrase and Boolean full-text search over stored text.
///
/// ADR 0002 chooses FTS5 with one shared Rust Greek/Hebrew normalizing tokenizer so the index
/// and the query parser normalize identically.
pub trait TextIndex {
    /// Runs a text query across the named editions.
    fn search_text(&self, query: &TextQuery) -> Result<Vec<TextHit>>;
    /// The normalization the index applies, as a versioned mode name (`ol-norm-1`).
    fn normalization_mode(&self) -> &'static str;
    /// Normalizes a string the way the index does; the search key of display text.
    fn normalize(&self, text: &str) -> String;
}

/// One `feature=value` test on a token slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeaturePredicate {
    /// Feature column name as the pack declares it (`lemma`, `case`, `mood`, ...).
    pub feature: String,
    /// Exact value after the pack's own normalization of that feature.
    pub value: String,
}

/// A token slot: the conjunction of its predicates. An empty slot matches every token.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TokenPattern {
    /// Predicates that must all hold.
    pub predicates: Vec<FeaturePredicate>,
}

impl TokenPattern {
    /// A slot with one `feature=value` predicate.
    #[must_use]
    pub fn one(feature: &str, value: &str) -> TokenPattern {
        TokenPattern {
            predicates: vec![FeaturePredicate {
                feature: feature.to_owned(),
                value: value.to_owned(),
            }],
        }
    }

    /// Adds another predicate.
    #[must_use]
    pub fn and(mut self, feature: &str, value: &str) -> TokenPattern {
        self.predicates.push(FeaturePredicate {
            feature: feature.to_owned(),
            value: value.to_owned(),
        });
        self
    }
}

/// Text units a query can be scoped to or test structurally. `Verse`, `Chapter` and `Book`
/// come from the verse id; the rest are container kinds a pack may carry (`clause`,
/// `sentence`, `phrase`, or any other name the pack declares).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Unit {
    /// One verse.
    Verse,
    /// One chapter.
    Chapter,
    /// One book.
    Book,
    /// A pack-declared container kind.
    Container(String),
}

/// A relation between two slots of a [`TokenQuery`]. Slot indexes refer to
/// `TokenQuery::slots`. Distances count token positions, so `distance: 1` means adjacent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Constraint {
    /// `|pos(a) − pos(b)| ≤ distance`, in either order (`WITHIN n`).
    Within {
        /// First slot.
        a: usize,
        /// Second slot.
        b: usize,
        /// Maximum distance in tokens.
        distance: u32,
    },
    /// `a` precedes `b`, with at most `distance` tokens between them when given (`BEFORE n`).
    Before {
        /// Earlier slot.
        a: usize,
        /// Later slot.
        b: usize,
        /// Maximum gap in tokens; `None` is anywhere later inside the scope.
        distance: Option<u32>,
    },
    /// The named features have equal, non-empty values on both slots (`AGREE(case,number)`).
    Agree {
        /// First slot.
        a: usize,
        /// Second slot.
        b: usize,
        /// Feature names that must agree.
        features: Vec<String>,
    },
    /// Both slots sit in the same innermost container of this kind (`PEER`).
    Peer {
        /// First slot.
        a: usize,
        /// Second slot.
        b: usize,
        /// Container kind.
        unit: Unit,
    },
    /// The slots sit in different innermost containers of this kind (`XPEER`).
    XPeer {
        /// First slot.
        a: usize,
        /// Second slot.
        b: usize,
        /// Container kind.
        unit: Unit,
    },
    /// Some container of this kind holds both slots, at any nesting depth (`ANCESTOR`).
    Ancestor {
        /// First slot.
        a: usize,
        /// Second slot.
        b: usize,
        /// Container kind.
        unit: Unit,
    },
}

/// A linguistic query: one or more token slots, all inside one unit of `scope`, related by
/// constraints. Every match binds each slot to a distinct token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenQuery {
    /// Edition to search.
    pub edition: String,
    /// The slots; at least one.
    pub slots: Vec<TokenPattern>,
    /// All slots of a match sit in the same unit of this kind.
    pub scope: Unit,
    /// Relations between slots.
    pub constraints: Vec<Constraint>,
    /// Restrict to one USFM book number.
    pub book: Option<u8>,
    /// Maximum matches to materialize; counting is exact regardless.
    pub limit: usize,
}

impl TokenQuery {
    /// A single-slot query over the whole edition, scoped to the verse.
    #[must_use]
    pub fn single(edition: &str, slot: TokenPattern) -> TokenQuery {
        TokenQuery {
            edition: edition.to_owned(),
            slots: vec![slot],
            scope: Unit::Verse,
            constraints: Vec::new(),
            book: None,
            limit: usize::MAX,
        }
    }
}

/// One match: the token bound to each slot, in slot order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenMatch {
    /// Positions, `slots.len()` long.
    pub positions: Vec<TokenPos>,
}

/// The result of a [`TokenQuery`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenHits {
    /// Exact number of matches, even beyond `limit`.
    pub count: u64,
    /// Up to `limit` matches in position order of their first slot.
    pub matches: Vec<TokenMatch>,
}

/// Stored columns of one token, for display and facets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenInfo {
    /// Position.
    pub pos: TokenPos,
    /// Verse.
    pub verse: VerseId,
    /// Byte-exact surface form.
    pub surface: String,
    /// `(feature, value)` pairs in the pack's feature order; absent values are omitted.
    pub features: Vec<(String, String)>,
}

/// A facet row: hits grouped by one feature's value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Facet {
    /// The value (`book` facets use the USFM number as text).
    pub value: String,
    /// Matches carrying it.
    pub count: u64,
}

/// The linguistic query engine: lemma, morphology, proximity, agreement and structure.
///
/// ADR 0002 chooses a custom columnar token engine on roaring bitmaps after Text-Fabric's
/// slot/feature/edge model (specs/19-search-and-retrieval.md); its correctness oracle is
/// Text-Fabric on N1904 (B064, benchmark R6).
pub trait TokenEngine {
    /// Evaluates a token query.
    fn token_search(&self, query: &TokenQuery) -> Result<TokenHits>;
    /// The stored columns of one token.
    fn token_info(&self, edition: &str, pos: TokenPos) -> Result<TokenInfo>;
    /// Groups positions by a feature (`lemma`, `surface`, `book`, ...) and counts.
    fn facets(&self, edition: &str, positions: &[TokenPos], feature: &str) -> Result<Vec<Facet>>;
}

/// Where an edge starts or ends: a work plus a locator inside it. For Bible editions the
/// locator is the verse id as decimal text; for other works it is the canonical citation
/// (specs/19, Cross-resource links).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Locator {
    /// Work id (`bsb`, `calvin-institutes`).
    pub work: String,
    /// Locator inside the work.
    pub locator: String,
}

impl Locator {
    /// A verse locator.
    #[must_use]
    pub fn verse(work: &str, verse: VerseId) -> Locator {
        Locator {
            work: work.to_owned(),
            locator: verse.0.to_string(),
        }
    }
}

/// How an edge came to exist (specs/19 item 4). Model similarity never carries a curated
/// relation type; engines reject such rows at load time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceKind {
    /// A named curated source such as TSK or OpenBible.
    Curated,
    /// Produced by a parser; the method names its version.
    Extracted,
    /// A model's similarity judgement; the method names model and version.
    ModelSimilarity,
    /// Device-local user material.
    User,
}

/// A typed, attributed relation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// Source end.
    pub src: Locator,
    /// Relation type (`cross_reference`, `quotes`, `cites`, `synonym_of`, ...).
    pub rel: String,
    /// Destination end.
    pub dst: Locator,
    /// Source-defined weight (votes, confidence); 0 when none.
    pub weight: f32,
    /// Provenance.
    pub provenance: ProvenanceKind,
    /// The pack or work that asserts the edge.
    pub source_work: String,
}

/// Which way a traversal follows edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// From src to dst.
    Outbound,
    /// From dst to src (backlinks).
    Inbound,
    /// Both.
    Both,
}

/// Limits for a multi-hop traversal (specs/19 item 7: depth limits, cycle detection, path
/// deduplication).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraversalOptions {
    /// Relation types to follow; empty means all.
    pub rels: Vec<String>,
    /// Direction.
    pub direction: Direction,
    /// Maximum hops, 1 to 6.
    pub max_depth: u8,
    /// Maximum edges expanded per node per hop (fan-out cap); 0 means unlimited.
    pub fan_out: u32,
    /// Maximum reached nodes.
    pub max_nodes: usize,
}

/// One node reached by a traversal, with the first (shortest) path to it. Each node appears
/// once however many paths lead to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reached {
    /// The node.
    pub node: Locator,
    /// Hops from the start.
    pub depth: u8,
    /// The edges of the first path found.
    pub path: Vec<Edge>,
}

/// Typed relations between passages, tokens, lexemes and work locators.
///
/// ADR 0002 chooses one typed edge table with covering indexes and recursive traversals,
/// plus a device-local link index merged from every installed pack (specs/19,
/// Cross-resource links). Counts are library-relative: they describe the installed set.
pub trait RelationStore {
    /// Edges pointing at a locator ("who cites this"), newest pack first is not guaranteed.
    fn backlinks(&self, target: &Locator, rel: Option<&str>, limit: usize) -> Result<Vec<Edge>>;
    /// Number of inbound edges across the installed library.
    fn backlink_count(&self, target: &Locator, rel: Option<&str>) -> Result<u64>;
    /// Edges leaving a locator.
    fn outlinks(&self, source: &Locator, rel: Option<&str>, limit: usize) -> Result<Vec<Edge>>;
    /// Breadth-first traversal with cycle detection and per-node deduplication.
    fn traverse(&self, start: &Locator, options: &TraversalOptions) -> Result<Vec<Reached>>;
    /// The most-cited locators of one work in the installed library.
    fn most_cited(&self, work: &str, rel: Option<&str>, limit: usize) -> Result<Vec<(Locator, u64)>>;
}

/// Vector storage precision (specs/19 item 5: float32 never ships to devices).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quantization {
    /// One signed byte per dimension.
    Int8,
    /// One bit per dimension, Hamming distance.
    Binary,
}

/// Identity of a vector set; corpus and query vectors must agree on all of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorSetInfo {
    /// Set name inside the pack.
    pub name: String,
    /// Embedding model id.
    pub model: String,
    /// Model revision.
    pub revision: String,
    /// Dimensions.
    pub dimensions: u32,
    /// Storage quantization.
    pub quantization: Quantization,
    /// Vectors stored.
    pub count: u64,
}

/// A query vector in the set's own quantization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryVector {
    /// Int8 components, `dimensions` long.
    Int8(Vec<i8>),
    /// Packed bits, `dimensions / 8` bytes.
    Binary(Vec<u8>),
}

/// One nearest neighbour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorHit {
    /// The stored row id (chunk id).
    pub id: i64,
    /// Distance in the set's metric (L2 for int8, Hamming for binary).
    pub distance: f32,
}

/// Semantic nearest-neighbour search over precomputed, quantized vectors.
///
/// ADR 0002 chooses sqlite-vec brute force (binary or int8 coarse scan, int8 rescoring) with
/// USearch as the escalation path. Corpus and query vectors must come from the identical
/// model, revision, prefixes and quantization; the manifest records them.
pub trait VectorIndex {
    /// Vector sets the open packs carry.
    fn vector_sets(&self) -> Result<Vec<VectorSetInfo>>;
    /// The `k` nearest vectors of a set.
    fn nearest(&self, set: &str, query: &QueryVector, k: usize) -> Result<Vec<VectorHit>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A backend that provides nothing. Proves the five traits can be implemented together
    /// by one type, which is how the embedded SQLite engine presents itself.
    struct Null;

    impl TextStore for Null {
        fn editions(&self) -> Result<Vec<EditionInfo>> {
            Ok(Vec::new())
        }
        fn verse_text(&self, edition: &str, _: VerseId) -> Result<Option<String>> {
            Err(RetrievalError::UnknownEdition(edition.to_owned()))
        }
        fn chapter_text(&self, edition: &str, _: u8, _: u16) -> Result<Vec<(VerseId, String)>> {
            Err(RetrievalError::UnknownEdition(edition.to_owned()))
        }
        fn map_verse(&self, _: &str, _: VerseId, _: &str) -> Result<Vec<VerseMapping>> {
            Ok(Vec::new())
        }
    }
    impl TextIndex for Null {
        fn search_text(&self, _: &TextQuery) -> Result<Vec<TextHit>> {
            Ok(Vec::new())
        }
        fn normalization_mode(&self) -> &'static str {
            "none"
        }
        fn normalize(&self, text: &str) -> String {
            text.to_owned()
        }
    }
    impl TokenEngine for Null {
        fn token_search(&self, q: &TokenQuery) -> Result<TokenHits> {
            Err(RetrievalError::UnknownEdition(q.edition.clone()))
        }
        fn token_info(&self, edition: &str, _: TokenPos) -> Result<TokenInfo> {
            Err(RetrievalError::UnknownEdition(edition.to_owned()))
        }
        fn facets(&self, _: &str, _: &[TokenPos], _: &str) -> Result<Vec<Facet>> {
            Ok(Vec::new())
        }
    }
    impl RelationStore for Null {
        fn backlinks(&self, _: &Locator, _: Option<&str>, _: usize) -> Result<Vec<Edge>> {
            Ok(Vec::new())
        }
        fn backlink_count(&self, _: &Locator, _: Option<&str>) -> Result<u64> {
            Ok(0)
        }
        fn outlinks(&self, _: &Locator, _: Option<&str>, _: usize) -> Result<Vec<Edge>> {
            Ok(Vec::new())
        }
        fn traverse(&self, _: &Locator, _: &TraversalOptions) -> Result<Vec<Reached>> {
            Ok(Vec::new())
        }
        fn most_cited(&self, _: &str, _: Option<&str>, _: usize) -> Result<Vec<(Locator, u64)>> {
            Ok(Vec::new())
        }
    }
    impl VectorIndex for Null {
        fn vector_sets(&self) -> Result<Vec<VectorSetInfo>> {
            Ok(Vec::new())
        }
        fn nearest(&self, set: &str, _: &QueryVector, _: usize) -> Result<Vec<VectorHit>> {
            Err(RetrievalError::LayerUnavailable(set.to_owned()))
        }
    }

    fn assert_backend<B>(_: &B)
    where
        B: TextStore + TextIndex + TokenEngine + RelationStore + VectorIndex,
    {
    }

    #[test]
    fn one_type_can_implement_the_whole_retrieval_boundary() {
        assert_backend(&Null);
    }

    #[test]
    fn verse_id_round_trips_its_parts() {
        let id = VerseId::new(43, 3, 16);
        assert_eq!(id.0, 43_003_016);
        assert_eq!((id.book(), id.chapter(), id.verse()), (43, 3, 16));
        assert_eq!(id.chapter_key(), 43_003);
        assert!(VerseId::new(1, 1, 1) < VerseId::new(1, 1, 2));
        assert!(VerseId::new(1, 50, 26) < VerseId::new(2, 1, 1));
    }

    #[test]
    fn queries_serialize_for_the_bindings() {
        let q = TokenQuery {
            edition: "n1904".into(),
            slots: vec![TokenPattern::one("lemma", "ἀγαπάω").and("mood", "imperative")],
            scope: Unit::Container("clause".into()),
            constraints: vec![Constraint::Agree {
                a: 0,
                b: 0,
                features: vec!["case".into()],
            }],
            book: Some(43),
            limit: 10,
        };
        let json = serde_json::to_string(&q).unwrap();
        let back: TokenQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(back, q);
        assert!(json.contains("\"Container\":\"clause\""));
    }
}
