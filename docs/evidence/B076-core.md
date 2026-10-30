# B076 evidence (core part) — the contextual tool well

Recorded 30 October 2026 on the Mac mini (`office-hub`, Apple M1). Work was done in the git
worktree `openlogos-b076` (branch `b076-core-well`, from main 267360c) by a Claude Code
subagent. Nothing was pushed or merged; no simulator, device, keychain or macOS user was
touched. This is the core part only: the web and Mac hosts draw the well in their own B076
parts, on top of this branch.

Owner feedback on the B072 shell: "The tool bar needs to be contextually useful for what it's
looking at." Owner choice: the tool set changes with the focused tab's content, and a text
selection adds an actions group below the tools; ink tools only on reading text. The core
decides the well so web and Mac stay in lockstep (specs/22 §8). Spec: specs/22 §4.1.

## What was built

| Item | Where |
|---|---|
| Context from focus, tab kind and resource language | `crates/workspaces/src/well.rs`: `ToolContext` (`translation`, `original`, `notes`, `reference`, `none`), `tool_context`; `commands::Focus` (`tab`, `notes_dock`, `empty`) as a new `CommandContext.focus`; `Session.resource_languages` (runtime, like `resource_titles`) with `Shell::set_resource_languages` and `shell::is_original_language` (`grc`, `hbo`, `arc`, `he`, `heb`) |
| Well contents from the core | `tool_well(session, context) -> ToolWell` and `Shell::tool_well`; groups `tools`, `ink` (translation, original), `selection` (selection and not `none`) |
| Effective tool | `well::effective_tool`; `ToolWell.effective_tool`; `SessionView.effective_tool` and `tool_context` (for the active tab) |
| Registry | 10 commands appended after `edit.redo` (no id moved or renamed): `tool.word_study`, `tool.type`, `note.bold`, `note.italic`, `note.heading`, `note.list`, `note.link_passage`, `selection.note`, `selection.search`, `selection.copy_reference`; `DisabledReason::WrongContext { what, place }` (kind `wrong_context`) |
| Palette | `PaletteContext.focus`; a `Reference` argument fills inline (`link passage Jn 3:16`); in notes a passage adds "Link John 3:16" under the navigation row |
| Bindings | UniFFI (`crates/ffi/src/native/shell.rs`) and wasm (`crates/ffi/src/wasm.rs`), same shape; `ShellHandle.fixture()` registers the fixture editions' languages |
| Conformance | `crates/conformance/fixtures/tool-well.json` with `crates/conformance/src/well.rs`: 117 cases; `shell-commands` registry ids extended |
| Spec, backlog, validator | specs/22 §4.1 and §6; B076 in specs/backlog.json (B073 now depends on it); session schema `tool` accepts `word_study`, `type`; `tool_well` fixture kind in `tools/validate.py` |
| Speed | `openlogos-shell-bench` row "tool well for the focused tab or dock" |

### Commits

| Commit | Content |
|---|---|
| `cbedc5f` | core: contexts, effective tool, registry, disabled reason, palette focus |
| `73a0221` | `tool-well` fixture and runner, session schema tools, validator |
| `84e3889` | UniFFI and wasm surfaces; Rust, wasm worker, Node, Swift and Kotlin host tests |
| `c070f92` | bench row |
| `8fee677` | specs/22 §4.1, §6 keys; B076 backlog entry |
| `f864784` | host README conformance counts (1337), VALIDATION note |
| this commit | this file |

## API

Rust (`openlogos_workspaces`):

```rust
commands::Focus { Tab (default), NotesDock, Empty }
commands::CommandContext { has_selection: bool, focus: Focus }        // serde default
commands::DisabledReason::WrongContext { what: String, place: String } // "Pen works on Bible text"
Shell::set_resource_languages(&mut self, BTreeMap<String, String>)     // bumps revision
Shell::tool_well(&self, &CommandContext) -> well::ToolWell
well::{tool_context, effective_tool, tool_well, ToolContext, ToolWell, ToolWellGroup,
       ToolWellGroupKind, ToolWellItem, ICONS}
view::SessionView { .., effective_tool: String, tool_context: String, .. }
palette::PaletteContext { .., focus: Focus }
```

