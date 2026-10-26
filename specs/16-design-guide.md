# Reader and study design guide

This is a proposed interaction and visual baseline, informed by the six user-supplied Logos screenshots and the product's Reformed study/preaching audience. It is not a finished visual identity or evidence of usability. Prototype values below must be tested with real readers. The screenshots are private review references, not redistributable assets or implementation templates.

## Hierarchy and navigation

The text is the primary surface. Default launch restores the last reading workspace. Start with no permanent navigation rail. A visible workspace/menu control exposes Read, Search, Library, Notes and Collections; a collapsible sidebar is available when needed. Research is an optional pane/tool. A command launcher provides a fast keyboard route to the same visible actions. Layouts are available beside the workspace name. There is no dashboard, store, news feed or permanent list of every possible tool.

Use one compact shared workspace/passage bar and one single-row identity header per pane. Global navigation, layout and appearance commands appear once; each pane retains only the resource, local passage and explicit link/pin state needed to prevent ambiguity. A reading header exposes resource identity, local passage, link/pin state and an overflow menu; it does not duplicate the shared command bar. Typography, resource history and less frequent commands belong in that menu or a drawer. Do not repeat a word-processor ribbon across reading panes. Notes show formatting controls when editing; the note index and filters can collapse independently of the editor.

Prototype budget at normal text size: a 40–48 CSS-pixel shared bar and a 32–40 pixel identity row on pointer-oriented desktop layouts, with larger touch controls and scalable heights on mobile. These are design targets, not clipping constraints at large text sizes. Content-area chrome should fit within roughly 88 pixels vertically without stacked ribbons. At 1440 × 900 CSS pixels, demonstrate three readable panes with a compact collection drawer or clearly explain which pane the dock replaces. At 1024, 768 and 390 pixel widths, preserve content/state while adapting the visible pane count. An always-visible fourth column is not a requirement.

## Visual baseline

| Element | Starting direction | Acceptance |
|---|---|---|
| Reading text | Open-license book serif; web default around 20 px with adjustable 1.45–1.65 leading | No clipped Hebrew/Greek marks; no font substitutions that lose glyphs; native dynamic type supported |
| Prose measure | Aim for approximately 55–75 Latin characters per line in a focused pane | Multi-pane views may be shorter; do not shrink text to force a target measure |
| UI and notes | Restrained open-license sans serif; web UI starting around 14–16 px, body notes 16–18 px | Readable at scaling; secondary text remains legible |
| Color | Neutral ink/paper surfaces, one restrained accent; light/dark and optional warm paper | Validate every state including selections, links and disabled controls; no marketing gradients |
| Spacing | 4/8/12/16/24/32 pixel token scale | Padding adapts with text; full-width cramped lists are avoided |
| Borders and elevation | Fine separators; shadows only for temporary overlays | Pane boundaries remain visible in all themes |
| Active pane | Labeled header/focus treatment plus a subtle accent | Color alone never identifies the active pane or a link group |
| Highlights | Small user-named palette; underline/annotation markers available | Legible over text, in dark mode and in grayscale export |
| Verse/footnote marks | Subordinate scale and baseline treatment appropriate to source markup | Adequate interactive target and screen-reader labels without making the paragraph look like a link cloud |

Exact font families, color tokens and native sizes are B004 prototype outputs. Measure contrast before freezing them. Use WCAG 2.2 AA for web, including text contrast, keyboard operation, reflow, focus visibility and target sizing [S40]. Product touch targets should aim for at least 44 logical units where feasible; this is a product target, distinct from WCAG's 24 CSS-pixel minimum with specified exceptions. Test web text at 200% and reflow at 320 CSS-pixel width. A multi-pane arrangement is not an excuse to prevent ordinary text from reflowing. Native controls follow platform accessibility behavior; matching pixels across platforms is not the goal.

Red-letter rendering is opt-in and only uses supported edition markup. Default Scripture text is monochrome. Poetry, paragraph structure and source footnotes are preserved. The screenshots' visual density is an observation about those views, not a claim that Logos lacks configurable typography or accessibility settings.

## Selection and contextual study

Primary selection actions are Highlight, Collect and Study alongside native Copy and selection actions. A short personal observation is available within capture or More; opening a full editor is not required. Additional actions live under More. On systems with native selection menus, preserve standard platform actions and put project actions in an accessible extension. Do not disable the OS menu to force a custom one.

Study opens a drawer, not a sprawling cascading menu. Its header states what was selected: translation wording, reference and edition. Where licensed alignment exists it offers the original occurrence(s), lemma, transliteration if requested, and a plain-language morphology summary. A translated span may align to several tokens or none; show that explicitly. Strong's identifiers are optional reference fields, not the word's identity or its definition.

