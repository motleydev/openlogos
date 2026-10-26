# Optional research workspace and user-owned model access

## Scope and priorities

An LLM interface for search and research is now a planned product component, not an excluded generic chatbot. It is optional at runtime and independent of the experimental Bible-translation pipeline. Reading, deterministic search, notes, Obsidian links and handwriting still work with no provider account.

User preference: offer supported subscription usage before asking for an API key. If no approved subscription route works for the user's installation, offer BYOK or a local model. Never silently switch a subscription request to billable API usage, another provider or a remote model. The development agents' model budget and the reader user's inference budget are separate.

## Provider strategy, checked 26 October 2026

| Route | Product decision |
|---|---|
| ChatGPT plan usage / Codex | Preferred feasibility path for eligible open-source/self-hosted usage through documented authorization. Account, platform and preview compatibility still need a real integration test. |
| Claude Code subscription | Conditional companion integration; no copied OAuth tokens or unofficial API emulation. Resolve the current documentation discrepancy and product/distribution conditions before enabling. |
| OpenAI/Anthropic API key | Supported BYOK path using official APIs and capability adapters. API billing is separate from consumer subscriptions. |
| Local model endpoint | Supported privacy/offline option where a model is available; credentials, capabilities and quality are explicitly configured. |

OpenAI documents an OSS plan-usage authorization flow and a Codex app-server integration [S23,S24]. The latter is a process/RPC route, not a library that runs inside an iOS browser. The documented preview requires streaming/stateless request handling and has tool limitations; use a tested capability matrix rather than assuming API-key feature parity [S25]. App-owned conversation storage is essential.

Claude's current product-integration page permits certain unmodified, user-authenticated Claude Code setups under stated terms [S32]. Its Agent SDK overview still states a restriction on third-party subscription login [S33]. Treat that discrepancy as unresolved for this application's exact embedding mode. The headless guide also says bare mode does not use subscription authentication [S34]. Therefore 'just wrap the CLI' is not a universal approved shortcut, and safe isolation must not depend on an incompatible flag. Prefer an explicitly supported end-user CLI/MCP workflow where appropriate; enable integrated subscription chat only after version-specific validation/clarification.

No authentication capability bypasses source-content rights or permits uploading licensed texts for a use not covered by their license. The ESV policy remains unchanged.

## Runtime architecture

`packages/llm-providers` defines normalized capabilities, streaming events, tool calls, cancellation, usage, errors and authentication references. Separate adapters handle subscription-authorized OpenAI requests/app-server, approved Claude runtime, ordinary APIs and local endpoints. Each advertises verified support for context size, image inputs, tools, structured output, resumption and search; absent capabilities are visibly disabled.

The default mobile/web strategy for long-lived credentials or CLI runtimes is an optional personal research companion, reachable through HTTPS on Tailscale/Headscale/plain WireGuard. It is a Nix-managed service or local desktop process. A locally hosted web UI can use it; there is no operator-run shared inference relay. A multi-user family host isolates each user's tokens, sessions and context. App identity and provider identity remain separate.

Native direct API access may be offered with OS secure credential storage and platform testing. Do not put keys in browser localStorage, IndexedDB, exported settings, vault Markdown or client bundles. A pure static web app cannot safely hide a long-lived provider key; prefer the user's companion. Direct OAuth on a platform is enabled only if the provider's documented flow supports it. Private companion URL support does not imply the inference itself is local.

Codex/Claude process adapters use isolated workspaces, typed RPC/structured subprocess arguments and allowlisted tools. No shell/exec or arbitrary file writes are enabled for Bible research. Retrieved books, notes, PDFs and vault folders must not become automatically loaded agent instructions, hooks or plugins. Supply evidence through explicit structured tools; do not launch a CLI with the user's vault as its working directory. Pin adapter/runtime versions and test what configuration is auto-loaded.

The inference host and chosen provider can see the context sent for generation. Ordinary E2EE note sync does not make server-side inference blind. Show that boundary before sending private notes or images. The companion gets a scoped, user-approved decrypted context bundle; it cannot mine encrypted notes automatically. Logs redact prompts, tokens and note bodies by default. Providers' retention settings are disclosed where known; no blanket zero-retention claim.

## Search and evidence flow

1. Resolve exact references locally; run lexical/full-text filters locally.
2. Optionally let the model propose a query plan or query expansion within a strict schema.
3. Run bounded tools against the selected library/notes sources. Local embeddings are optional indexes; remote embedding of private notes needs separate consent.
4. Retrieve exact source spans with edition/revision, locator, rights policy and content hash. Distinguish public catalog metadata from actual accessible text.
5. Generate an answer from the selected evidence. Quoted Bible text is resolved from the corpus, not invented by the model. Every citation ID must resolve to a retrieved span; quotation bytes are verified.
6. Let the reader open evidence beside the answer, inspect disagreements and save selected content to a note through a preview.