JSON shape (wasm returns it as is; UniFFI records carry the same fields, enums as strings):

```
ToolWell      { context, tool, effective_tool, groups: [ToolWellGroup] }
ToolWellGroup { kind: "tools" | "ink" | "selection", items: [ToolWellItem] }
ToolWellItem  { command_id, tool: ShellTool | null, label, key: string | null, icon,
                active, value: string | null, disabled_text: string | null }
```

`key` is the web default, identical on Mac for every well command (a unit test asserts it).
`value` is set on the ink items only (swatch `#RRGGBB`, alternate swatch, brush width). An item
that only lacks its argument (Link passage, Swatch, Brush width) has no `disabled_text`: the host
asks for the value.

| | wasm (`ShellHandle`, TypeScript) | UniFFI (`ShellHandle`, Swift / Kotlin) |
|---|---|---|
| the well | `toolWell(context?: ShellCommandContext): ShellToolWell` | `toolWell(context: ShellCommandContext) -> ShellToolWell` |
| languages | `setResourceLanguages(Record<string, string>)` | `setResourceLanguages(languages: [String: String])` |
| commands with focus | `commands(context)`, `apply(invocation, context)` (context gains `focus?`) | `commandsIn(context:)`, `applyIn(commandId:args:context:)`; `commands(hasSelection:)` / `apply(..hasSelection:)` keep meaning focus on the tab |
| context | `ShellCommandContext { has_selection?, focus?: "tab" \| "notes_dock" \| "empty" }` | `ShellCommandContext(hasSelection:, focus: ShellFocus)`, `ShellFocus.tab / .notesDock / .empty` |
| palette | `ShellPaletteContext.focus?` | `ShellPaletteContext(.., focus: ShellFocus? = nil)` |
| session view | `effective_tool`, `tool_context` | `effectiveTool`, `toolContext` |
| types | `ShellToolWell`, `ShellToolWellGroup`, `ShellToolWellItem`, `ShellToolContext`, `ShellFocus`; `ShellTool` adds `word_study`, `type` | `ShellToolWell`, `ShellToolWellGroup`, `ShellToolWellItem` records |

### Commands and keys

| Command | Title | Key (web = Mac) | Scope | Core performs | Available |
|---|---|---|---|---|---|
| `tool.word_study` | Word study tool | `W` (bare) | session | yes (stores the tool) | original |
| `tool.type` | Type tool | `T` (bare) | session | yes (stores the tool) | notes |
| `note.bold` | Bold | `Cmd+B` | tab | no | notes |
| `note.italic` | Italic | `Cmd+I` | tab | no | notes |
| `note.heading` | Heading | none | tab | no | notes |
| `note.list` | List | none | tab | no | notes |
| `note.link_passage` | Link passage (argument `reference`) | none | tab | no | notes, with a passage |
| `selection.note` | Note on selection | none | selection | no | a selection |
| `selection.search` | Search selection | `Cmd+E` | selection | no | a selection |
| `selection.copy_reference` | Copy reference | none | selection | no | a selection |

None is undoable. Tool commands behave as the existing six do: not undoable, they bump the
revision. The existing tool commands are now also context-bound (Pen refused in notes, etc.).

### Icon names

`select`, `word-study`, `highlight`, `pen`, `highlighter-pen`, `eraser`, `collect`, `type`,
`bold`, `italic`, `heading`, `list`, `link-passage`, `swatch`, `swatch-alternate`,
`brush-width`, `highlight-now`, `note`, `search`, `copy-reference`, `study`.

## The well per context

From `ShellHandle.fixture()` through the wasm build (`apps/web/core-host/pkg`): the
translation tab (column 1, BSB), the Greek tab with a selection (column 2, SBLGNT), the lexicon
(column 3), the notes dock after choosing Pen on the BSB tab (the stored tool stays `pen`, Type
is in effect), and an empty column with a selection (no selection group).

