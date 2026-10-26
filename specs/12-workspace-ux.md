# Three-pane workspaces, notes and research UX

Revision 2 requirement: three parallel reading panes are a core use case, not a later extension of a two-pane comparison screen. The data model supports an ordered pane tree; the first release guarantees three simultaneous readable panes at suitable widths. Notes/research can occupy a pane or a separate dock. Extra panes are allowed only when useful dimensions and measured performance permit them.

## Workspace presets

| Preset | Main panes | Optional dock |
|---|---|---|
| Parallel reading | Translation A / Translation B / Translation C or Greek/Hebrew edition | Notes |
| Language study | Translation / Original-language text / Lexicon or commentary | Notes or research |
| Study and write | Bible / Commentary or ancient work / Notes | Research |
| Research | Bible / Source evidence / Selected chat | Notes |
| Margin mode | Frozen composite of selected panes, with ink canvas | Pens and snapshot metadata |

Presets are starting points. Users can save named layouts, duplicate/rename them, change content, resize/reorder panes, focus one pane, and restore a closed pane. Persistent state includes edition/revision, passage, scroll anchor, linked-navigation group, typography, pane widths, selected note/chat IDs and keyboard focus target. Missing content restores as an actionable placeholder instead of substituting a different edition. Credentials and transient selected private text never enter layout files.

## Linking and navigation

Each pane can be pinned (independent), in a named reference-linked group, or following the active passage. A deliberate navigation/scroll action elects the driver for that event; propagation uses reference mappings and a transaction ID to avoid feedback loops. Programmatic scrolls do not become new driver actions. Unsaved notes and selected handwriting are not moved by another pane's scrolling.

Users can link A+B while C remains fixed. A linked commentary moves only if it has an explicit compatible passage index. No mapping means an 'unmapped here' state, never a fabricated passage match. The app preserves per-pane history and a workspace-level reference history; Back has a defined active-pane target. Links offer open-here / open-in-pane / open-in-new-pane instead of unexpectedly replacing a comparison.

## Responsive behavior

Start with content-derived minimum widths: approximately 300–340 CSS pixels per reading pane at normal text size, increased where large text or original-language rendering needs it. These are prototype defaults, not immutable breakpoints. Three columns typically require a wide tablet in landscape or desktop/browser width. Resizing never forces tiny type.

On narrow widths retain all pane state and switch to tabs or a two-plus-one stacked arrangement. Phone users can cycle the same three sources and retain reference linking; they are not promised three illegibly narrow columns. Opening a notes or chat dock can reduce visible panes while preserving their state. Large-type and keyboard users receive the same commands through menus/shortcuts. Focus order follows visible pane order and separators have accessible resize controls.

## Notes as a first-class destination

Provide a notes pane plus a full notes view. Users can create passage, selection, topic and unattached notes; tag, search, link and attach snapshots. A note can target several passages/editions without duplicating its body. The notes pane shows a selected note or a list related to the active passage; auto-follow is explicit. Markdown editing, autosave with visible status, undo, revision/conflict history and recoverable deletion are required.

Separate semantic text highlights from ink strokes. 'Highlight selection' remains anchored to text and follows reflow; a drawn highlighter belongs to a fixed canvas. Links to an external Obsidian-backed note display its backing store and sync/conflict status. No AI edits a note without a visible proposed diff and acceptance.

## Search and research UX

Search starts as exact references/keywords and optional linguistic filters. 'Ask about these results' opens or selects a research thread and previews its evidence scope. Natural-language query expansion is visible/editable; search results remain usable without inference.

A research pane supports multiple named chats, branch/regenerate, archive/delete, streaming stop, model/provider selection and resume. Each chat has its own context scope; the current reader selection is never silently added to every conversation. Source chips open exact passages/edition revisions in another pane. Answer text distinguishes quotation, cited synthesis and an unsupported suggestion.

## Margin-mode entry and exit

A snapshot action previews which panes, scroll extent and private material will be captured. Default: currently visible reading content, without application chrome, chat or private notes unless selected. Scope options include one pane, the visible composite, or an explicitly bounded passage rendered across pages. Never silently capture an entire long document.

The frozen sheet offers added left/right/bottom margin space, pen/highlighter/eraser/lasso, undo and pan/zoom. Save creates a named margin note linked back to the original passages and workspace recipe. 'Return to reading' restores the live workspace without moving ink onto reflowing text. 'Capture again' creates a new snapshot revision; it never changes the old sheet under existing strokes.

## Acceptance additions

Test three distinct texts simultaneously and a Bible/Bible/notes layout, independently pinned/linkable panes, focus restore, missing-edition restoration, narrow-screen adaptation and large text. Include one long Hebrew/Greek passage and one difficult versification mapping. Performance reporting now includes three active reading panes and an open notes dock; the earlier two-Bible fixture is insufficient for the full workspace acceptance gate.

Usability sessions add: compare three renderings, take a linked note without losing place, embed a verse in Obsidian, capture a margin sheet, find a saved research thread and inspect a citation. Report accidental navigation, obscured active-pane state and misplaced ink as defects. Optional features stay out of the default reading path until the user enables them.