Citation validity is not proof that the cited material supports the inference. Evaluate support/entailment and theological/linguistic uncertainty separately. An unsupported claim stays labeled or omitted. Machine chat answers never enter the reviewed scholarly graph or translation corpus automatically.

Web research is per-chat opt-in with separately visible external sources and search-provider costs. A subscription connection does not guarantee native web-search entitlement. The adapter can use a separately configured search tool where authorized; otherwise explain that web research is unavailable. Default scope is selected local library, not the whole web or vault.

## Multiple chats and context ownership

Persistent `ResearchThread` records contain title, message branches/parent IDs, provider/account reference (never secret), chosen model, selected collections, immutable evidence bundles and tool-run history. Thread IDs are app-owned, not provider conversation IDs. Provider handles are optional adapter state. A reply records the actual provider/model and source context used. Switching provider previews any history sent to the new provider and never silently reuses credentials.

Users can create, name, search, duplicate/branch, archive, delete and export chats; resume after restart; interrupt generation; and save selected replies with citations to Markdown/Obsidian notes. Partial/cancelled answers are visibly incomplete. Concurrent chats cannot leak context into each other. Optional note-sync encryption covers chat history and evidence attachments, excluding provider secrets; public library sharing excludes all chats by default.

Context scopes: current selection, current passage, selected panes, chosen books/collections, individually selected notes, or selected vault folder. Each has an inclusion preview and exclusion controls. 'Include current panes' is a per-chat action, not a global background hook. Ink/snapshots require explicit image inclusion; handwriting recognition is not automatic. Retrieval queries themselves may disclose interests, so local-only mode prohibits external inference/search and explains unavailable features.

## Tools and permissions

Initial tool surface is read-only: resolve reference, retrieve passage, search approved corpus, inspect evidence, query reviewed linguistic data, retrieve selected note and list explicitly selected sources. Optional MCP exposes the same narrow surface to the user's own Codex/Claude client; MCP access is not itself a subscription entitlement.

Writing a note, overwriting a vault block, downloading a corpus, starting a costly OCR job or changing a workspace is a separate proposed action with scoped confirmation/authorization and an undo or recovery path. Imported content cannot request secrets, widen scope or invoke tools. No autonomous doctrinal adjudication, library-wide rewrite or subscription-funded unattended corpus backfill.

## Budgets and failure states

Show provider/account, subscription vs API vs local, context selection and estimated tokens before expensive research. Subscription quota is not an unlimited free API; only display quota figures the provider actually supplies. Set per-turn time/tool/retrieval limits and API spend ceilings. If output-token controls are unsupported on a subscription route, do not send invalid fields or claim an exact token cap; bound the task and cancel when required, disclosing that billing/usage can lag cancellation.

Handle revoked login, token renewal, account changes, quota exhaustion, blocked model/tool, unavailable companion, offline device and provider interruption without losing drafts. A retry never automatically duplicates a billable tool action. There is no silent paid fallback. Credentials are per-user, revocable and excluded from Nix store/backups by default except deliberate encrypted credential backup.

## Evaluation and release gate

Use at least 60 human-adjudicated cases spanning reference search, factual lookup, cross-source synthesis, conflicting interpretation, unknown answer and hostile-source prompt injection. Include original-language claims only where source coverage supports them. Measure retrieval recall, citation resolution, quotation fidelity, support of claims, abstention, privacy scope, cost and latency. False citations, cross-chat leakage and unauthorized uploads block release. Pin benchmark artifacts/rights and do not use model-generated labels as ground truth.

Deliver one supported subscription path when permitted/available, BYOK adapters and local endpoint support in independently testable increments. The research feature can release with clear unavailable subscription options while those integrations are unresolved; it must not imply broad Claude/Codex plan compatibility from successful developer CLI use alone.

## Audience-aware source selection and collection handoff

The audience informs useful library collections and study workflows, not the assistant's interpretation. Research follows the explicit question and retrieved evidence. Do not inject a Reformed persona, preferred doctrinal conclusion or confession-as-authority into system prompts, query expansion or answer ranking. A user may explicitly request a named author's or tradition's perspective; return an attributed account without silently adopting it as a persistent preference.

Record source scope, evidence and the user's request. Distinguish biblical quotation, attributed interpretation and generated synthesis. Relevant disagreement and uncertainty stay visible; an exhaustive all-traditions preamble is not required. No doctrinal preference may change source-language analysis or translation fidelity.

Selected replies can be collected with their machine-generated status and evidence preserved, then handed to Obsidian. Drafting and service planning are outside scope. Human-reviewed cases cover unrequested-perspective injection, an explicitly requested perspective, conflicting sources, confessional misattribution, false quotations and pressure to distort a translation. Agreement with the expected audience's theology is not a passing criterion.