#### translation
```json
{ "context": "translation", "tool": "select", "effective_tool": "select", "groups": [
  { "kind": "tools", "items": [
    {"command_id": "tool.select", "tool": "select", "label": "Select", "key": "V", "icon": "select", "active": true, "value": null, "disabled_text": null},
    {"command_id": "tool.highlight", "tool": "highlight", "label": "Highlight", "key": "H", "icon": "highlight", "active": false, "value": null, "disabled_text": null},
    {"command_id": "tool.pen", "tool": "pen", "label": "Pen", "key": "P", "icon": "pen", "active": false, "value": null, "disabled_text": null},
    {"command_id": "tool.highlighter_pen", "tool": "highlighter_pen", "label": "Highlighter pen", "key": "M", "icon": "highlighter-pen", "active": false, "value": null, "disabled_text": null},
    {"command_id": "tool.eraser", "tool": "eraser", "label": "Eraser", "key": "E", "icon": "eraser", "active": false, "value": null, "disabled_text": null},
    {"command_id": "tool.collect", "tool": "collect", "label": "Collect", "key": "C", "icon": "collect", "active": false, "value": null, "disabled_text": null}
  ] },
  { "kind": "ink", "items": [
    {"command_id": "tool.swatch", "tool": null, "label": "Swatch", "key": null, "icon": "swatch", "active": false, "value": "#F5D90A", "disabled_text": null},
    {"command_id": "tool.swap_swatches", "tool": null, "label": "Alternate swatch", "key": "X", "icon": "swatch-alternate", "active": false, "value": "#4CC38A", "disabled_text": null},
    {"command_id": "tool.width", "tool": null, "label": "Brush width", "key": null, "icon": "brush-width", "active": false, "value": "4", "disabled_text": null}
  ] }
] }
```

#### original_with_selection
```json
{ "context": "original", "tool": "select", "effective_tool": "select", "groups": [
  { "kind": "tools", "items": [
    {"command_id": "tool.select", "tool": "select", "label": "Select", "key": "V", "icon": "select", "active": true, "value": null, "disabled_text": null},
    {"command_id": "tool.word_study", "tool": "word_study", "label": "Word study", "key": "W", "icon": "word-study", "active": false, "value": null, "disabled_text": null},
    {"command_id": "tool.highlight", "tool": "highlight", "label": "Highlight", "key": "H", "icon": "highlight", "active": false, "value": null, "disabled_text": null},
    {"command_id": "tool.pen", "tool": "pen", "label": "Pen", "key": "P", "icon": "pen", "active": false, "value": null, "disabled_text": null},
    {"command_id": "tool.highlighter_pen", "tool": "highlighter_pen", "label": "Highlighter pen", "key": "M", "icon": "highlighter-pen", "active": false, "value": null, "disabled_text": null},
    {"command_id": "tool.eraser", "tool": "eraser", "label": "Eraser", "key": "E", "icon": "eraser", "active": false, "value": null, "disabled_text": null},
    {"command_id": "tool.collect", "tool": "collect", "label": "Collect", "key": "C", "icon": "collect", "active": false, "value": null, "disabled_text": null}
  ] },
  { "kind": "ink", "items": [
    {"command_id": "tool.swatch", "tool": null, "label": "Swatch", "key": null, "icon": "swatch", "active": false, "value": "#F5D90A", "disabled_text": null},
    {"command_id": "tool.swap_swatches", "tool": null, "label": "Alternate swatch", "key": "X", "icon": "swatch-alternate", "active": false, "value": "#4CC38A", "disabled_text": null},
    {"command_id": "tool.width", "tool": null, "label": "Brush width", "key": null, "icon": "brush-width", "active": false, "value": "4", "disabled_text": null}
  ] },
  { "kind": "selection", "items": [
    {"command_id": "selection.highlight", "tool": null, "label": "Highlight now", "key": "Cmd+Shift+H", "icon": "highlight-now", "active": false, "value": null, "disabled_text": null},
    {"command_id": "selection.collect", "tool": null, "label": "Collect", "key": "Cmd+Shift+C", "icon": "collect", "active": false, "value": null, "disabled_text": null},
    {"command_id": "selection.note", "tool": null, "label": "Note on it", "key": null, "icon": "note", "active": false, "value": null, "disabled_text": null},
    {"command_id": "selection.search", "tool": null, "label": "Search this", "key": "Cmd+E", "icon": "search", "active": false, "value": null, "disabled_text": null},
    {"command_id": "selection.copy_reference", "tool": null, "label": "Copy reference", "key": null, "icon": "copy-reference", "active": false, "value": null, "disabled_text": null},
    {"command_id": "selection.study", "tool": null, "label": "Study", "key": "Cmd+Shift+S", "icon": "study", "active": false, "value": null, "disabled_text": null}
  ] }
] }
```

