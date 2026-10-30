# Workspace shell: numbered columns, tabs, parallel workspaces and the command palette

Revision 1, 2026-10-30. Owner direction for the usability-baseline sprints (B070–B075). Where
this file and specs/12, 16 or 21 disagree, this file wins and the older text is revised in the
same change that implements it. Unchanged from those specs: the text is the primary surface,
link groups and pane identity (specs/21 §1–2), selection actions (specs/16), required states
(specs/21 §11), accessibility targets (specs/16) and offline operation with no account.

## 1. Owner decisions (2026-10-30)

1. **The command palette can do everything the interface can, and more.** Every action is a
   command in one registry in the Rust core; buttons, menus, shortcuts and the palette all
   dispatch the same command, so the palette is a superset by construction, not by audit.
2. **The palette is search.** One field at the top of the window, always visible, takes a
   reference, a word or phrase, a command, a tab or workspace name. ⌘K focuses it. There is no
   second search box.
3. **Columns are numbered.** Columns are 1…n from leading to trailing in every workspace, the
   number is shown in the column's identity row, and keyboard commands and palette commands
   address a column by number ("open in 2", "move tab to 3").
4. **Each column holds tabs.** A column is a stack of tabs; one tab is shown. A tab is what
   specs/21 called a pane: one resource at one passage with its own scroll anchor, history and
   link state.
5. **Parallel workspaces.** Any number of workspaces exist at once, each with its own columns,
   tabs, widths and active column. Switching is instant and loses nothing. The palette searches
   across all workspaces (tabs by resource, passage and title; workspaces by name).
6. **A floating tool palette on the left**, in the manner of the old Adobe tool palettes:
   modes and tools that change what a click, tap, drag or Pencil stroke does.
7. **No cost in speed.** The budgets in §7 are acceptance criteria, measured on every host.

This replaces specs/16's "no permanent navigation rail" for the tool palette only: it is a
small floating tool well, not a navigation rail, and it can be collapsed or hidden.

## 2. Model

```
Session
  workspaces: [Workspace]          ordered; one is active
Workspace
  id, title
  columns: [Column]                1…n, leading to trailing; one is active
  notes_dock                       as specs/21 §4
Column
  id
  tabs: [Tab]                      ordered; one is shown
  width_fraction
Tab                                (specs/21's pane)
  id, kind, content_id, content_revision, reference, scroll_anchor,
  link_group | pinned, history
```

- Column numbers are positions, not ids: moving a column renumbers it, and the identity row
  says so on the next frame.
- Link groups span columns and tabs within one workspace, never across workspaces. A tab that
  is not shown still follows its group, so revealing it never shows a stale passage.
- Layout profiles (specs/18, B062) keep their meaning per workspace: a compact device shows
  one column at a time with the others reachable by number; nothing is closed by a narrower
  window.
- Persistence is the workspace schema at a new MAJOR version (ADR 0003): a 1.x workspace
  migrates losslessly, each visible pane becoming a column with one tab and the tab-order panes
  becoming tabs of the last column. The session file stores workspace order and the active
  workspace; each workspace stays its own record so one can be shared or exported alone.

## 3. The command palette

**Placement.** A single-line field across the top of the window, above the columns, always
visible (the "shared bar" of layout A, now owned by the palette). Focusing it (click, ⌘K, or
`/` when no text field has focus) opens a result list beneath it, over the columns, at most
half the window tall. Esc closes the list and returns focus to the tab that had it.

**One ranked list, typed sections.** Results are grouped in this order, each group capped and
expandable:

| Section | Matches | Enter does |
|---|---|---|
| Passage | `parseReference` succeeds (`Jn 3:16`, `Rom 8:28-30`, `ברא 1`) | Navigate the active tab (its link group follows) |
| Commands | title, aliases, keywords of every registered command | Run it; commands with arguments take them inline |
| Tabs and workspaces | resource, passage and title of every tab in every workspace; workspace names | Switch to that workspace, column and tab |
| Text | full-text hits in the installed editions (§5), with edition, reference and a snippet | Open the passage in the active tab |

**Grammar.** Plain typing searches everything. A leading `>` restricts to commands, `@` to
tabs and workspaces, `#` to text, `:` to a passage. A trailing ` @N` targets column N for any
result that opens something (`Jn 3:16 @2`, `#agape @3`); ` @new` opens a new column.

