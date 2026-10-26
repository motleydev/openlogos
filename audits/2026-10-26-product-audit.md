# Product and UX audit — 26 October 2026

## Verdict

The initial blueprint has sound source-provenance, offline ownership and interoperability principles. Its main weaknesses were an underdefined audience, feature growth without a sufficiently compact interaction model, and gaps between promised workflows and the task graph. A three-pane reader with more attached tools is not by itself a better study experience.

The revised product centers Bible study and resource gathering for an audience of Reformed persuasion: read, compare, investigate, annotate, collect and hand off. Drafting happens elsewhere. Obsidian is the sole first-class external writing integration; ordinary exports serve other tools. Dedicated sermon drafting, delivery mode and slides are excluded. This reflects the owner's latest clarification and supersedes any sermon-writing interpretation of the earlier brief.

This is a specification and screenshot audit. No running OpenLogos app exists, and no Logos runtime was tested. Findings about clutter and interaction cost are design judgments from these views, not measured competitor performance or evidence that Logos cannot be configured differently. Closure below means the specification/task gap was addressed; no implementation or usability result is implied.

## Evidence and boundaries

Reviewed all 48 files' baseline inventory and the relevant product, workspace, architecture, data, rights, research, translation, agent and backlog contracts at commit `7690d325e12e00ed51fd89c9583361c62ed63f9f`. Re-ran the structural validator. Six user-supplied screenshots were visually inspected locally. The generic labels below identify the UI patterns reviewed; private filenames, image bytes, personal note contents, branding assets and Bible/lexicon text are not added to the public repo.

| Reference | Reference view | Observed pattern | Decision for OpenLogos |
|---|---|---|---|
| R1 | Notes beside a Bible | Notes filters, note list, editor and reading pane coexist with global navigation and multiple toolbars | Keep passage-linked annotations and useful filtering; collapse list/filter surfaces during study, and do not infer a need for a sermon word processor |
| R2 | Parallel reading | Two distinct passages, resource tabs, link controls and repeated pane menus | Keep independent/linkable passages, readable poetry and resource identity; consolidate global commands and make extra chrome optional |
| R3 | Layout picker | Small visual layout picker with several split arrangements | Keep discoverable layout selection and saved presets; add labels and keyboard/touch access rather than icon-only guesswork |
| R4 | Selection menu | Selected translation word exposes reference, original-language fields and many actions in adjacent menus | Keep occurrence-to-source access; use a short selection action row and a progressive Study drawer, with AI secondary |
| R5 | Word study | Lemma resources, translation distribution, occurrence list and grammatical groupings | Lead with contextual evidence and scoped occurrence tables; optional chart, advanced semantic roles only with cleared evaluated data |
| R6 | Search help | Search scopes, syntax help, operators and rich semantic filters | Preserve exact search capability; use visible scope and a query builder, and offer only filters supported by installed data |

The screenshots establish visible behavior, not underlying implementation, licensing, historical accuracy, data completeness or typical user preference. No screenshot totals, linguistic classifications or excerpted text are adopted as truth or seed data.

## Prioritized findings and dispositions

Severity indicates product/release consequence, not a software vulnerability rating.

| ID | Severity | Finding in revision 2 | Disposition and traceability |
|---|---|---|---|
| A01 | High | Audience was generic; multi-canon data support could be read as an all-faiths product mandate | Revision 4 specifies audience-driven resource/workflow priorities, initial Protestant library scope and accurate historical-source representation. B055 checks scope without imposing interpretation |
| A02 | High | Notes and research did not define a clear resource-gathering handoff; a sermon editor would add unwanted scope | Add lightweight study collections and provenance-preserving resource packets, with drafting elsewhere. B058 and specs/15; dedicated sermon editor/delivery explicitly excluded |
| A03 | High | Three-pane layout existed, but space budgets, control duplication and default navigation visibility were underspecified | New compact design guide, shared commands, closed drawers, focus/restore and comparative prototype requirement. B004/B014; actual visual design and usability remain open |
| A04 | High | Deterministic search was promised without a dedicated implementation task or query contract | B056 plus specs/17 define scope, phrases/Boolean queries, normalization, invalid syntax and offline expected-result tests; no AI dependency |
| A05 | High | Cross-reference provenance existed conceptually, but a usable early cross-reference feature had no explicit release task | B003/B057 require a cleared attributable baseline, declared coverage and non-destructive passage previews. Rich generated links remain later and separate |
| A06 | High | Obsidian was entirely a later optional track, despite its importance to the external writing workflow | B042/B043/B060 form a bounded P1 first-class handoff. Obsidian remains optional to install; fuller app-side editing and dynamic fences stay P2. External manuscript text is protected |
| A07 | High | First-release ancient reading and signed file exchange were listed in P1 product promises but P2 task records | B016/B017/B018 moved into P1 and reader gate dependencies updated; LAN exchange stays P2. Estimates explicitly require revision |
| A08 | High | Sophisticated word-study screens could imply data completeness that the project does not have | B031 and specs/16–17 require coverage, counting units, denominators and unknown states. No screenshot-derived lexicon, semantic-role data or alignment is imported |
| A09 | Medium | The audit initially overextended audience focus into an editorial stance and doctrinal ratification | Corrected in revision 4: remove implicit theological persona, runtime editorial profile and confessional ratification. Resource selection remains audience-aware; B051 checks that interpretation is not inferred from audience |
| A10 | High | Public visibility and an open-source goal coexist with no adopted first-party LICENSE | Unresolved release blocker recorded; B037 moved to P0 and feasibility gate. No license has been unilaterally selected or asserted over upstream works |
| A11 | Medium | Research adapters had a local runtime need before the P3 hosted companion | B049 now explicitly requires a working local development/desktop path. P3 remains NixOS/private-host/multi-user packaging, not a prerequisite for all P2 research |
| A12 | High | A passing blueprint validator could be mistaken for product, scholarly or security readiness | Validation report and PR distinguish structural passes from absent implementation, corpus review and usability. Original-language accuracy, rights, model behavior and crypto remain unverified |