#### reference
```json
{ "context": "reference", "tool": "select", "effective_tool": "select", "groups": [
  { "kind": "tools", "items": [
    {"command_id": "tool.select", "tool": "select", "label": "Select", "key": "V", "icon": "select", "active": true, "value": null, "disabled_text": null},
    {"command_id": "tool.highlight", "tool": "highlight", "label": "Highlight", "key": "H", "icon": "highlight", "active": false, "value": null, "disabled_text": null},
    {"command_id": "tool.collect", "tool": "collect", "label": "Collect", "key": "C", "icon": "collect", "active": false, "value": null, "disabled_text": null}
  ] }
] }
```

#### notes_dock_pen_stored
```json
{ "context": "notes", "tool": "pen", "effective_tool": "type", "groups": [
  { "kind": "tools", "items": [
    {"command_id": "tool.type", "tool": "type", "label": "Type", "key": "T", "icon": "type", "active": true, "value": null, "disabled_text": null},
    {"command_id": "note.bold", "tool": null, "label": "Bold", "key": "Cmd+B", "icon": "bold", "active": false, "value": null, "disabled_text": null},
    {"command_id": "note.italic", "tool": null, "label": "Italic", "key": "Cmd+I", "icon": "italic", "active": false, "value": null, "disabled_text": null},
    {"command_id": "note.heading", "tool": null, "label": "Heading", "key": null, "icon": "heading", "active": false, "value": null, "disabled_text": null},
    {"command_id": "note.list", "tool": null, "label": "List", "key": null, "icon": "list", "active": false, "value": null, "disabled_text": null},
    {"command_id": "note.link_passage", "tool": null, "label": "Link passage", "key": null, "icon": "link-passage", "active": false, "value": null, "disabled_text": null},
    {"command_id": "tool.collect", "tool": "collect", "label": "Collect", "key": "C", "icon": "collect", "active": false, "value": null, "disabled_text": null}
  ] }
] }
```

#### none
```json
{ "context": "none", "tool": "pen", "effective_tool": "select", "groups": [
  { "kind": "tools", "items": [
    {"command_id": "tool.select", "tool": "select", "label": "Select", "key": "V", "icon": "select", "active": true, "value": null, "disabled_text": null}
  ] }
] }
```

## Conformance

