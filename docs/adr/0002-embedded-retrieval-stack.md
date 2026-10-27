# ADR 0002 — Embedded retrieval stack: SQLite, FTS5, bitmap corpus engine, sqlite-vec, edge tables

Status: proposed for blueprint revision 5 (27 October 2026); owner confirmation and B064/B066 measurements pending. Detailed contract: specs/19-search-and-retrieval.md. Evidence: reports/Bible corpus search architecture.md.

## Context

The owner asked for traditional search, advanced linguistic search, vectorized semantic (RAG) retrieval and concept relations, considering OpenSearch/Elasticsearch-class engines and Neo4j-class graph databases. The requirements are offline use on iPad and phone, identical results across platforms, and full function without AI.

## Decision

| Need | Choice | Rejected for the default tier |
|---|---|---|
| Canonical store, every client | SQLite (rusqlite; sqlite-wasm `opfs-sahpool` ≥ 3.54 on web/Obsidian) | Turso (pre-1.0, no Swift/Kotlin bindings; watch list) |
| Keyword/phrase/Boolean | FTS5 with one shared Rust Greek/Hebrew normalizing tokenizer | Tantivy (no official WASM; document-oriented); OpenSearch/Elasticsearch, Meilisearch, Typesense (cannot run on device; duplicate query logic; some copyleft/source-available) |
| Lemma, morphology, proximity, agreement, syntax, alignment | Custom Rust columnar token engine on roaring bitmaps, after Text-Fabric's model | Lucene-style engines (cannot express token-feature lattices without reimplementation) |
| Semantic vectors | sqlite-vec brute force over quantized, precomputed vectors; USearch as ANN escalation | sqlite-vss (dormant), instant-distance (archived), LanceDB/DuckDB vss at runtime (mobile and persistence immaturity) |
| Relations | Typed edge table + indexed lookups and `WITH RECURSIVE`, a device-local link index across installed packs, petgraph for in-memory analytics | Neo4j (GPLv3 community, server-only), Memgraph (BSL), Kuzu (archived 2025), CozoDB (dormant); PGlite + Apache AGE (browser-only, no native mobile runtime). Escalation candidates gated on R9: GraphQLite, then LadybugDB |
| Optional server | Same Rust core and packs; PostgreSQL + pgvector for multi-user data and large vector sets | Dedicated search or graph clusters unless measured need appears |

## Consequences

- No-AI mode is complete: every deterministic query runs on device from signed packs.
- The linguistic engine is in-house work. Its correctness oracle is Text-Fabric on N1904 (B064, benchmark R6).
- The most uncertain choice is semantic quality on Koine Greek and Biblical Hebrew. Until B066 passes, semantic search anchors on translations, and original-language vectors remain a labeled experiment.
- sqlite-vec has one maintainer, revived in 2026, and its ANN indexes are alpha. Brute force keeps the dependency small and replaceable.
- Model similarity is stored and displayed separately from curated relations; agents receive the same labels.
- Revisited 27 October 2026 at the owner's request: offline graph engines do exist (PGlite + Apache AGE in browsers, LadybugDB and GraphQLite in-process). None yet has demonstrated iOS, Android and WASM builds from one engine, so SQLite remains the default. The decision is revisited if benchmark R9 fails or if libpglite ships native mobile builds. Traversals are exposed as core functions, so swapping the engine does not change UI or agent callers.
- Links between works need work-level locators, section maps and a device-local link index, because packs are separate SQLite files and SQLite attaches at most 125 databases (specs/19-search-and-retrieval.md, Cross-resource links).
