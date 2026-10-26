# Phases and capacity

## Recommended organization

Fourteen role definitions do not mean fourteen continuously running agents. Start with an existing infrastructure agent plus product/architecture, reader design, client engineering, source stewardship and QA roles. Enrichment, security/sync and translation roles activate only for ready work. Maximum simultaneous coding runs: two initially, each in an isolated worktree. One independent review run may be added if resources permit.

Human project owner, source/language reviewers and security reviewer remain explicit outside the agent count. One person may fill several roles only with relevant competence; the author of a disputed claim should not be its sole approver.

## Delivery gates

| Phase | Outcome | Exit criteria |
|---|---|---|
| P0: feasibility | Nix skeleton, rendering/storage/rights spikes, import mapping | Selection/RTL on real devices, exact-text fixtures, verified source candidates, model benchmark |
| P1: reader | Offline mobile/web reader and portable annotations | Three reviewed text editions with declared coverage, three-pane layouts, first-class notes, export/import, no login/network dependency |
| P2: personal study | Portable library plus optional Obsidian, ink/margin mode and multi-chat research tracks | Separate library, vault, ink and research release gates; no private-content leakage |
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

## Revision 2 scope and sequencing

The earlier effort ranges describe the original base reader/library and do not include the expanded personal-study scope. Re-estimate after P0 provider/ink/vault spikes. Tentatively reserve an additional 6–12 engineering weeks across Obsidian, handwriting/margin mode and research, with substantial uncertainty from cross-platform integration and provider access. This is not a delivery commitment. Each P2 feature has its own gate so a provider limitation does not block ink or vault support.

Three-pane layout contracts and basic notes are P1 requirements. Subscription/provider feasibility and ink engine selection begin in P0. Obsidian/ink/research engineering runs on demand with the existing two-writer concurrency cap; adding role records does not create more simultaneous workers. P3 extends these features with optional private-host operation and encrypted attachment/chat synchronization.
