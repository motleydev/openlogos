# Phases and capacity

## Recommended organization

Fourteen role definitions do not mean fourteen continuously running agents. Start with an existing infrastructure agent plus product/architecture, reader design, client engineering, source stewardship and QA roles. Enrichment, security/sync and translation roles activate only for ready work. Maximum simultaneous coding runs: two initially, each in an isolated worktree. One independent review run may be added if resources permit.

Human project owner, source/language reviewers and security reviewer remain explicit outside the agent count. One person may fill several roles only with relevant competence; the author of a disputed claim should not be its sole approver.

## Delivery gates

| Phase | Outcome | Exit criteria |
|---|---|---|
| P0: feasibility | Nix skeleton, rendering/storage/rights spikes, import mapping | Selection/RTL on real devices, exact-text fixtures, verified source candidates, model benchmark |
| P1: reader | Offline mobile/web reader and portable annotations | Three reviewed text editions, compact three-pane workspaces, study notes/collections, local search/cross references, bounded Obsidian block/handoff path, export/import and no login/network dependency |
| P2: personal study | Foreground LAN exchange, fuller vault editing, ink/margin mode and multi-chat research tracks | Separate library, vault, ink and research release gates; no private-content leakage |
| P3: personal hosting | OIDC, optional encrypted sync, mesh and research-companion profiles | Fresh NixOS install, no-public-service mode, restore/recovery and security review |
| P4: enrichment | Lexical studies, citations, heritage and PDF review/backfill | Evidence-first UI, scoped gold sets, no fabricated genealogy, incremental reruns |
| P5: translation research | Human-reviewed pilot and transparent evaluation | Approved inputs, blind fidelity review, no unpermissioned modern text pipeline |

P3 can proceed independently of deep semantic enrichment after core schemas and identity boundaries are stable. P4 imported lexical data can be prototyped earlier, but it must not delay the reader. No phase requires immediate activation of every agent.

## Rough effort envelope, not a commitment

With one experienced human technical lead, agent assistance and timely reviewer access: P0 roughly 2–3 weeks; P1 6–10 additional weeks; P2 3–5; P3 4–8; P4 starts with a 6–12 week bounded corpus pilot and continues thereafter. Some work overlaps. App-store review, source licensing and native selection problems can extend the calendar. A defensible whole-Bible translation is a separate potentially multi-year editorial project; a pilot does not establish a completion date.

The principal cost is human review and engineering integration, not storage. Budget separately for developer/CI hardware (including a Mac), app distribution/signing, optional hosting/relay bandwidth, OCR/inference and language/security review. No price estimate is implied without measured pilot usage.

## Critical path

1. Source eligibility and renderer feasibility.
2. Edition/reference/annotation identity and package contract.
3. Exact text → parallel reader → durable highlights → export/reimport.
4. Package integrity and safe exchange.
5. Optional identity/E2EE sync and recovery.
6. Reviewed enrichment and translation research.

A failure of native text selection reopens the UI stack decision before building broad features. A lack of source permission replaces the candidate text; it does not block the reader architecture. A lack of qualified translation reviewers blocks the 'reviewed' designation, not publication of clearly labeled research tooling.

## Definition of the initial infrastructure handoff being done

The target repo exists with real lockfiles, minimal licensed fixtures, schema/tests/CI skeleton, isolated agent worktrees, installed-version Paperclip mapping and a dependency-aware initial backlog. P0 tasks are ready, later agents are paused, no unapproved large corpus has been fetched, and a short report names which checks actually ran. Production deployment and content publication have not been implied by successful scaffolding.

## Capacity for expanded personal study

The earlier effort ranges describe the original base reader/library and do not estimate the revised release scope. Re-estimate after provider/ink/vault spikes. Obsidian handoff, handwriting/margin mode and research each introduce platform-specific work and need separate estimates. Each later feature has its own gate so a provider limitation does not block ink or vault support.

Three-pane layout contracts, study notes/collections and the bounded Obsidian handoff are P1 requirements. Subscription/provider feasibility and ink engine selection can begin in P0 as non-blocking spikes. Obsidian/ink/research engineering runs on demand with the existing two-writer concurrency cap; adding role records does not create more simultaneous workers. P3 extends these features with optional private-host operation and encrypted attachment/chat synchronization.

## Revision 3 sequencing corrections

P1 includes one ancient work/local catalog (B016), signed package install/file exchange (B017/B018), deterministic search (B056), basic attributed cross references (B057), collections (B058), and a bounded Obsidian path (B042/B043/B060). This resolves the earlier mismatch between the product's first-release promise and its P2 task labels. LAN transfer and advanced enrichment remain later. B043 P1 covers materialized blocks; B044/B045 P2 cover fuller editing and dynamic rendering.

B055 checks audience-driven resource/workflow scope without a doctrinal ratification gate in P0; B059 introduces an edition-cleared confessional resource collection in P2. A sermon editor and teleprompter remain excluded; the bounded teaching view decided in D18-1 is P2 work (B069). No additional agent or concurrency slot is added. Earlier calendar estimates are superseded for this revised P1 scope: re-estimate after platform/vault and renderer spikes before making a release-date commitment. Each core track remains independently testable, while the complete P1 product gate requires them all.

## Revision 5 sequencing

P0 adds B061 (Rust core and bindings), B062 (platform capability tiers), B063 (Mac Mini agent/build host and signing lane) and B064 (embedded corpus engine and storage budgets) as gate inputs to B008. Apple platforms ship first, with iPadOS as the primary study and teaching device and macOS as the workstation. Web/Obsidian, then Android, follow the same conformance gate. Original-language search (B026/B065) and the optional semantic vector layer (B066) move into P2; alignment ingestion at scale (B067) stays P4. Calendar estimates remain superseded until the B005/B061/B064 spikes report measured results.
