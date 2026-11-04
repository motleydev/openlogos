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

B078 (2026-11-03): a reading tab is a continuous scroll through its edition, and its passage
follows the reading position; specs/23-continuous-reading.md describes the stream, the
reading API, the text pack format and the host behaviour.

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
  What a host saves is the user store of specs/24-persistence.md (B074): the session record
  and its workspace records in one document with the notes, highlights and ink, saved 1 s
  after the last change and on quit, restored at launch with every tab's scroll anchor.

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

**Word study results** open through `tab.open_lexicon` (`text`: the word as clicked,
`reference`, and `column` or `new_column`). The target is the column given, else the column
to the right of the active one, else a new column. A Lexicon tab already in the target column
is reused (its word and passage change, it is pinned and shown); otherwise a new pinned
Lexicon tab opens there and is shown. Focus stays on the column where the word was clicked, so
the reader keeps reading. The command is undoable like `tab.new`;
the tab carries the word as `lexicon_word` (workspace schema 2.0, optional). In the palette,
"Open in lexicon" takes the word inline (`open in lexicon agape`) and without one is disabled
with "Needs a word to look up".

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

### 4.2 Context menu (B077)

Owner direction, 2026-11-03: the OpenLogos menu replaces the right-click menu on web and in
the native app, with power features such as word searches. As with the well, the core
decides the menu for what was clicked (`ShellHandle.context_menu` / `contextMenu`) and the
hosts only draw it and perform host-side effects, so web and Mac stay in lockstep (§8).

**Where.** The OpenLogos menu replaces the platform menu in reading text, notes, tab strips,
identity rows and the empty area of a column. Elsewhere (the palette field, dialogs) the
platform menu stays.

**The platform menu stays reachable.** ⇧-right-click on web and ⌥-right-click on Mac fall
through to the platform's own menu, so Copy, Look Up, spelling, Services and Inspect remain
one modifier away. One convention per host; neither is a registered command, as opening a
menu is not an action.

**Keyboard.** The same menu opens from the keyboard for what has focus: ⇧F10 or the
context-menu key on web; ⌃-click, VoiceOver's open-menu command (VO-⇧-M) and the context-menu
key of a PC keyboard on Mac. The target is the selection if there is one, else the word at
the caret in reading text or the caret in a note, else the focused tab, else the background
of the active column; the menu opens at it. Arrow keys move, Return runs, Esc closes and
returns focus.

**Targets** (the host passes what was clicked; `tab_id` absent means the active tab, or the
notes dock for a note):

| Target | Fields | Menu context |
|---|---|---|
| `word` | `text` as shown, `reference`, `segment_id`, `scalar_start`/`scalar_end`, `resource_id`, `tab_id` | the host selects the word as it opens the menu, as macOS does, so Mark items are the selection commands |
| `selection` | `text`, `reference` (start), `end_reference` when it ends in another verse, `resource_id`, `tab_id` | a selection |
| `note_text` | `text` when note text is selected, `tab_id` of a Notes tab | notes, with or without a selection |
| `note_link` | `reference` of a passage link, `tab_id` | notes |
| `tab` | `tab_id` | the tab |
| `column` | `column` number (identity row or badge) | the column |
| `background` | `column` number, optional | the column |

**Focus first.** A menu on something that does not have focus carries `focus`: the
`tab.focus` or `column.focus` a click there would have run. The host applies it before the
chosen row (focus changes are not undoable) and dispatches every row under the menu's
`context`; the menu's rows are worked out as if that focus change had happened. A tab or
column that does not exist is refused (`no_such_tab`, `no_such_column`).

