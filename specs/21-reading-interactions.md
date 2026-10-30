# Reading interactions: panes, drawers, capture and handoff

B004 output. This document specifies the interactions the reader exposes; specs/16-design-guide.md
remains the governing visual and interaction baseline and is not restated here. Where the two
differ, specs/16 wins and this file is corrected. The SwiftUI prototype under
`apps/apple/OpenLogos/Sources/Prototype/` illustrates every section with hard-coded fixture
data; it is a fidelity prototype of interaction and typography, not the production reader
(B062 and later own that). Measurements and screenshots are in docs/evidence/B004.md.

Nothing in these flows requires an account, a store, a network connection or a model call.
Every state below is reachable with installed data only.

## 1. Pane model

A **workspace** is an ordered list of panes plus a dock. A **pane** shows one resource (a Bible
edition, an original-language text, a commentary or other work) at a **local passage** with a
scroll anchor. Panes have no fixed roles: any pane can hold any installed resource, and the
first release guarantees three readable panes at suitable widths (specs/12-workspace-ux.md).

Each pane carries exactly these visible identity facts, once, in one row:

| Fact | Shown as | Why |
|---|---|---|
| Resource identity | Edition or work short name, with the full title in the pane menu | A reader must be able to say which edition they are reading |
| Local passage | Native reference of the resource (own versification) | Panes may differ in versification; the mapping state is explicit |
| Link state | A word plus a glyph: `Pinned`, `Group 1`, `Group 2`, `Follows` | Never color alone; see §2 |
| Pane menu | One overflow control | Typography, history, replace resource, close, restore |

The identity row is the only per-pane chrome. There is no per-pane toolbar, formatting ribbon or
search field in layout A. Layout B adds the passage field to the active pane's header (§10).

**Active pane.** Exactly one pane is active. It is marked by its identity row's label weight, an
accent underline and the accessibility trait `isSelected`; its name is announced on focus
change. Tapping, clicking or keyboard-focusing inside a pane activates it. Global commands
(passage field, Study, Collect, Highlight) act on the active pane unless a selection exists in
another pane, in which case the selection's pane is the target and the confirmation names it.

