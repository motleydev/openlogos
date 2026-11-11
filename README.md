# OpenLogos

A quiet, fully open-source Bible study tool for a Reformed audience: one Rust core, native interfaces over it, and an independently evolving scholarly data pipeline. It helps readers collect resources for teaching and preaching, with Obsidian as the sole first-class external writing integration. Repository: https://github.com/motleydev/openlogos. OpenLogos is a working project name; no trademark clearance or affiliation with Logos/Faithlife is implied.

The repository began as a blueprint (specs/, revision 5.1 of 27 October 2026) and now holds the implementation that follows it. The specs remain the source of truth for scope and decisions; `specs/backlog-status.json` records which backlog tasks are done, and `docs/evidence/` holds the measurements and screenshots behind each.

## What exists today

| Area | State |
|---|---|
| Rust core (`crates/`) | References and citation parsing, versification and canon tables, anchors, the reading stream, the workspace shell (sessions, numbered columns of tabs, command registry, palette, contextual tool well, context menu, user store and restore), notes authority, ink geometry, and the embedded retrieval engine on SQLite (FTS5 with a shared Greek and Hebrew tokenizer, a columnar token engine, the device link index). |
| Bindings (`crates/ffi`) | Swift and Kotlin through UniFFI, the browser through wasm-bindgen. One conformance suite runs against the core and every binding. |
| Mac app (`apps/apple`) | The workspace shell of specs/22: palette field, columns of tabs, tool well, context menu and continuous reading. An iPad build runs in the simulator from the earlier reading prototypes; the shell on iPad is B075. |
| Web (`apps/web`) | The same shell as plain ES modules over the WebAssembly core, with the user store and session restore, kept in lockstep with the Mac app: the core decides behaviour and every host action is a registered core command. |
| Android (`apps/android`) | The Kotlin binding and its JVM test host only. |
| Not started | Windows and Linux desktop shell, services, the production ingestion pipeline, sync between devices (the protocol crate `crates/sync` exists and is not yet used by an app), the research assistant, the Obsidian plugin. |

`main` reads the core's small fixture texts. Full editions come from locally built packs: importers refuse any source whose rights record (`rights/`) the source steward has not approved, and no corpus bytes are committed.

## Getting started