**Menu shape.** `ContextMenu { target_kind, title, focus, context, sections: [{ label, items
}] }`; a row is `{ command_id, args, label, key, icon, active, value, disabled_text, submenu }`,
the tool well's fields where they overlap. Every row is a registered command with its
arguments filled from the target (the clicked word's passage and edition, the tab's column).
Disabled rows stay visible with words; a section that does not apply (Open without a passage)
is left out. A submenu is one level deep; its parent row carries the first enabled child's
command, which a host may run on click where its platform allows. `active` marks the current
choice (theme, swatch, the tab's link mode). Menus have at most twelve top-level rows, power
features first.

| Target | Sections and rows |
|---|---|
| `word` | **Search**: Search “word” in this edition, … in all editions (`search.text`, scope `edition` / `all`), Search lemma (disabled: "Needs the lexicon import"), Word study (`tab.open_lexicon`; on a translation disabled: "Word study works on Greek and Hebrew text"), Find in this chapter (`search.text`, scope `chapter`). **Open** (with a passage): Compare in *edition* or Compare in ▸ (`tab.open_edition`), Open John 3:16 in ▸ column N / New column (`nav.navigate`), Cross references (disabled: "Needs cross-reference data"). **Mark**: Highlight ▸ swatches (`selection.highlight_in`), Collect, Note on it. **Copy** ▸: Copy word (`copy.text`), Copy reference (`copy.reference`), Copy verse with reference (`copy.passage`) |
| `selection` | **Search**: Search selection in this edition / in all editions. **Open**: Compare, Open in ▸. **Mark**: Highlight ▸, Collect, Note on it, Study. **Copy**: Copy (`host.copy`), Copy with reference (`copy.passage` with the text and the passage range) |
| `note_text` | **Edit**: Cut, Copy, Paste (`host.cut`, `host.copy`, `host.paste`; Cut and Copy need a selection). **Format**: Bold, Italic, Link passage (the host asks for the passage). **Search**: Search selection in all editions |
| `note_link` | **Open**: Open in ▸ column N / New column, Compare in ▸. **Link**: Copy reference, Remove link (`note.remove_link`) |
| `tab` | **Tab**: Close tab, Close other tabs (`tab.close_others`). **Move**: Duplicate to ▸, Move to ▸ (every column and New column). **Link**: Pin, Follow active passage, Link group ▸ (the workspace's groups and the next free number) |
| `column` | **Column**: Focus column N, New tab. **Move**: Move left, Move right. **Layout**: Equalize widths, Close column |
| `background` | **Tab**: New tab, Paste passage (`nav.paste_passage`; reading tabs). **View**: Text size ▸ (Larger, Smaller, Actual size), Theme ▸ |

Open in ▸ lists every column except the one the passage is read in and columns showing a note.
**Editions** are the resources with a registered language; Compare offers the others,
leaving out an original-language edition that cannot hold the passage (Greek outside the New
Testament, Hebrew and Aramaic outside the Old). The swatches are the four of
design/tokens/shell.json, with a custom current swatch first.

**New commands**, appended to the registry and findable in the palette:

| Command | Arguments | Performed by | Notes |
|---|---|---|---|
| `search.text` "Search text" | `text`, `scope` (`edition`, `all`, `workspace`, `chapter`; default `all`), `resource_id`, `reference` | host | puts the query in the palette with the scope and runs it |
| `search.lemma` "Search lemma" | `text` | host | disabled: "Needs the lexicon import" |
| `nav.cross_references` "Cross references" | `reference` | host | disabled: "Needs cross-reference data" |
| `tab.open_edition` "Open in edition" | `resource_id`, `reference` (else the active tab's), `column` or `new_column` | core, undoable | the target column is the one given, else right of the active one, else a new column; a Reading tab of that edition there is reused (moved alone, its group not dragged), else a new tab with the source's link state opens there; focus stays on the source |
| `tab.close_others` "Close other tabs" | none | core, undoable | closes the active column's other tabs, each reopenable |
| `copy.text` "Copy text" | `text` | host | |
| `copy.reference` "Copy passage reference" | `reference` | host | `formatHuman` form |
| `copy.passage` "Copy passage with reference" | `reference`, `text` (else the passage's text in `resource_id`) | host | |
| `host.cut`, `host.copy`, `host.paste` | none | host | the platform's own action; ⌘X ⌘C ⌘V shown in the menu, no registry binding |
| `nav.paste_passage` "Paste passage" | none | host | parses a passage from the clipboard and opens it |
| `selection.highlight_in` "Highlight in colour" | `text` (swatch `#RRGGBB`) | host | needs a selection |
| `note.remove_link` "Remove passage link" | `reference` | host | in notes only |

Icons beyond the well's: `search-all`, `lemma`, `find`, `compare`, `open`, `column`,
`new-column`, `cross-references`, `copy`, `copy-passage`, `cut`, `paste`, `remove-link`,
`close`, `close-others`, `duplicate`, `move`, `pin`, `follow`, `link`, `focus`, `move-left`,
`move-right`, `new-tab`, `equalize`, `close-column`, `paste-passage`, `text-size`,
`text-larger`, `text-smaller`, `text-reset`, `theme`.

The menu is built on every right-click and must show on the next frame (§7).

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
| Context menu (§4.2) | right-click, ⇧F10, context-menu key | ⌃-click, VO-⇧-M |
| Platform menu instead (§4.2) | ⇧-right-click | ⌥-right-click |
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
| Context menu shown after a right-click | next frame (≤ 16 ms) |
| Save the user store (core share, ten workspaces of twelve tabs, 200 notes, 1,000 highlights, 200 strokes; specs/24) | ≤ 16 ms |
| Restore at launch, file read to the active workspace's first painted text (specs/24 §5) | ≤ 100 ms |
| Tab drag into another column, workspace new / rename / duplicate / close | next frame / ≤ 100 ms |

Hidden tabs and workspaces keep their model state but release their rendered text; the
budgets above include re-rendering a released tab.

On the Mac the context menu's own work (hit test, core, NSMenu) is held to the frame (≤ 2.2 ms
p95, B077); the menu window's appearance is bounded by AppKit's NSMenu presentation (about
50 ms for an 18-row menu on the Mac mini), accepted as a platform cost of the native menu.

## 8. Lockstep on web and Mac

Each sprint lands on both the web shell (apps/web) and the Mac app (apps/apple) from the same
core model and command registry; a feature is done when both hosts pass its cases and the
budgets. iPad follows when a device is available (B075).

## 9. Out of scope for the baseline

Lemma and morphology queries, cross-workspace link groups, tear-off windows, multi-window
sessions, sync of sessions between devices, research chats, and any content beyond the three
approved editions and the existing fixtures.