`tool-well` (117 cases) runs through `run_all` on every host: 16 well cases for the contexts
(translation; original Greek and Hebrew; notes tab and dock; lexicon and research; none), each
with and without a selection; 11 fallback and restore cases (Pen, Word study, Type, Collect,
Highlight across contexts, with `Shell::view`'s effective tool); 7 cases of refused commands and
reason texts per context; 2 cases of the note and selection actions handing off
(`handled: false`) or failing for a missing passage or selection; 12 palette queries (Link
passage row in notes only and never with a column target, inline passage argument, disabled
tool and note rows with their words and shortcuts). Four cases compare the whole well JSON,
labels, keys, icons and values included. All reasons are checked to be words (no `_`).
Total conformance: 1337 cases (`shell-commands` 261, `tool-well` 117).

## Bench (`just bench-shell`, release, 100 rounds)

The well is built after each column focus, alternating the tab and the notes dock with a
selection; budget is the column-focus frame.

| interaction | specs/22 §7 budget | samples | p50 µs | p95 µs | max µs |
|---|---:|---:|---:|---:|---:|
| column focus (column.focus N) | 16 ms | 300 | 0.5 | 0.7 | 1.3 |
| tool well for the focused tab or dock (Shell::tool_well) | 16 ms | 600 | 7.7 | 8.4 | 35.9 |
| tool change (tool.*) | 16 ms | 300 | 0.8 | 1 | 2 |

Other rows of the same run: palette keystroke p95 265.5 µs, session view (120 tabs) p95
96.4 µs, workspace switch p95 0.5 µs.

## Lanes (committed tree 8fee677; f864784 and this file change documentation only)

| Command | Result |
|---|---|
| `nix develop .#core -c just ci` | exit 0. `Summary [   6.362s] 306 tests run: 306 passed, 0 skipped`; clippy and `cargo check --target wasm32-unknown-unknown` clean |
| `nix develop .#core -c just test-swift` | exit 0. `Executed 8 tests, with 0 failures (0 unexpected)`; `testToolWellFollowsFocus` passed; `conformance cases: 1337 passed, 0 failed` |
| `nix develop .#android -c just test-kotlin` (test-results cleared first) | exit 0. `CoreHostTest` 8 tests, 0 failures; `toolWellFollowsFocus() PASSED`; `conformance cases: 1337 passed, 0 failed`; `BUILD SUCCESSFUL` |
| `nix develop .#web -c just test-web` | exit 0. `tests 8, pass 8, fail 0`; `conformance cases: 1337 passed, 0 failed (api 0.1.2)` |
| `nix develop .#web -c just test-wasm` | exit 0. `running 7 tests`, `shell_handle_returns_the_tool_well ... ok`, `conformance_suite_passes ... ok`, `test result: ok. 7 passed; 0 failed` |
| `nix develop .#core -c just bench-shell` | exit 0; table above |
| `nix develop .#core -c python3 tools/validate.py` | PASS: 33 schema/example validations; 76 acyclic tasks; 19 conformance fixtures; 125 negative tests |

`just test-kotlin` needs gradle from `.#android`, and `just test-web`/`test-wasm` need wasm-pack
from `.#web`; under `.#core` they fail at the tool lookup (exit 127 and 1), not in a test.

## Decisions to review

1. **Tool changes are not `quiet`.** The brief said tool changes are quiet; the existing six
   tools bump the revision (B070's test asserts it) and the hosts re-render on the revision, so
   the two new tools do the same. Not undoable, as before.
2. **Focus is host-supplied.** The session knows the active column, not whether the notes dock
   or an empty column has focus, so `CommandContext` gains `focus`. A column is never empty in
   the model (the schema requires a resource), so `none` arises only when the host passes
   `focus: "empty"`.
3. **Original language by registered tag.** Hosts register BCP 47 tags per resource, as they
   register titles. `he`/`heb` count as original because WLC is often published under them;
   a modern Hebrew translation tagged `he` would show the original-language tools. `el`
   (modern Greek) does not count.
4. **Select and Highlight are refused in notes** ("Select works outside notes"); notes default
   to Type. Collect works everywhere but `none`.
5. **Keys.** ⌘B and ⌘I for Bold and Italic (only while the note editor has focus); `W` and `T`
   bare for Word study and Type; ⌘E for Search selection (the Mac "use selection for find"
   convention; in a browser on Windows/Linux it maps to Ctrl+E, which the page must claim).
   Heading, List, Link passage, Note on it and Copy reference have no default key to avoid
   browser and registry collisions (⌘⇧N is New workspace on Mac, ⌘⇧R reloads in browsers).
6. **The width item dispatches `tool.width`** (host shows the widths) rather than
   `tool.width_up`, so it is never disabled at the widest width.
7. **The selection group in notes** includes Highlight now and Study, as the brief lists it
   for every context but `none`.
8. **New commands are appended** after `edit.redo`, so registry order is not grouped by area.
9. **Inline passage arguments for every `Reference` command**, so `go to passage Jn 3:16` also
   gets a Commands row with the passage; no existing palette case changed.
10. **`SessionView.effective_tool` is for the active tab**; with the dock focused the host
    reads `tool_well(..).effective_tool`.
11. **`ShellHandle.fixture()` registers `bsb en, sblgnt grc, wlc hbo`** without bumping the
    revision, so previews show the Greek tools on column 2 and host tests that count
    revisions are unaffected.

## For the host parts

- Call `setResourceLanguages` next to `setResourceTitles` with the installed editions'
  languages, or every Reading tab is a translation.
- Pass `focus` (`notes_dock` when the dock's editor has focus) to `toolWell`, `apply` /
  `applyIn`, `commands` / `commandsIn` and the palette; a bare-key tool press in the wrong
  context now throws `wrong_context` with words to show.
- Draw the well from `groups`; mark `active`; map `icon` to SF Symbols / inline SVG.
- Perform `note.*` and `selection.note`, `selection.search`, `selection.copy_reference`
  (`handled: false`), and ask for the passage when Link passage is clicked.

## Skipped

No host UI (the host agents' part). No screenshots: nothing renders in the core part.

## Follow-up: `tab.open_lexicon` for word study results

Commit `6611331` adds one core command so hosts can open Word study results.

| | |
|---|---|
| Command | `tab.open_lexicon`, "Open in lexicon", scope workspace, argument `text` (the word as clicked) with optional `reference` and `column` or `new_column`; undoable like `tab.new`; performed by the core |
| Target | `column` if given, else the column right of the active one, else a new column after the active one (also with `new_column`) |
| Reuse | a Lexicon tab already in the target column (the shown one first) gets the new word and passage (scroll reset), is pinned and is shown |
| New tab | a pinned Lexicon tab at the passage given or the active tab's, with the content id and revision of an existing lexicon tab in the workspace, else `lexicon` (`commands::DEFAULT_LEXICON_CONTENT_ID`, for the host to resolve) |
| Focus | stays on the column where the word was clicked (amended); the lexicon tab is the shown tab of its column |
| Word on the tab | `Tab.lexicon_word` (optional `lexicon_word` in workspace-2.schema.json, absent on other tabs); `TabView.lexicon_word`, UniFFI `ShellTab.lexiconWord`, wasm `ShellTabView.lexicon_word` / `ShellTab.lexicon_word?` (snake_case like every wasm field) |
| Disabled | without a word: `needs_argument`, "Needs a word to look up"; a column that does not exist: "There is no column N" |
| Palette | `open in lexicon agape` gives "Open in lexicon" with `{text: "agape"}`; `>open in lexicon` lists it disabled with "Needs a word to look up" |

Conformance: a `shell-commands` scenario "open in lexicon" (new tab in the column to the
right, reuse of the lexicon tab in column 3 (pinned, follow mode back on undo), two undos, a new column,
a new column from the last column, the two refusals, undo) checks columns, focus, passages,
link modes and the view's `lexicon_word`; two palette cases. Total 1370 cases
(`shell-commands` 294). Host assertions added in Rust (UniFFI), Node, Swift and Kotlin.

Lanes on `6611331`: `nix develop .#core -c just ci` exit 0, `Summary [   6.818s] 306 tests run:
306 passed, 0 skipped`; `just test-swift` exit 0, `Executed 8 tests, with 0 failures (0
unexpected)`, `conformance cases: 1370 passed, 0 failed`; `nix develop .#android -c just
test-kotlin` (test-results cleared) exit 0, 8 tests 0 failures, `BUILD SUCCESSFUL`, 1370
passed; `nix develop .#web -c just test-web` exit 0, tests 8 pass 8, 1370 passed; `nix develop
.#web -c just test-wasm` exit 0, `7 passed; 0 failed`; validate.py PASS. Bench unaffected:
tool well p50 8 µs, p95 9.5 µs; column focus p95 0.8 µs.

Amended 30 October 2026 (lead, agreed with the web host): focus stays on the column where the word was clicked, and a reused lexicon tab is pinned too. The lanes above ran before the amendment; the amended run is at the end of this file.

Decisions to review: a new or reused lexicon tab is pinned (the looked-up word keeps its
passage when the reader navigates); the word does not enter the tab's
Back history; a workspace without any lexicon tab opens content id `lexicon`.
