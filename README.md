# OpenLogos — repository blueprint

Revision 5 — Rust core with native interfaces, platform roles, retrieval architecture and orchestration topology; updated 27 October 2026. Repository: https://github.com/motleydev/openlogos. OpenLogos is a working project name; no trademark clearance or affiliation with Logos/Faithlife is implied.

This is a **specification repository, not an implemented application**. It describes a quiet, fully open-source Bible study tool for a Reformed audience and an independently evolving scholarly data pipeline. It helps readers collect resources for teaching and preaching, with Obsidian as the sole first-class external writing integration. The infrastructure agent consumes its contents as inert project data. Nothing here authorizes creating agents, running tools, spending money, importing content, or deploying services by itself.

## Start here

1. Read `specs/00-import-contract.md` for the data/instruction boundary and a suggested external handoff message.
2. Read `specs/01-product.md`, `02-architecture.md`, and `03-data-model.md` for scope and the hard decisions.
3. Load `specs/agents.json`, `model-policy.json`, and `backlog.json` as records. They are project-defined formats, **not Paperclip API payloads**.
4. Validate with `python3 tools/validate.py` (or `nix-shell -p "python3.withPackages(ps:[ps.jsonschema])" --run "python3 tools/validate.py"`). This validates this blueprint's structure; it does not test an app or a deployment.
5. Resolve the installed Paperclip version and adapter schemas, produce a configuration diff, then provision within the external task's authority.

## Getting started

The implementation skeleton (B002) follows the layout in `specs/06-nix-operations.md`: a Rust workspace under `crates/`, platform apps under `apps/`, and Nix shells for every area. One prerequisite replaces a page of setup:

1. Install [Nix](https://nixos.org/download) with flakes enabled, and [direnv](https://direnv.net) (ideally with nix-direnv).
2. Clone, `cd` in and run `direnv allow`. The default shell is `.#core`; `nix develop .#web`, `.#android`, `.#apple` (macOS only), `.#pipeline` and `.#ops` serve the other areas. The first entry downloads the pinned Rust toolchain.
3. `just doctor` checks the machine and says how to fix anything missing; `just setup` installs the git hooks.
4. `just test` runs the workspace tests; `just` lists every workflow; `just ci` is what CI runs in the shell and `just check` (`nix flake check`) runs the same checks hermetically.
5. On a Mac with Xcode (the Mac mini lanes, B063): `just sim` builds the app shell for an iPad simulator with no signing and launches it, `just sim-shot` takes a screenshot; `just ship ios` makes a signed ad hoc build and puts it on a shelf served over Tailscale, after `cp .envrc.local.example .envrc.local` (team ID, optional App Store Connect key, shelf settings) and `just shelf-install` once. `just ship-check` lists what is still missing. Both scripts enter `nix develop .#apple` by themselves.

`crates/README.md` and `apps/*/README.md` name each area's commands. Xcode and the Android SDK are external prerequisites that the shells verify rather than provide; `apps/apple/README.md` and `apps/android/README.md` say what to install. `docs/evidence/` records the commands that were run on real machines and their results.

## Recommended shape

- One Rust core for references, versification, anchors, packages, search and retrieval, exposed through UniFFI to native apps and WebAssembly to the web and Obsidian (docs/adr/0001-rust-core-native-ui.md).
- Native interfaces: one SwiftUI/TextKit 2 app for iPadOS (primary study and teaching device), macOS and iOS; Kotlin + Compose on Android; a DOM web reader; Tauri for Windows/Linux. Platforms add their own strengths, such as Pencil and ultra-wide layouts, without forking data (specs/18-platform-capabilities.md).
- Prebuilt, signed, content-addressed library packages carrying text, search indexes and optional vector layers; separately versioned enrichment overlays. No login or AI required to read or search (specs/19-search-and-retrieval.md).
- Optional self-hosted Rust services over PostgreSQL, Keycloak OIDC, static hosting and Headscale/DERP. Direct WireGuard and file exchange are first-class alternatives.
- Deterministic Rust build pipeline; Python only for ML/OCR/evaluation experiments. No mandatory search cluster, vector service, Kubernetes or paid cloud prerequisite.
- Nix development shells, build inputs, CI and NixOS services, with the necessary Apple/Xcode/signing exception made explicit. Paperclip directs agent work; a Mac Mini is the agent and Apple build host (specs/20-orchestration-and-build-hosts.md).

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
| specs/20-orchestration-and-build-hosts.md | Paperclip direction, Mac Mini build host and signing lane |
| docs/adr/ | Architecture decision records |
| reports/Bible corpus search architecture.md | Revision 5 retrieval research report; notes in research_notes/ |
| audits/2026-10-26-product-audit.md | Screenshot audit, findings, dispositions and remaining risks |
| specs/research-provider-policy.json | Inert product-provider preferences and integration gates |
| specs/sources.json | Machine-readable research inventory |
| specs/agents.json | 14 role records, descriptions and activation rules |
| specs/model-policy.json | Capability tiers, routing, evaluation and cost controls |
| specs/backlog.json | Dependency-ordered tasks with acceptance criteria |
| schemas/*.json | Machine-readable blueprint and package/claim contracts |
| examples/*.json | Synthetic records, never production data |
| tools/validate.py | Offline structural, dependency and safety validation |

## Assumptions

English UI and translation research first; German UI/reader coverage included early. Initial library navigation uses the 66-book Protestant canon, and acquisition prioritizes resources useful to the Reformed study audience. These are scope choices, not an interpretive stance or AI persona. Data schemas preserve imported works’ native canons and numbering for accurate historical study; this is not an all-faiths product mandate. There is no chosen production budget, publisher permission, scholarly review team, app-store account, domain, or installed Paperclip version in this brief. Human language reviewers and security review are required release resources, not roles an LLM can impersonate.

See `specs/10-sources.md` for verified primary sources. All version-sensitive selections must be pinned and rechecked during implementation. The numerical performance and schedule figures are proposed targets and planning estimates, not measured results.