**Superset rule.** A command appears in the palette if and only if it is in the core registry;
a host may not add a button, menu item or shortcut whose action is not a registered command.
The registry carries: id, title, aliases, scope (session, workspace, column, tab, selection),
argument shape, default shortcut per platform, and whether it is undoable. Commands that only
make sense with a selection or a tool still appear, disabled with the reason.

**Command set (first cut).** New, close, rename, duplicate and switch workspace; new column,
close column, move column left or right, focus column N; new tab, close tab, reopen closed
tab, next or previous tab, move tab to column N, duplicate tab into column N; navigate,
back, forward; link, pin, follow; open notes dock, collection drawer, study drawer; choose
tool (§4); text size up, down, reset; theme; focus mode; export workspace.

## 4. The floating tool palette

A narrow vertical well floating at the leading edge, one column of tool buttons (two at large
sizes), draggable along the edge, collapsible to a grip, hideable by command. It holds modes,
not destinations:

| Tool | Key | What a click, drag or stroke does |
|---|---|---|
| Select | V | Native text selection; the default |
| Highlight | H | A drag over text highlights it with the current swatch, no menu |
| Pen | P | Ink in the margin layer (B039, crates/ink) |
| Highlighter pen | M | Ink with the highlighter brush, composited once per stroke |
| Eraser | E | Removes whole strokes (stroke eraser, B039) |
| Collect | C | A drag over text collects it into the open collection |