1. Install [Nix](https://nixos.org/download) with flakes enabled, and [direnv](https://direnv.net) (ideally with nix-direnv).
2. Clone, `cd` in and run `direnv allow`. The default shell is `.#core`; `nix develop .#web`, `.#android`, `.#apple` (macOS only), `.#pipeline` and `.#ops` serve the other areas.
3. `just doctor` checks the machine and says how to fix anything missing; `just setup` installs the git hooks.
4. `just` lists every workflow. The ones used daily:

| Command | What it does |
|---|---|
| `just test` | the Rust workspace's tests |
| `just ci` | formatting, clippy, tests and the wasm type-check: what GitHub runs in the dev shell |
| `just check` | `nix flake check`, the same checks through the flake |
| `just audit` | dependency advisories, licences and sources (`deny.toml`) |
| `just ci-hosts` | every host lane GitHub does not run: wasm and web shell, Kotlin, Swift, the Mac app's tests |
| `just preview-web`, `just preview-mac` | serve the web shell on 127.0.0.1:8780, or build and open the Mac app, for hand review |
| `just sim` | build the app for an iPad simulator with no signing and launch it |

`crates/README.md` and `apps/*/README.md` name each area's commands. Xcode and the Android SDK are external prerequisites that the shells verify rather than provide (specs/06-nix-operations.md). The lockfiles (`flake.lock`, `Cargo.lock`) are committed and are the source of truth for versions.

## How work is done

A Claude Code session on the Mac mini orchestrates implementation with audited subagents and small commits; every change reaches `main` through a pull request the owner merges (specs/20-orchestration-and-build-hosts.md, "Interim operation"). Paperclip, which the blueprint first named as the orchestrator, is deferred; `specs/agents.json`, `specs/model-policy.json` and `specs/backlog.json` remain inert project records, **not Paperclip API payloads**, and nothing in this repository by itself authorizes creating agents, spending money, importing content or deploying services (specs/00-import-contract.md). `python3 tools/validate.py` (inside any dev shell; `just validate`) checks the records, schemas, examples and rights records offline.

## Recommended shape

- One Rust core for references, versification, anchors, packages, search and retrieval, exposed through UniFFI to native apps and WebAssembly to the web and Obsidian (docs/adr/0001-rust-core-native-ui.md).
- Native interfaces: one SwiftUI/TextKit 2 app for iPadOS (primary study and teaching device), macOS and iOS; Kotlin + Compose on Android; a DOM web reader; Tauri for Windows/Linux. Platforms add their own strengths, such as Pencil and ultra-wide layouts, without forking data (specs/18-platform-capabilities.md).
- Prebuilt, signed, content-addressed library packages carrying text, search indexes and optional vector layers; separately versioned enrichment overlays. No login or AI required to read or search (specs/19-search-and-retrieval.md).
- Optional self-hosted Rust services over PostgreSQL, Keycloak OIDC, static hosting and Headscale/DERP. Direct WireGuard and file exchange are first-class alternatives.
- Deterministic Rust build pipeline; Python only for ML/OCR/evaluation experiments. No mandatory search cluster, vector service, Kubernetes or paid cloud prerequisite.
- Nix development shells, build inputs, CI and NixOS services, with the necessary Apple/Xcode/signing exception made explicit. A Mac Mini is the agent and Apple build host (specs/20-orchestration-and-build-hosts.md).

## What ships first

A reader that opens a reviewed Bible offline, supports three parallel reading panes and first-class study notes and collections, preserves exact text, highlights reliably, exports and reimports notes, and reads one reviewed ancient work. Local search and attributed cross references are core requirements. A signed package can be copied between devices. A bounded Obsidian plugin/block-reference and resource-handoff path belongs to P1; fuller vault editing, handwriting/margin snapshots and optional multi-chat research follow separately. Sermon drafting and delivery remain outside the product. Supported subscription access is preferred, with BYOK/local-model alternatives. Private sync, rich language studies, broad PDF ingestion and an AI-assisted translation follow separate gates.

## Non-negotiable distinctions

**Original-language text is an edition or witness, not an available autograph.** Hebrew, Aramaic and Greek all matter. Translation genealogy, textual witnesses, word alignment, etymology and thematic connections are different kinds of evidence. A model's plausible connection is not a documented historical relationship.

**Ancient does not mean the digital edition is unrestricted.** The source registry distinguishes underlying works, editions, translations, scans and annotation layers. ESV is not part of the default open corpus or AI translation inputs. No permission is assumed from API access.

**Peer-to-peer is not anonymity or guaranteed connectivity.** Local exchange can work without the internet; remote NAT traversal may need a relay, which someone operates. Encrypted transport does not protect an unlocked or compromised endpoint.

## Contents

| File | Purpose |
|---|---|
| specs/00-import-contract.md | Safe consumption, Paperclip mapping, external handoff |
| specs/01-product.md | Reader UX, MVP, exclusions, benchmark targets |
| specs/02-architecture.md | Stack, boundaries, self-hosted and offline modes |
| specs/03-data-model.md | Identity, versification, graph, provenance, annotations |
| specs/04-ingestion.md | Corpus/PDF pipelines, review, backfill and publication |
| specs/05-sharing-security.md | Transfer protocol, identity, E2EE, limits |
| specs/06-nix-operations.md | Desired repo layout, environments and deployment gates |
| specs/07-translation-lab.md | AI-assisted translation research and human review |
| specs/08-quality-governance.md | Accuracy, licensing, release and open-source governance |
| specs/09-roadmap.md | Phases, resource assumptions and critical path |
| specs/10-sources.md | Primary-source research and reuse candidates |
| specs/11-open-decisions.md | Evidence needed before committing major choices |
| specs/12-workspace-ux.md | Three-pane layouts, first-class notes and research interactions |
| specs/13-obsidian-and-ink.md | Vault integration, scripture blocks, handwriting and margin mode |
| specs/14-research-assistant.md | BYOK/subscription/local-model research and multiple chats |
| specs/15-reformed-study-and-collections.md | Audience, resource priorities, source collections and Obsidian handoff |
| specs/16-design-guide.md | Compact chrome, progressive study controls and usability gates |
| specs/17-search-and-cross-references.md | Deterministic queries and attributed passage links |
| specs/18-platform-capabilities.md | Platform roles, capability tiers and lossless degradation |
| specs/19-search-and-retrieval.md | Corpus query engine, full text, vectors, relations and pipelines |
| specs/20-orchestration-and-build-hosts.md | Orchestration (Paperclip deferred), Mac Mini build host and signing lane |
| specs/21-reading-interactions.md | Panes, drawers, capture and handoff interactions |
| specs/22-workspace-shell.md | Palette-first shell, columns of tabs, tool well, context menu, speed budgets |
| specs/23-continuous-reading.md | Continuous reading stream and the text pack format |
| specs/24-persistence.md | User store, parallel workspaces and session restore |
| docs/adr/ | Architecture decision records |
| reports/Bible corpus search architecture.md | Revision 5 retrieval research report; notes in research_notes/ |
| audits/2026-10-26-product-audit.md | Screenshot audit, findings, dispositions and remaining risks |
| specs/research-provider-policy.json | Inert product-provider preferences and integration gates |
| specs/sources.json | Machine-readable research inventory |
| specs/agents.json | 14 role records, descriptions and activation rules |
| specs/model-policy.json | Capability tiers, routing, evaluation and cost controls |
| specs/backlog.json | Dependency-ordered tasks with acceptance criteria |
| specs/backlog-status.json | Which tasks are done, partly done or in review, with their evidence |
| rights/*.json | One rights record per source; the steward's approval gates every import |
| docs/evidence/ | Measurements and screenshots per task |
| schemas/*.json | Machine-readable blueprint and package/claim contracts |
| examples/*.json | Synthetic records, never production data |
| tools/validate.py | Offline structural, dependency and safety validation |

## Assumptions

English UI and translation research first; German UI/reader coverage included early. Initial library navigation uses the 66-book Protestant canon, and acquisition prioritizes resources useful to the Reformed study audience. These are scope choices, not an interpretive stance or AI persona. Data schemas preserve imported works’ native canons and numbering for accurate historical study; this is not an all-faiths product mandate. There is no chosen production budget, publisher permission, scholarly review team, app-store account, domain, or installed Paperclip version in this brief. Human language reviewers and security review are required release resources, not roles an LLM can impersonate.

See `specs/10-sources.md` for verified primary sources. All version-sensitive selections must be pinned and rechecked during implementation. The schedule figures are planning estimates. Performance budgets are targets; `docs/evidence/` holds what has been measured against them.