## What to retain, simplify and defer

Retain three readable panes, independent reference links, source-aware selection, source text fidelity, notes/highlights, exact search, cross-reference previews, export/reimport and the immutable margin-snapshot concept. The existing separation of translation ancestry, token alignment, etymology and theme is a major strength and remains unchanged.

Simplify the shell rather than the evidence. One compact workspace command surface, single-row source identities, transient study/collection drawers and a reversible focus action should replace a permanent tool catalog and repeated ribbons. Capture should keep the passage on screen. The full notes/collections destination is for deliberate organization, not mandatory chrome around every reading session.

Use documented patterns from Obsidian's collapsible workspace, Zed's focus/dock actions and Zotero's nonduplicating collections as independent design references [S41–S43]. This is not a recommendation to reproduce those entire UIs or add integrations to all of them. B004 must compare prototypes with real tasks; no numerical superiority claim is made from documentation.

Defer semantic-role explorers, automatic thematic graphs, large-scale OCR enrichment and translation research until evidence and capacity permit. Keep optional research and ink behind their own gates. Exclude commercial storefronts, engagement feeds, broad religious-persona onboarding and sermon-authoring/delivery machinery. Detailed provider access, licensing and source selection remain implementation checks, not resolved by this UX audit.

## Acceptance scenarios

1. **Read and compare:** open three installed editions, link two and pin one; temporarily focus a pane and restore all positions; repeat with large text and on a phone without forcing three narrow columns.
2. **Investigate:** select a translated span with ambiguous/no alignment; inspect actual available data; preview a cross reference; return without moving the original passage. Missing data cannot be disguised by generated content.
3. **Collect:** add three source items and one personal observation to a named collection without opening a manuscript editor. Check duplicate handling, attribution, rights states and restart recovery.
4. **Hand off:** materialize real scripture blocks and a collection in Obsidian, link them from a user-authored note, edit/rename externally and export again. Confirm stable references, conflict handling and no manuscript overwrite. Run desktop and mobile/direct-versus-fallback cases separately.
5. **Search:** run known exact, phrase and Boolean queries with the model adapter disabled; change editorial preferences and verify the same counts within the same corpus scope.
6. **Respect the actual question:** ask a question without a theological frame, explicitly request a named author/confession perspective, and inspect conflicting sources. Verify that the audience alone never injects an interpretation. Human review evaluates evidence, attribution and the request, not doctrinal agreement.
7. **Margin capture:** freeze selected reading content, add ink, change the live layout and export/reopen the sheet. Strokes stay fixed; private observations and chats are excluded unless selected.

These are future acceptance scenarios. The audit has not run them against an application or an Obsidian plugin.

## Remaining decisions

The high-value next implementation evidence is a real-device selection/RTL prototype, two compact workspace prototypes tested with the intended audience, and a small genuine Obsidian block/collection round trip. Resource stewardship, adopted first-party licenses, cleared source artifacts, exact confessional editions, platform vault access and usable original-language alignments remain unresolved. P1 effort must be re-estimated after those spikes. The full historical graph and AI translation are not on the critical path to a useful reader and source collection tool.

## Audience clarification

The owner clarified that Reformed persuasion informs which resources and features matter, not what interpretation the tool should imply. Revision 4 removes the earlier interpretive charter/persona. Study, source collection and Obsidian handoff remain priorities; Sunday Mass, Afternoon Prayer and other service/prayer-office design workflows are out of scope. No confessional commitment is required to scaffold or use the tool.