Below the tools: two swatch wells (current and alternate, `X` swaps) and the current brush
width. Single-letter keys work only when no text field has focus; every tool is also a
palette command. The current tool is shown by a filled button and a label in the tooltip and
accessibility value, never by colour alone. On touch, a Pencil uses the current tool and a
finger always scrolls and selects (B039's palm rules).

### 4.1 Context (B076)

Owner direction, 2026-10-30: the well is contextually useful for what it is looking at. The
tool set follows the focused column's shown tab, and a text selection adds an actions group
below the tools. The core decides the well (`ShellHandle.tool_well` / `toolWell`), so web
and Mac stay in lockstep (§8).

**Contexts.** Focus is the active column's shown tab unless the host says the notes dock or
an empty column has it (`focus`: `tab`, `notes_dock`, `empty`).

| Context | When | Default tool |
|---|---|---|
| `translation` | A Reading tab whose resource is not in an original language (BSB) | Select |
| `original` | A Reading tab whose registered language is `grc`, `hbo`, `arc`, `he` or `heb` (SBLGNT, WLC) | Select |
| `notes` | A Notes tab, or the notes dock with focus | Type |
| `reference` | A Lexicon or Research tab | Select |
| `none` | An empty column has focus | Select |

Resource languages are registered by the host as BCP 47 tags (`setResourceLanguages`), as
resource titles are; a resource without one reads as a translation.

**Groups**, top to bottom:

| Group | Shown | Items (key) |
|---|---|---|
| Tools, translation | always | Select V, Highlight H, Pen P, Highlighter pen M, Eraser E, Collect C |
| Tools, original | always | Select V, Word study W, Highlight H, Pen P, Highlighter pen M, Eraser E, Collect C |
| Tools, notes | always | Type T, Bold ⌘B, Italic ⌘I, Heading, List, Link passage, Collect C |
| Tools, reference | always | Select V, Highlight H, Collect C |
| Tools, none | always | Select V |
| Ink | translation and original | Swatch, Alternate swatch (X swaps), Brush width |
| Selection | text selected, context not `none` | Highlight now ⌘⇧H, Collect ⌘⇧C, Note on it, Search this ⌘E, Copy reference, Study ⌘⇧S |

Word study and Type are tools (modes). Bold, Italic, Heading, List and Link passage are
actions on the note editor: their commands (`note.*`) come back unhandled for the host, and
⌘B and ⌘I work only while the note editor has focus. Link passage takes a passage: the
palette fills it inline (`link passage Jn 3:16`), and in notes a typed passage also offers
"Link John 3:16" under the navigation row. The selection actions are host-performed like
the existing ones: Note on it appends the quoted text and its reference to the open note,
Search this puts the selected text into the palette field, Copy reference copies the
selection's reference in `formatHuman` form.

**Effective tool.**

| Rule | Behaviour |
|---|---|
| Storing | Choosing a tool stores it in the session; moving focus never changes the stored tool |
| Fallback | When the context does not offer the stored tool, the context's default is in effect and marked active |
| Restore | Returning to a context that offers the stored tool puts it back in effect |
| Out of context | A tool or note command outside its context is disabled with a reason in words and stays in the palette |

| Command outside its context | Reason |
|---|---|
| Pen, Highlighter pen, Eraser | "Pen works on Bible text" |
| Word study | "Word study works on Greek and Hebrew text" |
| Type, Bold, Italic, Heading, List, Link passage | "Type works in notes" |
| Select, Highlight in notes | "Select works outside notes" |
| Highlight, Collect with nothing focused | "Highlight works with a tab open" |

**Icons** are neutral names each host maps (SF Symbols on Mac, inline SVG on web): `select`,
`word-study`, `highlight`, `pen`, `highlighter-pen`, `eraser`, `collect`, `type`, `bold`,
`italic`, `heading`, `list`, `link-passage`, `swatch`, `swatch-alternate`, `brush-width`,
`highlight-now`, `note`, `search`, `copy-reference`, `study`.

The well is rebuilt on every focus change; building it is part of the column-focus budget
of §7.

## 5. Text search for the baseline

The palette's Text section searches the three editions approved for the preview on
2026-10-30: S52 Berean Standard Bible, S06 SBLGNT, S07 WLC with OSHB markup. They are packed
from the NAS artifacts with the B064 retrieval engine (SQLite FTS5 with the project tokenizer)
into a local preview pack outside the repository; nothing is committed and the pack is not the
B009 import. Lemma and morphology search are out of scope for the baseline.

## 6. Keyboard

Browsers reserve ⌘T, ⌘W, ⌘N and ⌘1–9, so the default bindings use ⌥ and are the same on web
and Mac; native hosts may add the platform-conventional aliases.

| Command | Default | Native alias |
|---|---|---|
| Focus palette | ⌘K, or `/` outside a text field | |
| Focus column N | ⌥1…⌥9 | ⌘1…⌘9 |
| Move current tab to column N | ⌥⇧1…⌥⇧9 | |
| New tab in current column | ⌥T | ⌘T |
| Close tab | ⌥W | ⌘W |
| Next, previous tab in column | ⌥], ⌥[ | ⌃Tab, ⌃⇧Tab |
| New column | ⌥N | |
| Next, previous workspace | ⌥⇧], ⌥⇧[ | |
| New workspace | ⌥⇧N | ⌘⇧N |
| Tools | V H P M E C, W word study, T type (§4.1); X swaps swatches | |
| Note editor (focus in a note) | ⌘B bold, ⌘I italic | |
| Search the selection | ⌘E | |
| Back, forward | ⌘[, ⌘] | |

Every binding is a command, so the palette shows it next to the command and the user can
search for it.

## 7. Speed budgets

Measured on the Mac mini (Apple M1) for web (Chrome) and the Mac app; iPad numbers are added
when the device is available. p95 over at least 200 samples unless stated.

| Interaction | Budget |
|---|---|
| Keystroke to updated palette results: passage, commands, tabs | ≤ 16 ms (one frame) |
| Keystroke to text results over the three editions | ≤ 50 ms |
| Tab switch within a column, column focus | next frame (≤ 16 ms) |
| Workspace switch, ten workspaces of 3 columns × 4 tabs | ≤ 100 ms to first painted text |
| Open a chapter in a tab (warm) | ≤ 100 ms |
| Cold launch to the restored workspace's text | ≤ 1 s |
| Tool change | next frame |

Hidden tabs and workspaces keep their model state but release their rendered text; the
budgets above include re-rendering a released tab.

## 8. Lockstep on web and Mac

Each sprint lands on both the web shell (apps/web) and the Mac app (apps/apple) from the same
core model and command registry; a feature is done when both hosts pass its cases and the
budgets. iPad follows when a device is available (B075).

## 9. Out of scope for the baseline

Lemma and morphology queries, cross-workspace link groups, tear-off windows, multi-window
sessions, sync of sessions between devices, research chats, and any content beyond the three
approved editions and the existing fixtures.