**Pane operations.** Open here, open in pane N, open in new pane; replace resource (never
touches notes or collection items anchored to the old one); close (recoverable with its
location through Undo and the workspace menu's Restore pane); reorder by drag or by the pane
menu's Move left / Move right; resize with the separator, which is an accessible slider with
a 44-point touch target and keyboard step of 8 points; focus (§8).

**Missing resource.** A workspace that references an uninstalled or unavailable resource
restores the pane as an actionable placeholder with the resource name, passage and the actions
Install, Choose another, Close pane. It never substitutes a different edition.

## 2. Link and pin groups

A pane is in exactly one of three navigation modes:

- **Pinned.** Independent. Navigation elsewhere never moves it.
- **Group n.** Member of a named reference-linked group. A deliberate navigation in any member
  (typed passage, verse tap, cross-reference "open here", scroll past a verse boundary) elects
  that pane as the driver for the event, which carries a transaction ID. Other members move
  through the reference mapping; programmatic moves do not become new driver events.
- **Follows.** Follows the active passage of whichever pane is active, without being a driver.
  Used for a lexicon or commentary pane that should track reading.

Groups are independent: A+B in Group 1 while C is pinned; A in Group 1, B+C in Group 2. Up to
three groups exist; the label shows the group number and the glyph so colorblind and
grayscale readers see the same state. Changing mode is a one-step action in the identity row's
link control (a menu with the three modes and the available groups), with the keyboard command
Toggle link (§9) cycling Pinned → Group 1 → Pinned for the active pane.

Mapping outcomes are visible: `exact` moves silently; `partial` and `ambiguous` move and show a
one-line notice in the pane ("Psalm 9:21 here corresponds to 10:1 in the driver"); `unmapped`
does not move and shows "Unmapped here", with the driver's reference and an Open anyway action
that opens the nearest division. A commentary without a passage index cannot join a group; the
link control explains why and offers Follows instead.

Unsaved notes, an open Study drawer, a selection and handwriting are never moved by another
pane's navigation. Per-pane history and a workspace-level reference history exist; Back acts on
the active pane.

## 3. Selection and the study drawer

Selection is the platform's. On iPadOS, iOS and macOS the text surface uses the native text
selection with its handles and edit menu; project actions are added to the native menu as a
labeled group, in this order: **Highlight**, **Collect**, **Study**, **More**. Copy, Look Up,
Translate, Share and the other system items stay where the OS puts them. The prototype adds the
same four actions as a visible action strip beneath the native menu because SwiftUI `Text`
does not expose the selected range; the production reader (B062) uses `UIEditMenuInteraction`
and `NSMenu` extension instead and must not replace the OS menu.

Selection is arbitrary and can cross verse boundaries. The selection target is the exact UTF-16
range which the core turns into anchors (`selectionToAnchors`); the UI never rounds a selection
to a verse unless the reader chose a verse number.

- **Highlight** applies the last-used named highlight immediately and shows a one-line
  confirmation with the highlight's name and Undo; a second control opens the small palette
  (user-named colors with underline and marker alternatives). Highlights follow reflow.
- **Collect** adds a source item for the selection to the active collection (§5).
- **Study** opens the drawer (below) without changing selection or scroll.
- **More** holds Copy with reference, Open in pane, Search for this, Short observation, Compare
  renderings. A short observation is a one-line text field in the capture confirmation, not an
  editor.

The action strip and the drawer never cover the selection on narrow screens: in compact width
the drawer opens as a sheet at medium detent anchored below the selection's pane and the pane
scrolls the selection above the sheet if it would be hidden.

**Study drawer.** A drawer, not a cascading menu. Its header states what was selected: the
selected wording, the reference and the edition. It progresses through four sections, each
collapsible, in this fixed order:

1. **Context**: the verse with the selection marked, translation footnotes for the span, the
   paragraph or poetry unit it belongs to.
2. **Cross references**: attributed relations for the span's verses with source and kind
   (imported cross reference, explicit citation, quotation, allusion, lexical parallel,
   thematic). Each opens a transient passage preview with its own local navigation and the
   actions Open in pane and Collect. The preview never moves the reading pane.
3. **Word**: where licensed alignment exists, the aligned original-language token(s), lemma,
   transliteration on request, and a plain-language morphology summary with the technical code
   behind a disclosure. A span may align to several tokens or to none; "No alignment data for
   this span in this edition" is a normal state, not an error, and Strong's numbers are an
   optional reference field, never the word's identity. Renderings and occurrence counts show
   scope, counting unit and coverage; no chart without known coverage.
4. **Sources**: installed commentaries and works with an entry for this passage, as attributed
   excerpts with Collect. Uninstalled works the catalog knows appear as metadata with Install.

Within each section, content is marked as Scripture, source-backed data, attributed
interpretation or (only when the reader invoked it) generated text. The drawer can be pinned as
a pane with one action; pinning keeps its scroll position and the pinned pane then has the
ordinary identity row (resource: "Study", passage: the selection's reference, mode: Follows).
Closing the drawer restores focus to the selection and leaves selection and scroll intact.

## 4. Notes dock

Notes are a first-class destination (full Notes view) and an optional dock. The dock is a pane
slot at the trailing edge in regular and wide widths and a sheet in compact width. It shows
either the selected note or the list of notes related to the active passage; following the
active passage is an explicit toggle in the dock header, off by default.

The dock editor is Markdown with formatting controls visible only while editing. The note index
and its filters collapse independently of the editor. Autosave shows a status word (`Saved`,
`Saving`, `Offline, saved locally`, `Conflict`) in the dock header. Opening the dock can reduce
the visible pane count; the hidden pane keeps its state and is listed in the workspace menu.

A note anchored to a passage shows its anchors as chips; one body can target several passages
and editions. A vault-backed note shows its backing file and sync state (specs/13). The dock
never becomes a manuscript editor: there is no outline, section or delivery mode.

## 5. Collection capture

The current collection is shown as a compact **destination indicator**: the collection's name
and its item count, once, in the shared bar (A) or the edge launcher (B). Capture never opens
an editor and never replaces a pane.

Capture flow, every input method:

1. Reader selects text (or a verse number, a cross-reference row, a word-study result, a source
   excerpt) and chooses Collect (selection menu, action strip, keyboard command, or drag onto
   the indicator where pointer or touch allows).
2. A confirmation appears for a few seconds near the indicator, not over the selection: "Added
   to *Romans 8 series* (4)", with Undo, Change collection, and Add observation. Add observation
   reveals a one-line field; Return commits it as a separate observation item attached to the
   source item.
3. The indicator count increments. Nothing else moves.

No active collection: Collect opens a small chooser (recent collections, New collection) and
then continues. A duplicate capture of the same anchor and edition focuses the existing item in
the confirmation ("Already in this collection", with Show and Add observation anyway); it does
not create a second item.

The **collection drawer** is closed by default. The indicator opens it. In regular and wide
widths it opens as a trailing drawer over the last pane's width (the covered pane is named in
the drawer header: "Covering KJV; pin to keep both"); in compact width it is a sheet. Pin keeps
it as a pane. It lists items in order with grouping under user-named headings, each item showing
kind (passage, excerpt, observation, cross reference, margin sheet), resource identity, exact
locator, and rights state (quotable, metadata only). Items can be reordered, grouped, tagged and
removed with Undo. Tapping an item's locator opens the source in a passage preview; Open in pane
is explicit.

## 6. Obsidian handoff

Handoff is an export of the collection to the user's vault through the plugin or through a file
transfer, never a write into the user's manuscript note. The flow:

1. From the collection drawer or the full Collections view: **Hand off to Obsidian**.
2. **Preview** lists destination (vault and app-owned folder, or "Export file" where no vault is
   reachable), each item with its quotation text or "reference only" and the reason (rights
   unknown, rights denied, source not installed), included observations, attachments and the
   stable item IDs. Restricted and unresolved items are listed, not dropped.
3. **Export** writes the collection index, evidence blocks and materialized scripture notes with
   stable block IDs and return-to-source links. The result screen shows what was written and
   offers Insert links into current note (plugin only, inserts links or embeds at the cursor of
   the user's open note and nothing else) and Open in Obsidian.
4. **Re-export** reconciles managed blocks by ID and last-export hash. Changed source quotations
   offer a new resource revision; renamed or deleted blocks never retarget an existing embed;
   concurrent edits surface as a conflict (§11) with both bodies kept.

Handoff works offline on the same machine; it does not touch a model or a server. A user without
Obsidian gets the same package as Markdown plus JSON through the share sheet or a file picker.

## 7. RTL, large type and VoiceOver

**Right-to-left.** A pane's layout direction follows its resource's script, not the UI
language: a Hebrew edition lays out right-to-left inside a left-to-right workspace. Verse numbers
sit on the reading-start side (right for Hebrew), the identity row keeps the UI direction, and
the paragraph uses the platform bidi algorithm with the stored text untouched (no mark
stripping, no normalisation). Mixed Hebrew-in-English notes use isolates as the core stores
them. In a right-to-left UI language the whole chrome mirrors and pane order is visually
reversed with the data order unchanged; the active-pane and link labels remain words.

**Large type.** All text uses Dynamic Type styles; leading scales with `@ScaledMetric`.
Chrome heights grow with the text size and never clip: at accessibility sizes the shared bar
wraps into two rows (passage field on its own row) and the identity row truncates the resource
name before the link state. Cantillation and Greek diacritics must not clip at any size: the
line height is at least 1.45 × the font size and the glyph boxes are not clipped by fixed-height
rows. Pane minimum widths scale with the text size; when three panes no longer fit, the
workspace shows two plus the third as a tab with its state preserved, then one plus tabs. Text
is never shrunk to keep a pane count. Touch targets are at least 44 points at every size.

**VoiceOver and keyboard.** Focus order is: shared bar (or launcher), then panes in visible
order, each pane's identity row before its text, then the dock, then any drawer. A pane's
identity row is one accessibility element announcing "Berean Standard Bible, John 3:16, Group
1, active" with the menu as a custom action. Verse numbers are accessible elements with the
label "verse 16" and the action Select verse; they are not links. Highlights announce their
name. The Study drawer is a modal landmark on open with focus on its header; Escape or the
Close button returns focus to the selection. Collect confirmations are polite announcements.
Link-state changes announce the new mode. Every pointer or hover enhancement has a touch and a
keyboard route to the same information.

## 8. Focus mode

Focus mode hides the shared bar, the identity rows of inactive panes and the dock, and centers
the active pane at the prose measure. It is entered by a deliberate command (menu item, keyboard
command, or the pane menu) and never by hover or by scrolling. A small persistent recovery
control ("Exit focus", 44-point target) stays in the top trailing corner; Escape exits; moving
the pointer to the top edge reveals the bar as an enhancement only. Exiting restores the exact
prior layout, pane widths, drawer state, selections and scroll positions. Focus mode never
closes other panes or clears a selection.

## 9. Keyboard commands

> Revised by specs/22-workspace-shell.md §3 and §6 (B070): every command below is an entry in the
> core command registry (`crates/workspaces`, `commands::registry`), ⌘K focuses the palette that
> replaces the passage field and launcher, and the pane commands become the column and tab
> bindings of specs/22 §6 (⌥1…⌥9 focus column N; ⌘⇧N is New workspace on native hosts, so the
> notes dock is toggled from the palette). The rows not replaced there keep their bindings.

Commands are available from the menu bar (macOS), the workspace menu, and the command launcher
(⌘K) on every platform with a hardware keyboard. Shortcuts are enhancements; every command has a
visible route.

| Command | Shortcut | Scope |
|---|---|---|
| Passage field | ⌘L | Workspace (A: shared bar; B: active pane header) |
| Command launcher | ⌘K | Workspace |
| Search | ⌘F | Workspace |
| Activate pane 1, 2, 3 | ⌘1, ⌘2, ⌘3 | Workspace |
| Next, previous pane | ⌘⌥→, ⌘⌥← | Workspace; in compact width this cycles the visible pane |
| Toggle link (Pinned ↔ Group 1) | ⌘⇧L | Active pane |
| Pin pane | ⌘⇧P | Active pane |
| Highlight selection | ⌘⇧H | Selection |
| Collect selection | ⌘⇧C | Selection |
| Study selection | ⌘⇧S | Selection |
| Toggle notes dock | ⌘⇧N | Workspace |
| Toggle collection drawer | ⌘⇧O | Workspace |
| Focus mode | ⌘⇧F | Workspace |
| Back, Forward | ⌘[, ⌘] | Active pane |
| Close drawer or leave focus mode | Esc | Topmost transient |
| Undo | ⌘Z | Last annotation or layout operation |

In a right-to-left UI the arrow-key pane commands follow visual order.

## 10. Compact layouts A and B

Both layouts share the pane model, the drawers, capture and handoff above. They differ only in
where global commands live.

**Layout A: shared top bar, inline pane labels, transient drawers** (the starting hypothesis in
specs/16). One 44-point shared bar holds, in order: workspace menu (Read, Search, Library, Notes,
Collections, layouts), the passage field, Search, the collection destination indicator, and an
overflow with appearance and typography. Each pane has a 36-point identity row (§1). Drawers are
transient.

**Layout B: compact edge launcher, passage field in the active pane, explicit local headers.** No
top bar. A 44-point-wide launcher on the leading edge holds the same global actions as icons with
labels on hover or long press, plus a visible label row in compact width where the launcher
becomes a 44-point bottom strip. Every pane has an explicit 44-point header with the resource,
passage, link state and menu; the active pane's header additionally contains the passage field.

Neither layout has a mandatory rail or duplicated ribbons. The measured comparison (reading area
at 1440 × 900, 1024, 768 and 390 points wide; duplicated commands; interactions to inspect and to
capture a source) is in docs/evidence/B004.md with the method. The prototype's debug switch
(`Layout` in the debug menu, or the launch argument `-OPENLOGOS_LAYOUT A|B`) renders either.

At compact width (under about 700 points) both layouts show one pane with the others as tabs
under the identity row; link state is preserved and announced. At 768 both show two panes plus
a tab; at 1024 and above, three.

## 11. Required states

Every major interaction designs these states. The prototype reaches each one through the debug
menu's State picker or `-OPENLOGOS_STATE <name>`.

| State | Behavior |
|---|---|
| Loading | Skeleton lines in the pane at the prose measure, identity row already filled, no spinner over text; after 2 s a line "Still opening Berean Standard Bible" with Cancel. Chrome stays usable. |
| Empty | Collection with no items: "Nothing collected yet. Select text and choose Collect." The drawer stays closable; the indicator shows 0. Search with no results: the query echoed, scope stated, suggestions that only widen scope explicitly. |
| Offline | No network changes reading, search, capture or handoff. Only catalog browsing and remote model calls show "Offline" on their own controls. A vault reachable by file transfer only is "Export file" in the handoff preview. |
| Unavailable source | A pane whose resource is not installed restores as a placeholder with Install, Choose another, Close. A cross-reference target in an uninstalled edition shows metadata and Install; it never shows a different translation. |
| Invalid query | A local error under the search field with the position of the problem and a corrected example; the previous results stay visible; no model fallback. |
| Read-only | A resource with view-only rights (or a borrowed package) shows "Reference only" on capture: the item is added as metadata without quotation and the confirmation says so. Teaching view (D18-1) is read-only by design. |
| Conflict | A vault-backed note or exported block edited on both sides shows both bodies with timestamps and Keep mine, Keep theirs, Keep both; nothing is overwritten until chosen, and unsaved work stays on screen during the dialog. |

Undo restores the prior annotation or layout operation where feasible, and an accidentally
closed pane is restored with its location.

## 12. Margin capture (snapshot only)

Entering margin mode previews which panes and extent will be captured: the visible reading
content of the selected panes without chrome, dock, chat or private notes unless checked. The
preview shows the sheet with its added margin space and the capture metadata (editions,
revisions, references, pane rectangles). Capture produces a `ReadingSnapshot`; the ink tools
themselves are P2 (specs/13). Return to reading restores the live workspace unchanged. A read-only
or restricted source is captured as its reference and a hatched box, never as text.

## 13. Out of scope for the reader

No sermon drafting, manuscript lifecycle, delivery or teleprompter mode, slides, timers, or
generated sermon content appear anywhere in these flows (specs/15, specs/18 D18-1). Notes
support study; the collection hands off; writing happens in the user's own environment.