The drawer progresses through Context, Cross references, Word and Sources. A user can pin it as a pane. Technical morphology codes and lexical-domain IDs expand on demand. Inline hover is an enhancement, never the only interaction; touch and keyboard can reach the same information. Closing a popover restores focus and leaves selection and scroll location intact.

AI assistance is a secondary explicit action with visible context scope. A source-derived gloss and a generated explanation do not share an unlabeled result slot. No model call is needed to open a word study backed by installed data.

## Word-study presentation

Lead with the selected occurrence and its immediate context, candidate analysis and cited senses. Follow with a compact table of renderings and an occurrence list. Show source edition, corpus/book scope, mapped-token coverage, counting unit and excluded/unknown cases alongside totals. Count tokens, not just verses, and explain repeated tokens or multiword alignments. No chart is shown if coverage is unknown; counts do not imply lexical meaning or theological importance.

The screenshot's donut chart demonstrates a distribution view, not a distribution to reproduce. Our optional bar visualization must use the same underlying scoped counts as the accessible table and support navigation to the occurrences. Do not infer screenshot totals as facts, reproduce its lexical content, or compute English rendering counts from a Strong's lookup alone. Advanced subject/object/clause-role analysis is deferred until a cleared, evaluated dataset exists; a polished screen cannot supply missing linguistic data.

## Search and cross-reference presentation

One search field handles passage references and words. Show the chosen edition/collection and result type. Advanced filters have labeled controls and a readable query summary; expert syntax remains available as a secondary route. Start with a few useful examples rather than a page-length syntax manual. Deterministic search and AI research remain visibly different operations, with no silent query rewrite. See specs/17-search-and-cross-references.md.

Cross references open in a transient passage preview with open-in-pane and collect actions. A preview has its own local navigation so examining references does not move the main reading pane. Each relation exposes its source and kind; a themed connection is not displayed as a textual quotation merely because it is useful for a sermon.

## Recovery and states

Every major interaction has designed loading, empty, offline, unavailable-source, invalid-query, read-only and conflict states. Unsaved work stays visible during errors. Undo restores the prior annotation or layout operation where feasible. A pane closed accidentally can be restored with its location; replacing a resource never overwrites a note.

Margin mode freezes the text with a capture preview and keeps tool chrome off the exported sheet. A collection export remains structured sources and observations with stable links, not a screenshot or generated sermon. App export never owns the user's external writing note.

## Prototype and usability gate

B004 produces annotated prototypes for Read, Three texts with closed drawers, Three texts + collection, Selection/word study, Search results, Margin capture and Obsidian handoff. Every view includes mobile and keyboard/large-type behavior and at least one missing-data state. No claim of a finished design system is made until these are reviewed.

B014 sessions include five to eight intended readers with overlapping roles: at least two people who prepare sermons or Bible teaching from a Reformed perspective, a screen-reader user and a Hebrew reader. Test a 20-minute passage-to-resource-handoff task plus core reading/export tasks. Record completion, mistaken navigation, source confusion, hidden controls, lost context and data loss; do not invent scores from screenshot inspection. A person should be able to explain which edition they are reading, which pane is linked, and whether a statement is Scripture, a cited interpretation or AI output.

## Patterns to evaluate beyond Bible software

| Reference | Useful documented pattern | Independent OpenLogos application |
|---|---|---|
| Obsidian workspace/ribbon [S41] | Collapsible sidebars, saved arrangements and a hideable command ribbon | Preserve a study arrangement while letting navigation disappear; do not copy all of Obsidian's chrome |
| Zed workspace actions [S42] | Toggle docks and maximize/restore a pane | One action clears surrounding study tools or temporarily focuses a source without losing parallel positions |
| Zotero collections [S43] | An item can belong to several collections without source duplication | Collect evidence for several studies while retaining one source record and independent membership |

These are interaction references, not proof that another application's whole UI fits Bible study, and not requests for more integrations. Their implementation code/licenses are not being reused here.

B004 should compare two low-chrome prototypes: (A) shared top bar plus inline pane labels and transient drawers; (B) compact edge launcher plus a passage field in the active pane, with explicit local headers on all panes. Both need visible, keyboard-accessible commands and persistent link-state clarity. Favor A as the starting hypothesis because its controls remain discoverable. Validate the choice rather than copying Logos's shell or declaring icon-only controls automatically better.

Measure useful reading area at fixed viewport/font sizes, visible command duplication, and interactions needed to inspect/capture a source. Also measure errors: maximized whitespace is not success if readers cannot find collections or misidentify the active passage. Focus mode hides chrome through a deliberate toggle, with a visible recovery affordance and Escape/keyboard support. It never relies solely on mouse hover, and temporary menus must not obscure the selected text on narrow screens.
