//! Optional semantic retrieval and the storage boundary of
//! docs/adr/0002-embedded-retrieval-stack.md. The five traits declared here (TextStore,
//! TextIndex, TokenEngine, RelationStore and VectorIndex) are the seams between the core's
//! query logic and the embedded engine: SQLite as canonical store, FTS5 for keyword search,
//! the bitmap token engine, a typed edge table for relations and sqlite-vec brute force over
//! quantized vectors. Hybrid retrieval fuses BM25 and vector k-nearest-neighbour results by
//! reciprocal rank fusion, and every result is labeled with the model and version that
//! produced it so similarity is never presented as a documented relation
//! (specs/19-search-and-retrieval.md). No SQLite implementation exists yet; B064 and B066 add
//! and measure it.

/// Canonical storage for passages, user data and pack metadata.
///
/// ADR 0002 chooses SQLite: rusqlite on native targets, sqlite-wasm with the `opfs-sahpool`
/// VFS (SQLite 3.54 or later) on the web. Corpus packs are read-only and attached on demand;
/// user data lives in a separate mutable database. Methods arrive with B064.
pub trait TextStore {}

/// Keyword, phrase and Boolean full-text search over stored text.
///
/// ADR 0002 chooses FTS5 with one shared Rust Greek/Hebrew normalizing tokenizer so the index
/// and the query parser normalize identically. Methods arrive with B064.
pub trait TextIndex {}

/// The linguistic query engine: lemma, morphology, proximity, agreement, syntax and alignment.
///
/// ADR 0002 chooses a custom columnar token engine on roaring bitmaps after Text-Fabric's
/// slot/feature/edge model (specs/19-search-and-retrieval.md). Methods arrive with B064,
/// whose correctness oracle is Text-Fabric on N1904.
pub trait TokenEngine {}

/// Typed relations between passages, tokens, lexemes and work locators.
///
/// ADR 0002 chooses one typed edge table with covering indexes and `WITH RECURSIVE`
/// traversals, plus a device-local link index across installed packs. A `model_similarity`
/// row may never carry a curated relation type. Methods arrive with B064; the graph-engine
/// choice is revisited only if benchmark R9 fails.
pub trait RelationStore {}

/// Semantic nearest-neighbour search over precomputed, quantized vectors.
///
/// ADR 0002 chooses sqlite-vec brute force (binary or int8 coarse scan, int8 rescoring) with
/// USearch as the escalation path. Corpus and query vectors must come from the identical
/// model, revision, prefixes and quantization; the manifest records them. Methods arrive with
/// B064 and are measured in B066.
pub trait VectorIndex {}

#[cfg(test)]
mod tests {
    use super::{RelationStore, TextIndex, TextStore, TokenEngine, VectorIndex};

    /// A backend that provides nothing. Proves the five traits can be implemented together
    /// by one type, which is how the embedded SQLite engine will present itself.
    struct Null;

    impl TextStore for Null {}
    impl TextIndex for Null {}
    impl TokenEngine for Null {}
    impl RelationStore for Null {}
    impl VectorIndex for Null {}

    fn assert_backend<B>(_: &B)
    where
        B: TextStore + TextIndex + TokenEngine + RelationStore + VectorIndex,
    {
    }

    #[test]
    fn one_type_can_implement_the_whole_retrieval_boundary() {
        assert_backend(&Null);
    }
}
