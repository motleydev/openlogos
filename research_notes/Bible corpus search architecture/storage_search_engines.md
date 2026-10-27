# Storage, Search, Vector and Graph Technologies for OpenLogos (embedded + server), status as of 2026-10-27

Scope: embedded and server-side storage/search/vector/graph options for an offline-first Rust core (UniFFI to Swift on Apple platforms, Kotlin on Android, WASM in a worker with OPFS on the web, Tauri on desktop, WASM in an Obsidian plugin), plus an optional self-hosted server.

Method note: repository status (archived flag, license SPDX, last push, latest release tag and date) was pulled live from the GitHub REST API on 2026-10-27 (`https://api.github.com/repos/<owner>/<repo>` and `/releases?per_page=1`). The API rate-limited before it reached pgvector, ParadeDB, Neo4j, Apache AGE, Memgraph and sqlite/sqlite-wasm, so those rows rely on web sources or are marked unverified. "Latest release" is whatever GitHub returns as the most recent release object. For repos that publish several release lines (OpenSearch 2.x/3.x, for example) it may not be the highest version.

## Comparative evaluation table

Legend: Y = supported and documented; P = possible but unofficial or experimental; N = no / not documented; ? = not verified.

| Technology | Role | iOS / Android embeddable | Browser WASM | License (GitHub SPDX) | Latest release (GitHub, date) | Maintenance as of Oct 2026 | Fit for OpenLogos |
|---|---|---|---|---|---|---|---|
| SQLite + FTS5 | Relational store + FTS | Y / Y (ships in OS; can also bundle via rusqlite) | Y (official sqlite-wasm, OPFS VFS) | Public domain | ? (rate-limited) | Very active (SQLite 3.5x line; OPFS doc mentions 3.50/3.54 changes) | **Core choice** for user data, metadata, verse/paragraph FTS, edge tables |
| sqlite-vec | Vector search SQLite ext | Android binaries in releases; iOS via static link (P, `spm.json` asset seen but not documented) | Y (project tagline "runs anywhere"; not re-verified this session) | Apache-2.0 | v0.1.10-alpha.4, 2026-05-18 (stable v0.1.9, 2026-03-31) | Revived March 2026 ("sqlite-vec is back!"); ANN (DiskANN, rescore; IVF experimental) only in alphas | **Good on-device choice**: brute force is fine for small sets; ANN still alpha |
| sqlite-vss | Faiss-based SQLite ext | Hard (Faiss) | N | MIT | v0.1.2, 2023-08-06 | Dormant (last push 2024-05); superseded by sqlite-vec | Avoid |
| SQLite official `vec1` | Vector ANN (IVFADC+OPQ) | ? | ? | Public domain (presumably) | ? | Doc exists in sqlite.org tree; maturity unknown | Watch |
| libSQL (Turso fork of C SQLite) | SQLite fork + native vectors + sync | Y (has had mobile SDKs; not re-verified) | P | MIT | libsql-server-v0.24.32, 2025-02-14 | Repo pushed 2026-10-17, but company focus moved to the Rust rewrite | Viable, but strategic direction uncertain |
| Turso (Rust rewrite, ex-Limbo) | SQLite-compatible engine in Rust | Bindings listed: Go, JS, Java, .NET, Python, Rust, WASM; Swift/Kotlin not listed | Y (WASM binding) | MIT | v0.9.0-pre.1, 2026-10-15 | Very active; **pre-1.0**, FTS (Tantivy-based) experimental, vector ANN index "on roadmap" | Watch; not yet a safe base for the canonical store |
| Tantivy | Lucene-style FTS library (Rust) | Y via Rust (community TantivySwift exists) | P (community builds only; official WASM RFC #541) | MIT | 0.26.1, 2026-05-10 | Very active (Quickwit/Datadog) | Strong for server and native; WASM is DIY |
| roaring-rs / CRoaring | Compressed bitmaps | Y / Y (pure Rust / C) | Y (pure Rust compiles to wasm32) | Apache-2.0 / Apache-2.0 OR MIT (API: NOASSERTION) | v0.11.5, 2026-08-12 / v5.4.0, 2026-10-26 | Active | **Recommended** for custom linguistic/positional index |
| USearch | HNSW vector index (C++) | Y (C/C++, Swift, Java bindings) | P (has a WASM target; not re-verified) | Apache-2.0 | v2.26.6, 2026-10-25 | Very active | Good if sqlite-vec ANN is insufficient on device |
| hnswlib | HNSW (header-only C++) | P | P | Apache-2.0 | v0.10.0-rc.2, 2026-09-14 | Active again (RC) | Usable; USearch is more embed-friendly |
| instant-distance | HNSW in pure Rust | Y (pure Rust) | Y (pure Rust) | Apache-2.0 | none | **Archived** (now djc/instant-distance, archived) | Avoid |
| LanceDB / Lance | Columnar format + embedded vector DB | Not documented for mobile | N (not documented) | Apache-2.0 | lancedb v0.41.0-beta.15, 2026-10-27; lance v15.0.0-beta.3 (repo moved to `lance-format` org) | Very active, fast-moving, beta-tagged | Server/desktop analytics; not a mobile/WASM base |
| DuckDB (+ vss) | OLAP SQL engine + HNSW ext | P (C library; no official mobile packages verified) | Y (duckdb-wasm v1.33.0, 2025-12-16) | MIT | DuckDB v2.0.0, 2026-10-21 | Very active | Build/ETL tooling; vss persistence experimental |
| Kuzu | Embedded property graph | (was Y-ish) | (had WASM) | MIT | v0.11.3, 2025-10-10 | **Archived 2025-10-10 after Apple acquisition** | Do not adopt |
| LadybugDB (Kuzu fork) | Embedded property graph (Cypher) | Swift (lbug-swift), Java; iOS/Android not explicitly stated | Y (WASM bindings) | MIT | v0.22.0, 2026-10-25 | Active community fork; governance unclear | Only if a graph DB is truly needed |
| CozoDB | Datalog relational-graph-vector | Y (historically had mobile builds) | Y (historically) | MPL-2.0 | v0.7.6, 2023-12-11 | **Dormant** (last push 2024-12-04) | Avoid |
| redb | Pure-Rust KV | Y | P (needs custom storage backend) | Apache-2.0 | v4.4.0, 2026-10-13 | Active, stable 4.x | Good for custom index blobs on native |
| sled | Rust KV | Y | ? | Apache-2.0 | v0.34.7, 2021-09-12 | Effectively stalled releases (repo pushed 2026-04) | Avoid |
| heed (LMDB) | Typed LMDB wrapper | Y (C LMDB) | N (mmap) | MIT | v0.23.0, 2026-10-22 | Active (Meilisearch) | Native only |
| Fjall | LSM KV in Rust | Y | ? | Apache-2.0 | 3.1.13, 2026-10-25 | Active | Write-heavy native; unnecessary here |
| OpenSearch | Server search + k-NN | n/a | n/a | Apache-2.0 | 2.19.7, 2026-10-20 (2.x maintenance line; 3.x status not verified) | Linux Foundation governance | Heavy; overkill unless scale grows |
| Elasticsearch | Server search + vectors | n/a | n/a | AGPLv3 / SSPL / ELv2 (triple) | v9.5.5, 2026-10-06 | Very active | Heavy; AGPL option |
| Meilisearch | Server search + hybrid | n/a | n/a | MIT (CE) + BUSL-1.1 (EE parts) | v1.56.0, 2026-10-26 | Very active | Easy ops; weak for linguistic queries |
| Typesense | Server search + vectors | n/a | n/a | GPL-3.0 | v30.2, 2026-04-19 | Active | Similar to Meilisearch |
| Qdrant | Vector DB | n/a | n/a | Apache-2.0 | v1.20.0, 2026-10-22 | Very active | Best dedicated vector server, if needed |
| Weaviate | Vector DB | n/a | n/a | BSD-3-Clause (API returned NOASSERTION) | v1.37.18, 2026-10-25 | Very active | Heavier than Qdrant |
| Postgres + pgvector | RDBMS + vectors | n/a | n/a (PGlite exists; not researched) | PostgreSQL License | ? (rate-limited) | ? | **Recommended server base** |
| ParadeDB pg_search | BM25 in Postgres (Tantivy-based) | n/a | n/a | AGPL-3.0 (Community) | pg_search 0.25.6 (pgxn, 2026-08-27); 0.26.1 listed by Pigsty | Active | Good if Postgres FTS ranking is insufficient |
| Postgres FTS (tsvector) | Built-in FTS | n/a | n/a | PostgreSQL License | n/a | Core | Adequate for basic server keyword search |
| Neo4j | Graph DB | n/a | n/a | GPLv3 (Community); commercial (Enterprise) | ? | Active | Not warranted |
| Apache AGE | Cypher on Postgres | n/a | n/a | Apache-2.0 | ? (rate-limited) | ? | Unnecessary given PG19 SQL/PGQ |
| Memgraph | In-memory graph DB | n/a | n/a | BSL (Community) / MEL (Enterprise) | ? | Active (not verified) | Not warranted |

---

## 1. Embedded SQLite stack: FTS5, official WASM/OPFS build, sqlite-vec, sqlite-vss, libSQL, Turso

### Takeaway
SQLite (with FTS5 and custom tokenizers) remains the only relational engine that is first-class on every OpenLogos target, including the browser via the official sqlite-wasm OPFS VFSes. sqlite-vec came back in March 2026 and is the right embedded vector extension, though its ANN indexes are still alpha. sqlite-vss is dead. Turso's Rust rewrite is promising (WASM, Tantivy FTS, MVCC) but is pre-1.0 and lists no Swift/Kotlin bindings.

### Cited Findings
- The official SQLite WASM build offers two OPFS VFSes. `opfs` must run in a Worker, needs COOP/COEP headers (SharedArrayBuffer) and supports multiple connections with locking. `opfs-sahpool` must be installed from a Worker, needs no COOP/COEP, has the highest performance (especially for batch operations), stores files under internal names, and offers only limited multi-connection support (pause/unpause API added in 3.50; a locking bug before 3.54). — [SQLite WASM persistence docs](https://sqlite.org/wasm/doc/trunk/persistence.md)
- OPFS locks files exclusively, including for reads, so there are no "N concurrent readers". 2026-03 testing handled 8–10 workers when clients handled SQLITE_BUSY. — [SQLite WASM persistence docs](https://sqlite.org/wasm/doc/trunk/persistence.md)
- Browser support: Chromium from about mid-2022, Firefox 111+, Safari 16.4+ for OPFS APIs. The `opfs` VFS fails on Safari below 17 because of a WebKit bug. `opfs-sahpool` works on all major browsers since March 2023. — [SQLite WASM persistence docs](https://sqlite.org/wasm/doc/trunk/persistence.md)
- FTS5 supports custom tokenizers through the `fts5_tokenizer` C API and has built-in `unicode61` (with `remove_diacritics`), `porter`, `ascii` and `trigram` tokenizers. The trigram tokenizer supports substring and LIKE/GLOB acceleration. FTS5 supports phrase, NEAR, AND/OR/NOT and prefix queries. — [SQLite FTS5 docs](https://www.sqlite.org/fts5.html) (canonical documentation; not re-fetched this session)
- sqlite-vec: Apache-2.0, not archived, about 8.2k stars, last push 2026-05-18. Latest release v0.1.10-alpha.4 (2026-05-18); latest stable v0.1.9 (2026-03-31). — [GitHub API: asg017/sqlite-vec](https://github.com/asg017/sqlite-vec)
- sqlite-vec v0.1.7 (17 Mar 2026) was announced as "sqlite-vec is back!". It added DELETE support and distance-range constraints in KNN queries, with no ANN index yet. v0.1.10-alpha.1 (31 Mar) added ANN indexes: rescore, DiskANN, and IVF (experimental, not enabled). alpha.2 noted that DiskANN DELETEs "are costly for now". The v0.1.9 assets include Android loadable builds (aarch64, armv7a, i686) and an `spm.json` file. — [sqlite-vec releases](https://github.com/asg017/sqlite-vec/releases)
- sqlite-vss: MIT, last release v0.1.2 on 2023-08-06, last push 2024-05-05. — [GitHub API: asg017/sqlite-vss](https://github.com/asg017/sqlite-vss)
- SQLite has its own `vec1` ANN extension doc using IVFADC with OPQ (L2, cosine) as a virtual table. Its release status and date were not found. — [sqlite.org vec1 doc](https://sqlite.org/vec1/doc/trunk/doc/vec1.md)
- libSQL (C fork): MIT, about 17.3k stars, pushed 2026-10-17. Last GitHub release object is libsql-server-v0.24.32 (2025-02-14). — [GitHub API: tursodatabase/libsql](https://github.com/tursodatabase/libsql)
- Turso (Rust rewrite): MIT, about 25k stars, pushed 2026-10-27, latest v0.9.0-pre.1 (2026-10-15). The repo describes itself as "SQLite-compatible, now also speaking Postgres (experimental)". — [GitHub API: tursodatabase/turso](https://github.com/tursodatabase/turso)
- The Turso README says it has "not yet reached 1.0" and recommends independent backups until 1.0. Vector support is exact search only, with ANN indexing "on the roadmap" (modeled on libSQL). FTS is experimental and Tantivy-based. MVCC is available via `BEGIN CONCURRENT`. A WebAssembly binding exists. Listed bindings are Go, JS, Java, .NET, Python, Rust and WASM, with no Swift/Kotlin. It targets SQLite 3.50.4 file and dialect compatibility but is not yet at 100%. — [tursodatabase/turso README](https://github.com/tursodatabase/turso)
- Secondary sources describe Turso FTS as storing Tantivy segments inside the SQLite B-tree and exposing `CREATE INDEX ... USING fts`, `fts_match` and `fts_score`. — [ChatForest review](https://chatforest.com/reviews/turso-mcp-server/); [instagit write-up](https://instagit.com/tursodatabase/turso/implementing-full-text-search-tantivy-turso) (possibly AI-generated; treat with caution)

### Inferences
- For Greek and Hebrew normalization, register a custom FTS5 tokenizer implemented in Rust (via rusqlite's FTS5 tokenizer support or a C shim). It would strip diacritics and accents, final-sigma fold, and remove niqqud and cantillation (Unicode Mn ranges U+0591–U+05C7). The same Rust function can normalize queries, so normalization logic lives once in the core and is reused on every platform, including WASM.
- With `opfs-sahpool`, OpenLogos avoids COOP/COEP headers, which matters for the Obsidian plugin (Electron renderer; header control is limited) and for simple web hosting. Its single-connection constraint fits a design with one Rust core in one worker.
- At on-device scale (verses ≈ 31k per edition, pericopes in the thousands, commentary paragraphs in the tens of thousands), brute-force sqlite-vec KNN over int8 or binary-quantized vectors is likely sub-100 ms. ANN alphas are not required for launch. This is an estimate and has not been benchmarked.
- Turso is worth re-evaluating at 1.0, especially if it ships Swift/Kotlin bindings and an ANN index. Because it is file-compatible, a later migration from SQLite would be low-cost.

### Gaps
- Binary-size figures for sqlite-wasm, sqlite-vec and Turso WASM were not measured or found this session. Typical sqlite3.wasm is often quoted near 1 MB compressed (training knowledge; unverified).
- sqlite-vec WASM and iOS packaging for 0.1.9/0.1.10 were not confirmed from primary docs, and the purpose of the `spm.json` asset is not explained.
- The SQLite `vec1` extension's release status, maturity and inclusion in amalgamation builds are unknown.
- sqlite/sqlite-wasm npm release data was not retrieved (rate-limited).

## 2. Embedded full-text, linguistic index and vector libraries: Tantivy, roaring, USearch, hnswlib, instant-distance, LanceDB, DuckDB

### Takeaway
Tantivy is healthy and strong natively, but it has no official WASM support, and browser builds are community efforts that need in-memory or custom directories. For token-level linguistic search (lemma, morphology, proximity), a custom Rust index built on roaring bitmaps (roaring-rs compiles everywhere) is the most portable option. USearch is the best-maintained standalone embeddable ANN. instant-distance is archived. LanceDB and DuckDB are server, desktop or build-time tools: DuckDB-vss persistence is experimental, and LanceDB documents no mobile or WASM targets.

### Cited Findings
- Tantivy: MIT, about 16.3k stars, pushed 2026-10-27, latest 0.26.1 (2026-05-10). — [GitHub API: quickwit-oss/tantivy](https://github.com/quickwit-oss/tantivy)
- Tantivy has an open "wasm RFC" (issue #541) for official WASM support, which noted that threading in WASM was a work in progress. — [tantivy issue #541](https://github.com/quickwit-oss/tantivy/issues/541)
- Community WASM builds exist. StyMaar/test-tantivy (TantivyJS) runs indexing and search in browsers and Node and exports/imports an index as a Uint8Array. Limitations: segments are not searchable before finalization, and documents can't be added or removed after finalization. — [StyMaar/test-tantivy](https://github.com/StyMaar/test-tantivy)
- phiresky/tantivy-wasm ran tantivy in the browser against a statically hosted index. Its WASM was about 15 MB in debug (4 MB gzipped) and "about 1.5MB" in release, but the repo is old. — [phiresky/tantivy-wasm](https://github.com/phiresky/tantivy-wasm)
- carbon/TantivySwift (a Swift wrapper) has PR #15 adding a single-threaded wasm32-wasip1 build for browsers without SharedArrayBuffer. — [TantivySwift PR #15](https://github.com/carbon/TantivySwift/pull/15)
- roaring-rs: Apache-2.0, v0.11.5 (2026-08-12), pushed 2026-10-27. CRoaring: v5.4.0 (2026-10-26), with SIMD for AVX2, AVX-512 and NEON. — [GitHub API: RoaringBitmap/roaring-rs](https://github.com/RoaringBitmap/roaring-rs); [GitHub API: RoaringBitmap/CRoaring](https://github.com/RoaringBitmap/CRoaring)
- USearch: Apache-2.0, v2.26.6 (2026-10-25), pushed 2026-10-25, described as a search engine in C++ and C with multiple language bindings. — [GitHub API: unum-cloud/usearch](https://github.com/unum-cloud/usearch)
- hnswlib: Apache-2.0, v0.10.0-rc.2 (2026-09-14). — [GitHub API: nmslib/hnswlib](https://github.com/nmslib/hnswlib)
- instant-distance: the repository (resolving to djc/instant-distance) is **archived** and has no releases. — [GitHub API: instant-labs/instant-distance](https://github.com/instant-labs/instant-distance)
- LanceDB: Apache-2.0, v0.41.0-beta.15 (2026-10-27). The Lance format repo moved to the `lance-format` org: v15.0.0-beta.3 (2026-10-27), "Open Lakehouse Format for Multimodal AI". — [GitHub API: lancedb/lancedb](https://github.com/lancedb/lancedb); [GitHub API: lance-format/lance](https://github.com/lance-format/lance)
- No official LanceDB iOS/Android support statements or issues were found. Only third-party directories claim it suits mobile and edge use. — [LanceDB OSS FAQ](https://docs.lancedb.com/faq/faq-oss); [modern-datatools review](https://www.modern-datatools.com/tools/lancedb) (low-quality secondary)
- DuckDB v2.0.0 was released 2026-10-21 (MIT). duckdb-wasm's latest release is v1.33.0 (2025-12-16), and its repo was pushed 2026-10-25. duckdb-vss has no GitHub releases; its repo was pushed 2026-10-14. — [GitHub API: duckdb/duckdb](https://github.com/duckdb/duckdb); [duckdb/duckdb-wasm](https://github.com/duckdb/duckdb-wasm); [duckdb/duckdb-vss](https://github.com/duckdb/duckdb-vss)
- DuckDB vss HNSW details:
  - By default, HNSW indexes can only be created in in-memory databases. Disk persistence requires `hnsw_enable_experimental_persistence`, which is not recommended for production because "WAL recovery is not yet properly implemented for custom indexes".
  - The whole index is reserialized at each checkpoint.
  - "The index itself is not buffer managed and must be able to fit into RAM."
  - Only FLOAT32 vectors are supported.
  - [DuckDB 2.0 vss docs](https://duckdb.org/docs/2.0/core_extensions/vss.html)

### Inferences
- **Linguistic search design.** Store token feature columns (lemma ID, morph code, surface form, normalized form, syntactic node IDs) as flat arrays keyed by global token position. Build inverted postings per feature value as roaring bitmaps of token positions. Then:
  - Boolean AND/OR/NOT across features (for example lemma = λόγος AND case = genitive) is bitmap intersection.
  - Proximity "within N words" is a shift-and-intersect, or a merge over sorted positions.
  - Verse, pericope or sentence scoping intersects with range bitmaps.
  - At about 1–2M tokens per edition this is microsecond-to-millisecond work. roaring-rs is pure Rust, so the same code runs on iOS, Android, desktop and WASM.
  - The bitmaps can be persisted as BLOBs in SQLite, so no separate storage engine is needed.
- Tantivy is a reasonable choice for the server and for native-only desktop. It is not a good choice as the single cross-platform FTS engine, because the browser and Obsidian WASM paths would depend on unofficial builds and immutable segments. FTS5 plus a custom tokenizer covers verse- and paragraph-level keyword search everywhere.
- If on-device vector counts grow past what brute force handles (on the order of hundreds of thousands of vectors), USearch is the most portable maintained ANN, with C ABI, Swift and Java bindings. It can serialize to a blob or file.
- DuckDB is valuable in the corpus build pipeline (ETL from TEI/OSIS/USFM/morphology TSVs into Parquet and SQLite) rather than at app runtime.

### Gaps
- Tantivy WASM binary size with the current version (0.26) was not measured. The 1.5 MB figure comes from an old demo.
- I could not verify that USearch has a maintained WASM build or confirm its packaged binary sizes.
- I could not confirm whether duckdb-wasm supports loading the vss extension in the browser.
- No benchmarks were found specific to Greek or Hebrew morphological corpora on these engines.

## 3. Embedded graph and KV stores: Kuzu (archival), LadybugDB, CozoDB, redb, sled, heed, Fjall

### Takeaway
**Verified:** Apple agreed to acquire Kùzu Inc. on 2025-10-09, and the kuzudb/kuzu repo was archived on 2025-10-10. Its last release was v0.11.3 on the same day. An actively maintained MIT fork, **LadybugDB**, released v0.22.0 on 2026-10-25 and has WASM, Rust, Swift and Java bindings, but its governance is opaque. CozoDB has had no release since December 2023. Among KV stores, redb (4.x), heed and Fjall are active, while sled's last release dates to 2021.

### Cited Findings
- An EU disclosure shows that Apple agreed on 2025-10-09 to buy all of Kuzu's shares and hire select employees, through an unnamed subsidiary. Kuzu's GitHub repo was archived on 2025-10-10 and its website went offline. — [BetaKit](https://betakit.com/apple-strikes-deal-to-acquire-canadian-database-software-startup-kuzu/); [University of Waterloo CS news](https://uwaterloo.ca/computer-science/news/waterloo-based-graph-database-start-up-kuzu-acquired-apple); [heise](https://heise.de/-11177751)
- The GitHub API confirms kuzudb/kuzu is `archived=true`, MIT, last pushed 2025-10-10, with last release v0.11.3 (2025-10-10). The description reads "Embedded property graph database built for speed. Vector search and full-text search built in". — [GitHub API: kuzudb/kuzu](https://github.com/kuzudb/kuzu)
- LadybugDB describes itself as formerly Kuzu: an embedded property-graph database with Cypher, native FTS and vector indices, columnar storage, CSR adjacency, a vectorized and factorized processor, serializable ACID and WebAssembly bindings. It is MIT-licensed. Bindings: Python (`ladybug`), Node (`@ladybugdb/core`), Rust (`lbug`), Go, Swift (`lbug-swift`), Java (Maven `com.ladybugdb/lbug`) and C/C++. iOS and Android are not mentioned explicitly. — [LadybugDB/ladybug](https://github.com/LadybugDB/ladybug)
- LadybugDB repo: not archived, about 1.9k stars, pushed 2026-10-27, latest release v0.22.0 (2026-10-25). — [GitHub API: LadybugDB/ladybug](https://github.com/LadybugDB/ladybug)
- DBDB lists LadybugDB as a 2025 "revival fork of Kuzu after its acquisition and closure by Apple" (MIT, C++). — [DBDB.io LadybugDB](https://dbdb.io/db/ladybugdb/revisions/2)
- A self-described maintainer on the Julia forum said Kuzu "is alive and well" under the name LadybugDB. That identity was not independently verified. — [Julia Discourse](https://discourse.julialang.org/t/kuzudb-or-general-graphdbs/133139/5)
- No GitHub org or repo named "bighorn" turned up in this session's searches. Its existence as a Kuzu fork is **unverified**.
- CozoDB: MPL-2.0, not archived, last push 2024-12-04, last release v0.7.6 (2023-12-11). — [GitHub API: cozodb/cozo](https://github.com/cozodb/cozo)
- redb: Apache-2.0, v4.4.0 (2026-10-13). sled: Apache-2.0, last release v0.34.7 (2021-09-12), self-described "beta", repo pushed 2026-04-04. heed: MIT, v0.23.0 (2026-10-22). Fjall: Apache-2.0, 3.1.13 (2026-10-25). — [GitHub API: cberner/redb](https://github.com/cberner/redb); [spacejam/sled](https://github.com/spacejam/sled); [meilisearch/heed](https://github.com/meilisearch/heed); [fjall-rs/fjall](https://github.com/fjall-rs/fjall)

### Inferences
- Adopting Kuzu itself is off the table. LadybugDB is technically attractive (Cypher plus vectors plus FTS in one embedded engine with WASM). However, it is a young fork with unclear governance, explicit iOS/Android packaging is unconfirmed, and it would duplicate storage that SQLite already provides. It is a weak bet for a core dependency.
- A separate KV store is unnecessary if SQLite is the canonical store. Custom index blobs (roaring bitmaps, USearch graphs) can live in SQLite BLOBs or sidecar files. redb is the best fallback if a pure-Rust KV is ever needed on native, though it has no OPFS backend.

### Gaps
- LadybugDB's governance (company or foundation, maintainer identities), mobile build status and binary size are unverified.
- The "bighorn" fork could not be found.
- Apple's plans for Kuzu technology are not public.

## 4. Server-side search, vector and graph engines

### Takeaway
All the major server engines are actively maintained as of October 2026, but their licenses differ:
- Permissive: Qdrant (Apache), OpenSearch (Apache), Weaviate (BSD), pgvector and Postgres (PostgreSQL License).
- Copyleft: Elasticsearch (AGPL option added in 2024), ParadeDB Community (AGPL), Typesense (GPL-3), Neo4j Community (GPLv3).
- Source-available: Memgraph Community (BSL), and Meilisearch, which since v1.19 (Aug 2025) is MIT CE plus BUSL EE, with sharding as the EE-only feature.

Postgres 19 adds SQL/PGQ property-graph queries, committed in March 2026.

### Cited Findings
- OpenSearch: Apache-2.0, latest GitHub release object 2.19.7 (2026-10-20), pushed 2026-10-27. — [GitHub API: opensearch-project/OpenSearch](https://github.com/opensearch-project/OpenSearch)
- Elasticsearch: v9.5.5 (2026-10-06). On 2024-08-29 Elastic added AGPLv3 as a license option for the free portion of Elasticsearch and Kibana (alongside SSPL 1.0 and ELv2). Binary distributions are unchanged, and clients remain Apache-2.0. Shay Banon: "We're delighted to reintroduce an OSI-approved open source license". — [GitHub API: elastic/elasticsearch](https://github.com/elastic/elasticsearch); [BusinessWire press release](https://www.businesswire.com/news/home/20240829537786/en); [ADMIN Magazine](https://www.admin-magazine.com/News/Open-Source-AGPL-Added-as-License-Option-for-Elasticsearch/(tagID)/52)
- Meilisearch: v1.56.0 (2026-10-26). It moved to dual licensing in August 2025 with v1.19. Community Edition is MIT, and Enterprise Edition parts are BUSL-1.1, which converts to MIT four years after publication. Current docs say sharding is the only EE-exclusive feature. — [GitHub API: meilisearch/meilisearch](https://github.com/meilisearch/meilisearch); [Meilisearch EE license blog](https://www.meilisearch.com/blog/enterprise-license); [Meilisearch EE docs](https://meilisearch.com/docs/resources/self_hosting/enterprise_edition)
- Typesense: GPL-3.0, v30.2 (2026-04-19). — [GitHub API: typesense/typesense](https://github.com/typesense/typesense)
- Qdrant: Apache-2.0, v1.20.0 (2026-10-22). — [GitHub API: qdrant/qdrant](https://github.com/qdrant/qdrant)
- Weaviate: v1.37.18 (2026-10-25). The GitHub API reports the license as NOASSERTION; Weaviate is generally BSD-3-Clause (training knowledge, not re-verified). — [GitHub API: weaviate/weaviate](https://github.com/weaviate/weaviate)
- ParadeDB Community is AGPL-3.0, and Enterprise is commercial. pg_search 0.25.6 is dated 2026-08-27 on PGXN, and Pigsty lists 0.26.1. Vector search relies on pgvector. — [paradedb/paradedb](https://github.com/paradedb/paradedb); [PGXN pg_search 0.25.6](https://www.pgxn.org/dist/pg_search/0.25.6/); [Pigsty pg_search](https://pigsty.io/ext/e/pg_search/)
- Neo4j Community Edition is GPLv3, and Enterprise adds closed-source components under a commercial license. Community is single-instance only: no clustering, online backup or RBAC. — [neo4j/neo4j](https://github.com/neo4j/neo4j); [shortlisted.tools review](https://shortlisted.tools/products/neo4j) (secondary). A claimed 34M-node cap on Community appears only in an unverified StackShare post — [StackShare](https://stackshare.io/posts/105391326131406437).
- Memgraph Community has been under the Business Source License since Memgraph 2.0 (2021). Enterprise is under the MEL license. — [sourcepulse summary](https://www.sourcepulse.org/projects/940524); [Memgraph Enterprise blog](https://memgraph.com/blog/try-memgraph-enterprise-30-days)
- PostgreSQL 19: Peter Eisentraut committed SQL/PGQ on 2026-03-16. It adds the `GRAPH_TABLE` function and `CREATE/ALTER/DROP PROPERTY GRAPH`, and property graphs behave like views over base tables. — [depesz: Waiting for PostgreSQL 19 – SQL/PGQ](https://www.depesz.com/?p=4467); [PostgreSQL 19 devel docs: CREATE PROPERTY GRAPH](https://www.postgresql.org/docs/19/sql-create-property-graph.html); [Karol Broda blog](https://karolbroda.com/blog/postgres-has-graphs-now)
- During the PG19 beta, final release was expected around September or October 2026. — [Mike Pultz blog](https://mikepultz.com/?p=1614)

### Inferences
- A server holding 1–5M vectors is comfortably within pgvector HNSW territory on a single machine. With int8 or halfvec, it is also within Qdrant's. Choosing Postgres plus pgvector keeps vectors, metadata, user-sync data and graph edges in one transactional store.
- For a self-hosted open-source project, AGPL (Elasticsearch, ParadeDB) is acceptable if the server is published as open source, but it complicates third-party hosted offerings. Apache, BSD or PostgreSQL-licensed options keep the most freedom.
- The linguistic search logic (roaring-bitmap feature index) should run identically on the server as a Rust service, or as the same core library called from the server. That avoids reimplementing morphology queries in an Elasticsearch or OpenSearch DSL.

### Gaps
- pgvector's latest version and date, Apache AGE release cadence, and Memgraph's latest release were not retrieved (GitHub API rate limit). Apache AGE maintenance health in 2025–2026 is **unverified**.
- Whether PostgreSQL 19 final has shipped as of 2026-10-27 was not confirmed.
- OpenSearch 3.x release status was not verified; the API returned the 2.19.x maintenance line.

## 5. Is a graph database warranted at this scale, versus relational edge tables plus recursive CTEs?

### Takeaway
No. OpenLogos's graph workloads (cross-references, citations, lexeme relations, syntax-tree parent/child) are shallow (1–3 hops), bounded, and need no graph algorithms at runtime. Practitioner consensus is that well-indexed edge tables with recursive CTEs handle this well. Graph DBs earn their place with unbounded-depth traversal or algorithm-centric workloads, and the operational cost of a second store is the bigger concern.

### Cited Findings
- Capital One engineering argues that graph-shaped data does not by itself require graph infrastructure, and that Postgres recursive CTEs handle multi-hop traversals. — [Capital One: Do I need a graph database?](https://www.capitalone.com/software/blog/graph-database-evaluation/)
- A practitioner comparison found most "graph" problems are 1–3 hop lookups. On a 200K-node graph, a 6-hop indexed recursive CTE ran in under 2 ms. The author argues that claimed 100x gaps usually compare a well-modeled graph against unindexed CTEs, and highlights the operational cost of a second database plus CDC sync. — [Markaicode: Neo4j vs PostgreSQL](https://markaicode.com/vs/neo4j-vs-postgres/)
- LatentView traded a graph DB for BigQuery CTEs. It frames bounded depth (about 3–5 hops), data already in the warehouse, and intolerance for syncing a second system as conditions under which SQL suffices. It notes that algorithms like PageRank still favor specialized engines. — [LatentView blog](https://www.latentview.com/blog/why-we-traded-a-graph-database-for-bigquery-ctes/)
- Graph DBs make sense for variable-depth traversals unknown at design time and for algorithms (PageRank, community detection, shortest path). — [techinterview.org LLD guide](https://www.techinterview.org/post/3233473957/lld-graph-database-design/?format=md) (secondary)
- PostgreSQL 19 adds SQL-standard property-graph querying (SQL/PGQ) over ordinary tables. — [PostgreSQL 19 docs](https://www.postgresql.org/docs/19/sql-create-property-graph.html)

### Inferences
- Model all relations as typed edge tables in SQLite, for example `edge(src_kind, src_id, rel, dst_kind, dst_id, weight, source_work)` with covering indexes on (src, rel) and (dst, rel). SQLite supports `WITH RECURSIVE`, so the same schema and queries run on device and on Postgres, with minor dialect differences.
- Cross-reference sets such as the Treasury of Scripture Knowledge (TSK), at roughly 340k–600k links, are small for SQLite (size estimate from domain knowledge; unverified). Lexeme networks (Louw–Nida domains, cognates) are also small.
- Graph algorithms such as citation centrality or clustering for "related passages" should be precomputed offline (in the build pipeline, with DuckDB, Python or Rust), with results stored as edge weights. No runtime graph engine is needed.
- If Cypher-style ergonomics are wanted on the server later, PG19 SQL/PGQ provides them on the same tables, with no AGE or Neo4j.

### Gaps
- No Bible-software-specific practitioner write-ups on graph versus relational were found.
- The CTE latency figures come from secondary blog benchmarks, not peer-reviewed sources.

## 6. Recommended minimal technology set (embedded + server, minimal duplicated logic)

### Takeaway
Use one data model and one Rust query core everywhere:
- **SQLite** (rusqlite natively; official sqlite-wasm with `opfs-sahpool` in the browser and Obsidian) as the canonical store.
- **FTS5 with a shared Rust normalization tokenizer** for keyword, phrase and Boolean search.
- A **custom roaring-bitmap token-feature index** (roaring-rs) for lemma, morphology, proximity and syntax search.
- **sqlite-vec** for on-device vectors, brute force first.
- **Edge tables plus recursive CTEs** for graph navigation.

On the server, use **Postgres + pgvector** (optionally ParadeDB pg_search for BM25, and PG19 SQL/PGQ for graph syntax), running the same Rust core for linguistic queries. Avoid Kuzu, CozoDB, sqlite-vss, instant-distance and sled. Keep Turso, LadybugDB and Tantivy-WASM on a watch list.

### Cited Findings
- SQLite's official WASM OPFS support (`opfs-sahpool` works without COOP/COEP on all major browsers since March 2023). — [SQLite WASM persistence](https://sqlite.org/wasm/doc/trunk/persistence.md)
- sqlite-vec was revived with stable 0.1.9 and ANN in alpha. — [sqlite-vec releases](https://github.com/asg017/sqlite-vec/releases)
- roaring-rs is active and pure Rust. — [roaring-rs](https://github.com/RoaringBitmap/roaring-rs)
- Turso is pre-1.0 with no listed Swift/Kotlin bindings. — [turso README](https://github.com/tursodatabase/turso)
- Kuzu was archived after the Apple acquisition. — [BetaKit](https://betakit.com/apple-strikes-deal-to-acquire-canadian-database-software-startup-kuzu/)
- DuckDB vss persistence is experimental. — [DuckDB vss docs](https://duckdb.org/docs/2.0/core_extensions/vss.html)
- PG19 SQL/PGQ was committed in March 2026. — [depesz](https://www.depesz.com/?p=4467)

### Inferences
- **Layering:**
  1. Corpus packs are immutable, read-only SQLite files per edition or work. Each contains verse/paragraph text, the FTS5 index, token feature columns, roaring bitmap BLOBs and an optional sqlite-vec table. They are built once in the pipeline (DuckDB or Rust) and downloaded or attached on demand with `ATTACH`.
  2. User data (notes, highlights, tags, reading plans) lives in a separate mutable SQLite DB. Sync it through a CRDT or op-log layer in the Rust core (for example Automerge, Yrs, or cr-sqlite-style changesets; not researched in this note) against the optional server, or against plain files for Obsidian.
  3. On the server, Postgres holds user sync, a global vector index (pgvector HNSW, 1–5M vectors) and the same edge tables. The Rust core is linked into the server process, or a Rust service reads the same corpus packs, so linguistic queries are not reimplemented.
- **Logic shared across all targets:** text normalization (Greek/Hebrew), query parsing, the token-feature query engine (roaring), ranking and fusion (BM25 plus vector reciprocal-rank fusion), and graph traversal queries.
- **Logic duplicated by necessity:** the FTS dialect (FTS5 on device versus tsvector or pg_search on server) and the vector index (sqlite-vec versus pgvector). To keep this thin, the server can also just run SQLite FTS5 over the same corpus packs, since corpus text is read-only. Postgres is then needed only for multi-user data and large-scale vectors.
- **Escalation path:** if on-device ANN is needed before sqlite-vec DiskANN stabilizes, embed USearch, storing its index as a file or BLOB. If server vector load grows beyond pgvector's comfort, add Qdrant (Apache-2.0). Do not add Elasticsearch or OpenSearch unless multi-tenant search at much larger scale appears.

### Gaps
- No end-to-end benchmarks exist for this exact stack on Greek/Hebrew corpora. The recommendations rest on component characteristics and scale estimates, not on measurements.
- Binary-size budgets per platform (sqlite-wasm + sqlite-vec + Rust core WASM) were not measured. A prototype build is needed.
- Sync technology choices (CRDT libraries, cr-sqlite status in 2026, PowerSync, ElectricSQL) were out of scope and not researched.
