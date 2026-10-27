# Blueprint revision 5.1 — 27 October 2026

- Decides D18-1: a bounded teaching view (large-text preaching mode) is in scope as B069; sermon editor, teleprompter and slides stay excluded (specs/18, 01, 09, 11).
- Accepts ADR 0002 as the starting point, with storage and relation engines behind core traits so the engine can change later.
- Defers Paperclip. A Claude Code session on the Mac mini orchestrates with audited subagents, small commits and owner-merged pull requests; records Mac mini status and adopts the sudoku-color sim/ship/shelf pattern (specs/20; B001, B002, B063 adjusted).
- Reviews CoreQuarry (re-Isearch lineage): not adopted as a dependency; PEER/XPEER/ANCESTOR structural operators and MAYBE/NARROW/PROMOTE/DEMOTE score shaping borrowed into the query AST and tool registry (specs/19, B065, S72).
- 69 tasks, 72 sources.

# Blueprint revision 5 — 27 October 2026

- Replaces the TypeScript/React Native baseline with a Rust core and native interfaces: SwiftUI/TextKit 2 for iPadOS, macOS and iOS; Kotlin + Compose; DOM web; Tauri desktop; Obsidian via WebAssembly (docs/adr/0001-rust-core-native-ui.md).
- Adds platform roles and capability tiers. iPadOS is the primary study and teaching surface and macOS the power workstation (ultra-wide, search workbench); phones are first-class companions. Enhancements degrade without data loss (specs/18-platform-capabilities.md). Records open decision D18-1 on a bounded iPad teaching view.
- Fixes the retrieval stack after a five-track research pass (reports/Bible corpus search architecture.md): SQLite everywhere, FTS5 with a shared Rust tokenizer, a Text-Fabric-style roaring-bitmap corpus engine, optional sqlite-vec semantic layers anchored on translations, and typed edge tables instead of a graph database. PostgreSQL + pgvector serve the optional server tier only (specs/19-search-and-retrieval.md, docs/adr/0002-embedded-retrieval-stack.md).
- Specifies the deterministic Rust build pipeline, token ID lineage, versification pivot through `org`, token-keyed standoff alignments, rights build gates, the AI tool registry shared with the UI, and the AI proposal lifecycle.
- Adds Paperclip-directed orchestration with a Mac Mini agent/build host, self-hosted CI and an isolated signing lane (specs/20-orchestration-and-build-hosts.md).
- Adds cross-resource links: work-level locator schemes (CTS-style), FRBR-style section maps between editions, a device-local link index for backlinks across packs, visible unresolved targets, library-relative counts, chain/cycle rules and user-note edges (specs/19, specs/03; task B068).
- Re-evaluates offline graph engines (PGlite + Apache AGE, LadybugDB, GraphQLite, Grafeo, Oxigraph and others). SQLite edges stay the default; GraphQLite then LadybugDB are escalations gated on benchmark R9. Corrects the server tier: SQL/PGQ was reverted from PostgreSQL 19.
- Updates role work scopes; 14 roles, 68 tasks (B061–B068 added; B026 moved to P2) and 71 source references.

# Blueprint revision 4 — 26 October 2026

- Clarifies that the Reformed audience determines resource and workflow priorities, not interpretation.
- Removes the implicit theological research persona, runtime editorial-profile contract and doctrinal ratification gate.
- Keeps attributed confessional/commentary resources as study material; selecting them does not imply endorsement or a required answer stance.
- Excludes service/liturgy and prayer-office planning; retains study, collection and first-class Obsidian handoff.
- Updates agents, model policy, backlog and audit consistently; 14 roles and 60 tasks remain.

# Blueprint revision 3 — 26 October 2026

- Audits six supplied Logos screenshots as workflow evidence; screenshots and private note contents are not committed.
- Defines a Reformed study/teaching audience and Protestant default canon while retaining accurate imported-source metadata.
- Centers resource gathering and first-class Obsidian handoff; excludes dedicated sermon drafting/delivery software.
- Adds compact UI guidance and external interaction references, deterministic search and attributed cross-reference contracts.
- Adds source collections, stable export receipts and explicit protection of external writing.
- Moves a bounded Obsidian path and previously misphased first-release promises into P1; fuller vault editing remains P2.
- Adds human editorial accountability without another agent: 14 inert roles, 60 dependency-linked tasks and 43 source references.
- Existing structural validation passes; implementation, real-device usability, corpus rights, license adoption and human review remain outstanding.

# Blueprint revision 2 — 26 October 2026

- Makes three parallel reading panes, independent reference-link groups, saved workspaces and a first-class notes view part of P1.
- Adds optional direct Obsidian vault integration, a plugin with real native scripture block embeds and conflict-safe note authority.
- Adds portable handwriting/highlighter support and immutable margin snapshots with editable overlays and PNG/PDF export requirements.
- Adds optional multi-chat search/research with supported subscription access preferred, BYOK/local alternatives, scoped evidence and explicit privacy/budget controls.
- Records the documented OpenAI OSS plan-usage route and the unresolved Claude Code/Agent SDK documentation discrepancy; no subscription adapter is declared implemented.
- Extends Nix/companion hosting and security requirements; all provider runtimes stay optional.
- Expands to 14 inert agent records, 54 dependency-linked task records and 36 primary-source entries.
- Adds schemas, synthetic fixtures and rejection tests for workspace/ink/research boundaries.

The repository is the ongoing source of truth. The earlier ZIP is revision 1 and does not contain these additions. This remains a specification handoff, not an implemented application or activated Paperclip company.
