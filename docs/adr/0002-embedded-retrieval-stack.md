# ADR 0002 — Embedded retrieval stack: SQLite, FTS5, bitmap corpus engine, sqlite-vec, edge tables

Status: accepted by the owner on 27 October 2026 as the starting point ("start with SQLite, but don't get stuck"); **measured by B064 on 28 October 2026** (R3, R5, R6, R7, R9; see "B064 measurements" below and docs/evidence/B064.md); B066 (semantic quality) pending. Detailed contract: specs/19-search-and-retrieval.md. Evidence: reports/Bible corpus search architecture.md.

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
- **Don't get stuck.** The core reaches storage and relations only through its own traits (`TextStore`, `TextIndex`, `TokenEngine`, `RelationStore`, `VectorIndex`), with SQLite as the first implementation of each. The conformance suite runs against the traits, pack manifests name the engine and version, and no UI, tool-registry or agent caller sees SQL. A later engine (GraphQLite, LadybugDB, PGlite on the web, or anything R9 justifies) is a second implementation plus a pack migration, not a rewrite.

## B064 measurements (28 October 2026)

Measured on the owner's Mac mini (Apple M1, macOS 26.4, Xcode 26.6, iOS 26.5 simulator) with `crates/retrieval-sqlite` as the first implementation of the five traits. No physical iPad, iPhone or Android device was available; simulator numbers run on the Mac's own CPU and the Android build was not attempted (no NDK). Full tables, commands and provenance: docs/evidence/B064.md.

| Benchmark | Result | Decision |
|---|---|---|
| R3 binary size | The engine (bundled SQLite 3.54.1 with FTS5, the `openlogos` tokenizer, roaring, sqlite-vec 0.1.9, engine and link index) adds 1.24–1.27 MB to the stripped release `libopenlogos_ffi.dylib` on aarch64-apple-darwin, -ios and -ios-sim (0.83 MB → 2.06–2.09 MB). On wasm32 it adds 886 KB raw / 434 KB gzipped (428 KB → 1.31 MB after wasm-opt; 159 KB → 593 KB gzipped). | **Kept.** Within specs/01's time budgets; no byte budget is set. |
| wasm32 path | Option (a), SQLite compiled into the Rust module by sqlite-wasm-rs 0.5.5 through rusqlite's own wasm backend, builds and runs FTS5 with the custom tokenizer inside a headless-Chrome dedicated worker. Caveats: SQLite 3.53.0 (0.6.x: 3.53.4, needs wasm-bindgen 0.2.128), sqlite-vec not compiled for wasm yet, the C compile needs an unwrapped clang. | **Chosen**, with the official `@sqlite.org/sqlite-wasm` JS build as the fallback if the 3.54 OPFS fix or sqlite-vec cannot be brought in. |
| R6 linguistic correctness | Fifty queries (lemma, lemma+morphology, proximity, agreement, unit scope, PEER/XPEER/ANCESTOR) on N1904 match Text-Fabric 13.1.1 counts 50/50 after two fixes: feature values compare in NFC, and discontinuous containers keep exact slot membership. Latency on the Mac and the iPad simulator: sub-millisecond p50 for lemma, morphology, proximity and unit scopes; agreement over whole part-of-speech classes p95 ≈ 11–16 ms. | **Kept.** The bitmap token engine is correct and far inside the 300 ms search budget. |
| R5 vector brute force | Mac, k = 20, sqlite-vec at -O3 with NEON: binary 500k × 768 in 21 ms p50 (60 MB), int8 500k × 256 in 40 ms (140 MB), int8 500k × 768 in 102 ms (397 MB); at the workspace's opt-level z without NEON the int8 numbers are 5–6× worse. Simulator numbers are noisy upper bounds; no device, no browser. | **Kept**, and the item 5 plan (binary coarse scan, int8 rescoring, 256-d compact set) confirmed. sqlite-vec must be compiled for speed (profile override, now in the workspace). |
| R7 opfs-sahpool | Not run: the sahpool VFS (sqlite-wasm-vfs) and the two-worker test were not built inside the wasm timebox. | Open; blocks nothing native. |
| R9 relations | 5.0M synthetic edges across 200 packs; link index 489 MB, rebuild 2–5 min under load, incremental install of one pack 0.5–2.2 s. Mac p95: backlinks ≤ 63 ms on hubs (0.8 ms with a relation filter), backlink counts ≤ 0.3 ms, 1–2 hops ≤ 23 ms, capped 3–6 hop traversals materializing 5,000 nodes 310–360 ms, quotation chains 64 ms, uncapped recursive CTE to depth 3 7.5 ms; library-wide `most_cited` 0.4–1.9 s. | **Kept; no graph-engine escalation.** `most_cited` becomes a count table maintained on pack install; the join order of the batched traversal is pinned (planner regression found and fixed). |

Replacements named by B064: none of the stack components failed outright. The int8 brute-force path at 500k × 768 exceeds the budget on the Mac, so the plan of specs/19 item 5 stands (binary coarse scan with int8 rescoring of a short list; USearch as the ANN escalation). No graph-engine escalation is warranted by R9's numbers; the link index's library-relative aggregate (`most_cited`) should be materialized per install rather than computed per query.

