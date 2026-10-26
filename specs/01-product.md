# Product and reading experience

## Product promise

Study Scripture faithfully, gather sources, and carry the work into Obsidian or a portable export. Open the book and read. Study tools appear when requested. No store, app subscriptions, promoted books, advertisements, engagement feed, compulsory account, or mandatory AI. Full source, build instructions, data formats and exports remain public. Users can replace hosted services and leave with their notes.

The inspiration is Logos's useful workflows, alongside AndBible, Ezra Bible App, STEP and SWORD—not a binary-compatible clone. Evaluate publicly visible behavior and appropriately licensed source. No Logos assets, proprietary library dumps, DRM removal, or copied branding are required.

The primary audience studies and prepares to teach/preach from a Reformed perspective. The default Bible scope is the 66-book Protestant canon. See specs/15-reformed-study-and-collections.md for the editorial charter and the boundary between source evidence and interpretation. Sermon drafting takes place elsewhere; first-party notes support study, not a dedicated sermon editor.

## First-release workflows

| Workflow | Definition of success |
|---|---|
| Read | Open at last location offline; book/chapter navigation; paragraph/poetry layout; footnotes on demand |
| Compare | Three independently linkable/pinnable reading panes on suitable screens; saved layouts; preserved tabs/stacked views on phones |
| Highlight | Exact arbitrary text ranges including across verses; multiple colors with accessible labels; undo |
| Note | Study notes and optional dock/full view; Markdown, tags, multi-passage links, offline search and revisions; no persistent note editor required while reading |
| Find | Offline reference/word/phrase/Boolean search with visible edition and range scope |
| Connect | Attributed cross references with honest coverage and previews that preserve the reading position |
| Collect | Ordered source items and observations with exact provenance; capture without leaving the passage |
| Obsidian handoff | P1 tested plugin, genuine scripture blocks and collection transfer; preserve external writing |
| Leave | Export Markdown plus versioned JSON with anchors, tags, timestamps and revision history; reimport without loss |
| Discover | Search a bundled/local catalog by author, era, language, genre, tradition and rights; preview metadata before download |
| Ancient reading | One vetted ancient work with provenance, divisions, translator/edition metadata and linked references |
| Move library | Import/export a signed package by OS file picker/share sheet with no identity server |

Parallel reading panes must not force every verse onto an equal-height row in normal reading mode. Offer a study alignment mode separately. Anchor synchronized scrolling to passage mappings, not pixels. Missing/extra verses, merged verses, Psalm numbering, superscriptions and deuterocanonical material receive explicit markers. Scrolling never invents a correspondence.

## Reader requirements

- Typography: bundled open-license fonts, adjustable size/leading/margins, sensible measure, light/dark/sepia palettes, optional verse numbers and red-letter rendering only when source markup supports it. No ornamental panel chrome.
- Hebrew RTL mixed with English/Greek, vowel/cantillation marks, Greek diacritics, poetry, tables and footnote navigation survive font scaling and copy/paste.
- VoiceOver, TalkBack, keyboard and browser screen-reader traversal are designed alongside the text renderer. Highlight meaning never depends on color alone. Target WCAG 2.2 AA web behavior and comparable native usability.
- Selection uses platform selection affordances, arbitrary ranges, accurate handles, and a small context menu. Native text layout is a spike gate; an embedded renderer is an alternative only if measured accessibility and selection are better.
- Study drawer progression: translation footnote → cross references → lemma/morphology → lexical senses and witnesses → scholarly evidence. No graph clutter by default.
- A selected English word can align with zero, one or several original-language tokens. An unavailable alignment is displayed as unavailable; Strong's IDs alone cannot manufacture it.
- Library discovery works from imported catalog snapshots. Remote catalogs, cover images, fonts, telemetry, AI calls and analytics are not required or silently fetched.
- Private notes are separate from shareable content. Share/export preview makes included text and notes visible before handoff.

## Proposed measured budgets

Test a pinned corpus of three approved Bible/source-language editions with declared coverage, one ancient work, 10,000 annotations and a 100 MB package on a named midrange Android device, a named supported iPhone, and current pinned Safari/Firefox/Chromium builds. Record OS, hardware, release build, warm/cold conditions and at least 30 runs.

Targets: p95 cold open to usable last passage ≤2 seconds; warm chapter switch ≤150 ms; local reference jump ≤100 ms; keyword search ≤300 ms on the declared three-edition fixture; highlight commit visibly acknowledged ≤100 ms. Scroll target is 60 fps on those devices without selection jumps. Instrument long tasks and memory rather than declaring success from mean frame rate. Large PDFs and full historical corpora have separate budgets.

User studies: five to eight readers, including at least one screen-reader user and one Hebrew reader, complete open/compare/highlight/export flows. No lost annotations, incorrect mappings, or inaccessible primary action is acceptable. Speed targets may be revised from evidence; text correctness may not.

## Deliberately deferred

Dedicated sermon editor, delivery/teleprompter mode, slides, sermon generation, subscription marketplace, social network, collaboration editing, audio production, automatic reading streaks, complete Logos import parity, universal OCR, unreviewed theology ontology, autonomous whole-Bible translation, and remote always-on mobile seeding are outside the MVP.

German texts and interface are a high-priority follow-on; rights to modern Luther/Elberfelder revisions must be checked per edition. Imported works retain their own canon/versification metadata for accurate scholarship; the default reader/search canon is Protestant and launch coverage is explicit.

## Added planned feature tracks

Three-pane layouts, study notes/collections, deterministic search, attributed cross references and a bounded first-class Obsidian handoff belong to P1. Obsidian remains optional to install. P2 adds fuller bidirectional vault editing, open handwriting tools, fixed-layout margin snapshots and evidence-linked multi-chat research. Provider access prefers a verified supported subscription path; BYOK and local inference remain alternatives. These features are now in scope, while no AI is required for ordinary reading/search. See specs/12-workspace-ux.md through specs/14-research-assistant.md for the detailed contracts.

The design guide in specs/16-design-guide.md supersedes any assumption that a permanent rail, note list or repeated pane toolbar is desirable. Prototype compact shared controls and focus mode before freezing navigation. Success is source inspection and a reliable resource handoff, not time spent drafting inside the app.
