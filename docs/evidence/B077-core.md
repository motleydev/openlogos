# B077 evidence (core part) — the OpenLogos context menu

Recorded 3 November 2026 on the Mac mini (`office-hub`, Apple M1). Work was done in the git
worktree `openlogos-b077` (branch `b077-core-menu`, from `b076-core-well` 3702fc5, which is in
PR #26 and not merged yet) by a Claude Code subagent. Nothing was pushed or merged; no
simulator, device, keychain or macOS user was touched. This is the core part only: the web
and Mac hosts draw the menu in their own B077 parts, on top of this branch.

Owner direction (2026-11-03): "I want us to override right-click context menu when on web and
native app. It should give us power features available like word searches and things of the
like." As with the tool well, the core decides the menu for what was right-clicked, so web
and Mac stay in lockstep (specs/22 §8); hosts draw it and perform host-side effects. Spec:
specs/22 §4.2.

## What was built

| Item | Where |
|---|---|
| Targets, menu shape, focus-first rule | `crates/workspaces/src/menu.rs`: `ContextTarget` (`word`, `selection`, `note_text`, `note_link`, `tab`, `column`, `background`), `ContextMenu`, `ContextMenuSection`, `ContextMenuItem`, `MENU_ICONS`, `HIGHLIGHT_SWATCHES`; `context_menu(shell, target, context)` and `Shell::context_menu` |
| Registry | 14 commands appended after `tab.open_lexicon` (no id moved or renamed); `CommandArgs` gains `scope` and `resource_id`; `DisabledReason::Unavailable { needs }` (kind `unavailable`, "Needs the lexicon import"); `commands::SEARCH_SCOPES` |
| Core-performed commands | `tab.open_edition` (compare; undoable) and `tab.close_others` (undoable), sharing `side_column` with `tab.open_lexicon` |
| Well | a note action the well does not show (`note.remove_link`) works in notes and its wrong-context reason uses its title |
| Palette | `palette::human_reference` for labels; `concrete_shortcut` shared with the menu (`Alt+Shift+{n}` filled per column) |
| Bindings | UniFFI (`crates/ffi/src/native/shell.rs`) and wasm (`crates/ffi/src/wasm.rs`), same shape |
| Conformance | `crates/conformance/fixtures/context-menu.json` with `crates/conformance/src/menu.rs`: 223 cases; `shell-commands` registry ids extended |
| Spec, backlog, validator | specs/22 §4.2, §6 and §7; B077 in specs/backlog.json (B073 now depends on it); `context_menu` fixture kind in `tools/validate.py` with the fixture's session validated |
| Speed | `openlogos-shell-bench` row "context menu for every target kind" |

### Commits

| Commit | Content |
|---|---|
| `26bfb39` | core: menu module, registry, `tab.open_edition`, `tab.close_others`, unit tests |
| `3f0a447` | `context-menu` fixture and runner, `shell-commands` registry ids and one palette row, validator kind |
| `a784b37` | UniFFI and wasm surfaces; Rust, wasm worker, Node, Swift and Kotlin host tests; bench row |
| `ebd5258` | Kotlin test import (the first Kotlin run failed to compile without it) |
| `d1b72e3` | specs/22 §4.2, §6, §7; backlog B077; README conformance counts (1594); VALIDATION note |
| this commit | this file |

## API

Rust (`openlogos_workspaces`):

```rust
menu::ContextTarget            // serde tag "kind": word | selection | note_text | note_link | tab | column | background
menu::ContextTargetKind        // ALL, name()
menu::ContextMenu { target_kind, title: Option<String>, focus: Option<Invocation>,
                    context: CommandContext, sections: Vec<ContextMenuSection> }
menu::ContextMenuSection { label: Option<String>, items: Vec<ContextMenuItem> }
menu::ContextMenuItem { command_id, args: CommandArgs, label, key: Option<String>, icon,
                        active: bool, value: Option<String>, disabled_text: Option<String>,
                        submenu: Vec<ContextMenuItem> }       // one level; omitted when empty
menu::context_menu(&Shell, &ContextTarget, &CommandContext) -> Result<ContextMenu, DisabledReason>
Shell::context_menu(&self, &ContextTarget, &CommandContext) -> Result<ContextMenu, DisabledReason>
menu::{MENU_ICONS, HIGHLIGHT_SWATCHES}
commands::CommandArgs { .., scope: Option<String>, resource_id: Option<String> }
commands::DisabledReason::Unavailable { needs }               // "Needs the lexicon import"
commands::SEARCH_SCOPES = ["edition", "all", "workspace", "chapter"]
palette::human_reference(&str) -> Option<String>              // "John 3:16"
```

Targets (JSON as the host passes it; every field but the kind's required one is optional):

```
{ kind: "word", text, reference?, segment_id?, scalar_start?, scalar_end?, resource_id?, tab_id? }
{ kind: "selection", text, reference?, end_reference?, resource_id?, tab_id? }
{ kind: "note_text", text?, tab_id? }          // tab_id absent: the notes dock
{ kind: "note_link", reference, tab_id? }
{ kind: "tab", tab_id }
{ kind: "column", column }
{ kind: "background", column? }
```

The menu (wasm returns it as is; UniFFI records carry the same fields):

```
ContextMenu        { target_kind, title: string | null, focus: { command_id, args } | null,
                     context: { has_selection, focus }, sections: [ContextMenuSection] }
ContextMenuSection { label: string | null, items: [ContextMenuItem] }
ContextMenuItem    { command_id, args, label, key: string | null, icon, active,
                     value: string | null, disabled_text: string | null, submenu?: [ContextMenuItem] }
```

How a host uses it: call it on right-click (or the keyboard's menu key) with the target and
`has_selection`; draw `sections` (separators between them, `label` optional), `submenu` as a
one-level submenu, `active` as a check mark, `value` as a swatch dot, `disabled_text` as the
disabled row's reason (tooltip or secondary text). On a click: `apply(focus, context)` if
`focus` is set, then `apply({command_id, args}, context)`. `handled: false` means the host
performs it.

| | wasm (`ShellHandle`, TypeScript) | UniFFI (`ShellHandle`, Swift / Kotlin) |
|---|---|---|
| the menu | `contextMenu(target: ShellContextTarget, context?: ShellCommandContext): ShellContextMenu` | `contextMenu(target: ShellContextTarget, context: ShellCommandContext) throws -> ShellContextMenu` |
| target | `ShellContextTarget` (tagged union on `kind`, snake_case fields) | `ShellContextTarget` enum: `.word(text:reference:segmentId:scalarStart:scalarEnd:resourceId:tabId:)`, `.selection(..)`, `.noteText(text:tabId:)`, `.noteLink(reference:tabId:)`, `.tab(tabId:)`, `.column(column:)`, `.background(column:)` |
| records | `ShellContextMenu`, `ShellContextMenuSection`, `ShellContextMenuItem` (`submenu?`) | `ShellContextMenu` (`targetKind`, `title`, `focus: ShellInvocation?`, `context: ShellCommandContext`), `ShellContextMenuSection`, `ShellContextMenuItem` (`submenu: [ShellContextMenuSubitem]`), `ShellInvocation { commandId, args }` |
| a gone target | throws an `Error` with `kind` `no_such_tab` / `no_such_column`, `message` "That tab is not open" / "There is no column 9" | throws `ShellCommandError.InvalidDocument(detail: "context menu target: …")` (no new error case, so the Mac app's exhaustive switch still compiles) |
| arguments | `ShellCommandArgs.scope?`, `resource_id?` | `ShellCommandArgs(scope:, resourceId:)` (default `nil`) |

### Commands

| Command | Title | Argument | Scope | Performed by | Undoable | Key |
|---|---|---|---|---|---|---|
| `search.text` | Search text | `text`, `scope`, `resource_id`, `reference` | session | host | no | none |
| `search.lemma` | Search lemma | `text` | session | host; always disabled "Needs the lexicon import" | no | none |
| `nav.cross_references` | Cross references | `reference` | tab | host; always disabled "Needs cross-reference data" | no | none |
| `tab.open_edition` | Open in edition | `resource_id`, `reference`, `column` / `new_column` | workspace | core | yes | none |
| `tab.close_others` | Close other tabs | none | column | core | yes | none |
| `copy.text` | Copy text | `text` | session | host | no | none |
| `copy.reference` | Copy passage reference | `reference` | session | host | no | none |
| `copy.passage` | Copy passage with reference | `reference`, `text` or `resource_id` | session | host | no | none |
| `host.cut` | Cut | none (needs a selection) | selection | host | no | none in the registry; the menu shows `Cmd+X` |
| `host.copy` | Copy | none (needs a selection) | selection | host | no | none; menu `Cmd+C` |
| `host.paste` | Paste | none | session | host | no | none; menu `Cmd+V` |
| `nav.paste_passage` | Paste passage | none | tab | host | no | none |
| `selection.highlight_in` | Highlight in colour | `text` (swatch) | selection | host | no | none |
| `note.remove_link` | Remove passage link | `reference` | tab (notes only) | host | no | none |

Disabled words added: "Needs words to search for" (`search.text` without text), "Choose an
edition" (`tab.open_edition` without `resource_id`), "A search scope is one of edition, all,
workspace, chapter", and the two `Unavailable` texts. The menu adds three of its own: "Word
study works on Greek and Hebrew text" (translation), "No other edition is installed",
"Paste a passage into a reading tab".

### Icon names

Beyond the well's: `search-all`, `lemma`, `find`, `compare`, `open`, `column`, `new-column`,
`cross-references`, `copy`, `copy-passage`, `cut`, `paste`, `remove-link`, `close`,
`close-others`, `duplicate`, `move`, `pin`, `follow`, `link`, `focus`, `move-left`,
`move-right`, `new-tab`, `equalize`, `close-column`, `paste-passage`, `text-size`,
`text-larger`, `text-smaller`, `text-reset`, `theme`. The menu also uses the well's
`search`, `word-study`, `highlight`, `swatch`, `collect`, `note`, `study`, `copy-reference`,
`bold`, `italic`, `link-passage`. All are in `menu::MENU_ICONS`.

## The menu per target

From `ShellHandle.fixture()` through the wasm build (`apps/web/core-host/pkg`) with titles
`bsb BSB, sblgnt SBLGNT, wlc WLC, fixture:lexicon-1 Lexicon` registered: a word in the BSB tab
(column 1, active), a word in the SBLGNT tab (column 2, so `focus` is set), a selection over
John 3:16–17, a selection in the notes dock, a passage link in the dock, the hidden pinned tab
`t2`, column 2, and the background.

#### word_translation
```json
{ "target_kind": "word", "title": "loved", "focus": null, "context": {"has_selection":true,"focus":"tab"}, "sections": [
  { "label": "Search", "items": [
    {"command_id":"search.text","args":{"text":"loved","scope":"edition","resource_id":"bsb"},"label":"Search “loved” in this edition","key":null,"icon":"search","active":false,"value":null,"disabled_text":null},
    {"command_id":"search.text","args":{"text":"loved","scope":"all"},"label":"Search “loved” in all editions","key":null,"icon":"search-all","active":false,"value":null,"disabled_text":null},
    {"command_id":"search.lemma","args":{"text":"loved"},"label":"Search lemma","key":null,"icon":"lemma","active":false,"value":null,"disabled_text":"Needs the lexicon import"},
    {"command_id":"tab.open_lexicon","args":{"text":"loved","reference":"John.3.16"},"label":"Word study","key":null,"icon":"word-study","active":false,"value":null,"disabled_text":"Word study works on Greek and Hebrew text"},
    {"command_id":"search.text","args":{"text":"loved","reference":"John.3","scope":"chapter","resource_id":"bsb"},"label":"Find in this chapter","key":null,"icon":"find","active":false,"value":null,"disabled_text":null}
  ] },
  { "label": "Open", "items": [
    {"command_id":"tab.open_edition","args":{"reference":"John.3.16","resource_id":"sblgnt"},"label":"Compare in SBLGNT","key":null,"icon":"compare","active":false,"value":null,"disabled_text":null},
    {"command_id":"nav.navigate","args":{"column":2,"reference":"John.3.16"},"label":"Open John 3:16 in","key":null,"icon":"open","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"nav.navigate","args":{"column":2,"reference":"John.3.16"},"label":"Column 2 · SBLGNT","key":null,"icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"nav.navigate","args":{"column":3,"reference":"John.3.16"},"label":"Column 3 · Lexicon","key":null,"icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"nav.navigate","args":{"new_column":true,"reference":"John.3.16"},"label":"New column","key":null,"icon":"new-column","active":false,"value":null,"disabled_text":null}
    ] },
    {"command_id":"nav.cross_references","args":{"reference":"John.3.16"},"label":"Cross references","key":null,"icon":"cross-references","active":false,"value":null,"disabled_text":"Needs cross-reference data"}
  ] },
  { "label": "Mark", "items": [
    {"command_id":"selection.highlight_in","args":{"text":"#F5D90A"},"label":"Highlight","key":null,"icon":"highlight","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"selection.highlight_in","args":{"text":"#F5D90A"},"label":"Yellow","key":null,"icon":"swatch","active":true,"value":"#F5D90A","disabled_text":null},
      {"command_id":"selection.highlight_in","args":{"text":"#4CC38A"},"label":"Green","key":null,"icon":"swatch","active":false,"value":"#4CC38A","disabled_text":null},
      {"command_id":"selection.highlight_in","args":{"text":"#5B9BFF"},"label":"Blue","key":null,"icon":"swatch","active":false,"value":"#5B9BFF","disabled_text":null},
      {"command_id":"selection.highlight_in","args":{"text":"#F25C9A"},"label":"Pink","key":null,"icon":"swatch","active":false,"value":"#F25C9A","disabled_text":null}
    ] },
    {"command_id":"selection.collect","args":{},"label":"Collect","key":"Cmd+Shift+C","icon":"collect","active":false,"value":null,"disabled_text":null},
    {"command_id":"selection.note","args":{},"label":"Note on it","key":null,"icon":"note","active":false,"value":null,"disabled_text":null}
  ] },
  { "label": "Copy", "items": [
    {"command_id":"copy.text","args":{"text":"loved"},"label":"Copy","key":null,"icon":"copy","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"copy.text","args":{"text":"loved"},"label":"Copy word","key":null,"icon":"copy","active":false,"value":null,"disabled_text":null},
      {"command_id":"copy.reference","args":{"reference":"John.3.16"},"label":"Copy reference","key":null,"icon":"copy-reference","active":false,"value":null,"disabled_text":null},
      {"command_id":"copy.passage","args":{"reference":"John.3.16","resource_id":"bsb"},"label":"Copy verse with reference","key":null,"icon":"copy-passage","active":false,"value":null,"disabled_text":null}
    ] }
  ] }
] }
```

#### word_greek
```json
{ "target_kind": "word", "title": "ἠγάπησεν", "focus": {"command_id":"tab.focus","args":{"tab_id":"t3"}}, "context": {"has_selection":true,"focus":"tab"}, "sections": [
  { "label": "Search", "items": [
    {"command_id":"search.text","args":{"text":"ἠγάπησεν","scope":"edition","resource_id":"sblgnt"},"label":"Search “ἠγάπησεν” in this edition","key":null,"icon":"search","active":false,"value":null,"disabled_text":null},
    {"command_id":"search.text","args":{"text":"ἠγάπησεν","scope":"all"},"label":"Search “ἠγάπησεν” in all editions","key":null,"icon":"search-all","active":false,"value":null,"disabled_text":null},
    {"command_id":"search.lemma","args":{"text":"ἠγάπησεν"},"label":"Search lemma","key":null,"icon":"lemma","active":false,"value":null,"disabled_text":"Needs the lexicon import"},
    {"command_id":"tab.open_lexicon","args":{"text":"ἠγάπησεν","reference":"John.3.16"},"label":"Word study","key":null,"icon":"word-study","active":false,"value":null,"disabled_text":null},
    {"command_id":"search.text","args":{"text":"ἠγάπησεν","reference":"John.3","scope":"chapter","resource_id":"sblgnt"},"label":"Find in this chapter","key":null,"icon":"find","active":false,"value":null,"disabled_text":null}
  ] },
  { "label": "Open", "items": [
    {"command_id":"tab.open_edition","args":{"reference":"John.3.16","resource_id":"bsb"},"label":"Compare in BSB","key":null,"icon":"compare","active":false,"value":null,"disabled_text":null},
    {"command_id":"nav.navigate","args":{"column":1,"reference":"John.3.16"},"label":"Open John 3:16 in","key":null,"icon":"open","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"nav.navigate","args":{"column":1,"reference":"John.3.16"},"label":"Column 1 · BSB","key":null,"icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"nav.navigate","args":{"column":3,"reference":"John.3.16"},"label":"Column 3 · Lexicon","key":null,"icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"nav.navigate","args":{"new_column":true,"reference":"John.3.16"},"label":"New column","key":null,"icon":"new-column","active":false,"value":null,"disabled_text":null}
    ] },
    {"command_id":"nav.cross_references","args":{"reference":"John.3.16"},"label":"Cross references","key":null,"icon":"cross-references","active":false,"value":null,"disabled_text":"Needs cross-reference data"}
  ] },
  { "label": "Mark", "items": [
    {"command_id":"selection.highlight_in","args":{"text":"#F5D90A"},"label":"Highlight","key":null,"icon":"highlight","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"selection.highlight_in","args":{"text":"#F5D90A"},"label":"Yellow","key":null,"icon":"swatch","active":true,"value":"#F5D90A","disabled_text":null},
      {"command_id":"selection.highlight_in","args":{"text":"#4CC38A"},"label":"Green","key":null,"icon":"swatch","active":false,"value":"#4CC38A","disabled_text":null},
      {"command_id":"selection.highlight_in","args":{"text":"#5B9BFF"},"label":"Blue","key":null,"icon":"swatch","active":false,"value":"#5B9BFF","disabled_text":null},
      {"command_id":"selection.highlight_in","args":{"text":"#F25C9A"},"label":"Pink","key":null,"icon":"swatch","active":false,"value":"#F25C9A","disabled_text":null}
    ] },
    {"command_id":"selection.collect","args":{},"label":"Collect","key":"Cmd+Shift+C","icon":"collect","active":false,"value":null,"disabled_text":null},
    {"command_id":"selection.note","args":{},"label":"Note on it","key":null,"icon":"note","active":false,"value":null,"disabled_text":null}
  ] },
  { "label": "Copy", "items": [
    {"command_id":"copy.text","args":{"text":"ἠγάπησεν"},"label":"Copy","key":null,"icon":"copy","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"copy.text","args":{"text":"ἠγάπησεν"},"label":"Copy word","key":null,"icon":"copy","active":false,"value":null,"disabled_text":null},
      {"command_id":"copy.reference","args":{"reference":"John.3.16"},"label":"Copy reference","key":null,"icon":"copy-reference","active":false,"value":null,"disabled_text":null},
      {"command_id":"copy.passage","args":{"reference":"John.3.16","resource_id":"sblgnt"},"label":"Copy verse with reference","key":null,"icon":"copy-passage","active":false,"value":null,"disabled_text":null}
    ] }
  ] }
] }
```

#### selection
```json
{ "target_kind": "selection", "title": "“For God so loved the wo…”", "focus": null, "context": {"has_selection":true,"focus":"tab"}, "sections": [
  { "label": "Search", "items": [
    {"command_id":"search.text","args":{"text":"For God so loved the world","scope":"edition","resource_id":"bsb"},"label":"Search selection in this edition","key":null,"icon":"search","active":false,"value":null,"disabled_text":null},
    {"command_id":"search.text","args":{"text":"For God so loved the world","scope":"all"},"label":"Search selection in all editions","key":null,"icon":"search-all","active":false,"value":null,"disabled_text":null}
  ] },
  { "label": "Open", "items": [
    {"command_id":"tab.open_edition","args":{"reference":"John.3.16","resource_id":"sblgnt"},"label":"Compare in SBLGNT","key":null,"icon":"compare","active":false,"value":null,"disabled_text":null},
    {"command_id":"nav.navigate","args":{"column":2,"reference":"John.3.16"},"label":"Open John 3:16 in","key":null,"icon":"open","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"nav.navigate","args":{"column":2,"reference":"John.3.16"},"label":"Column 2 · SBLGNT","key":null,"icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"nav.navigate","args":{"column":3,"reference":"John.3.16"},"label":"Column 3 · Lexicon","key":null,"icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"nav.navigate","args":{"new_column":true,"reference":"John.3.16"},"label":"New column","key":null,"icon":"new-column","active":false,"value":null,"disabled_text":null}
    ] }
  ] },
  { "label": "Mark", "items": [
    {"command_id":"selection.highlight_in","args":{"text":"#F5D90A"},"label":"Highlight","key":null,"icon":"highlight","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"selection.highlight_in","args":{"text":"#F5D90A"},"label":"Yellow","key":null,"icon":"swatch","active":true,"value":"#F5D90A","disabled_text":null},
      {"command_id":"selection.highlight_in","args":{"text":"#4CC38A"},"label":"Green","key":null,"icon":"swatch","active":false,"value":"#4CC38A","disabled_text":null},
      {"command_id":"selection.highlight_in","args":{"text":"#5B9BFF"},"label":"Blue","key":null,"icon":"swatch","active":false,"value":"#5B9BFF","disabled_text":null},
      {"command_id":"selection.highlight_in","args":{"text":"#F25C9A"},"label":"Pink","key":null,"icon":"swatch","active":false,"value":"#F25C9A","disabled_text":null}
    ] },
    {"command_id":"selection.collect","args":{},"label":"Collect","key":"Cmd+Shift+C","icon":"collect","active":false,"value":null,"disabled_text":null},
    {"command_id":"selection.note","args":{},"label":"Note on it","key":null,"icon":"note","active":false,"value":null,"disabled_text":null},
    {"command_id":"selection.study","args":{},"label":"Study","key":"Cmd+Shift+S","icon":"study","active":false,"value":null,"disabled_text":null}
  ] },
  { "label": "Copy", "items": [
    {"command_id":"host.copy","args":{},"label":"Copy","key":"Cmd+C","icon":"copy","active":false,"value":null,"disabled_text":null},
    {"command_id":"copy.passage","args":{"text":"For God so loved the world","reference":"John.3.16-John.3.17","resource_id":"bsb"},"label":"Copy with reference","key":null,"icon":"copy-passage","active":false,"value":null,"disabled_text":null}
  ] }
] }
```

#### note_text
```json
{ "target_kind": "note_text", "title": "“grace”", "focus": null, "context": {"has_selection":true,"focus":"notes_dock"}, "sections": [
  { "label": "Edit", "items": [
    {"command_id":"host.cut","args":{},"label":"Cut","key":"Cmd+X","icon":"cut","active":false,"value":null,"disabled_text":null},
    {"command_id":"host.copy","args":{},"label":"Copy","key":"Cmd+C","icon":"copy","active":false,"value":null,"disabled_text":null},
    {"command_id":"host.paste","args":{},"label":"Paste","key":"Cmd+V","icon":"paste","active":false,"value":null,"disabled_text":null}
  ] },
  { "label": "Format", "items": [
    {"command_id":"note.bold","args":{},"label":"Bold","key":"Cmd+B","icon":"bold","active":false,"value":null,"disabled_text":null},
    {"command_id":"note.italic","args":{},"label":"Italic","key":"Cmd+I","icon":"italic","active":false,"value":null,"disabled_text":null},
    {"command_id":"note.link_passage","args":{},"label":"Link passage","key":null,"icon":"link-passage","active":false,"value":null,"disabled_text":null}
  ] },
  { "label": "Search", "items": [
    {"command_id":"search.text","args":{"text":"grace","scope":"all"},"label":"Search selection in all editions","key":null,"icon":"search-all","active":false,"value":null,"disabled_text":null}
  ] }
] }
```

#### note_link
```json
{ "target_kind": "note_link", "title": "Rom 8:28", "focus": null, "context": {"has_selection":false,"focus":"notes_dock"}, "sections": [
  { "label": "Open", "items": [
    {"command_id":"nav.navigate","args":{"column":1,"reference":"Rom.8.28"},"label":"Open Rom 8:28 in","key":null,"icon":"open","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"nav.navigate","args":{"column":1,"reference":"Rom.8.28"},"label":"Column 1 · BSB","key":null,"icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"nav.navigate","args":{"column":2,"reference":"Rom.8.28"},"label":"Column 2 · SBLGNT","key":null,"icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"nav.navigate","args":{"column":3,"reference":"Rom.8.28"},"label":"Column 3 · Lexicon","key":null,"icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"nav.navigate","args":{"new_column":true,"reference":"Rom.8.28"},"label":"New column","key":null,"icon":"new-column","active":false,"value":null,"disabled_text":null}
    ] },
    {"command_id":"tab.open_edition","args":{"reference":"Rom.8.28","resource_id":"bsb"},"label":"Compare in","key":null,"icon":"compare","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"tab.open_edition","args":{"reference":"Rom.8.28","resource_id":"bsb"},"label":"BSB","key":null,"icon":"compare","active":false,"value":null,"disabled_text":null},
      {"command_id":"tab.open_edition","args":{"reference":"Rom.8.28","resource_id":"sblgnt"},"label":"SBLGNT","key":null,"icon":"compare","active":false,"value":null,"disabled_text":null}
    ] }
  ] },
  { "label": "Link", "items": [
    {"command_id":"copy.reference","args":{"reference":"Rom.8.28"},"label":"Copy reference","key":null,"icon":"copy-reference","active":false,"value":null,"disabled_text":null},
    {"command_id":"note.remove_link","args":{"reference":"Rom.8.28"},"label":"Remove link","key":null,"icon":"remove-link","active":false,"value":null,"disabled_text":null}
  ] }
] }
```

#### tab
```json
{ "target_kind": "tab", "title": "BSB · Rom 8:28", "focus": {"command_id":"tab.focus","args":{"tab_id":"t2"}}, "context": {"has_selection":false,"focus":"tab"}, "sections": [
  { "label": "Tab", "items": [
    {"command_id":"tab.close","args":{},"label":"Close tab","key":"Alt+W","icon":"close","active":false,"value":null,"disabled_text":null},
    {"command_id":"tab.close_others","args":{},"label":"Close other tabs","key":null,"icon":"close-others","active":false,"value":null,"disabled_text":null}
  ] },
  { "label": "Move", "items": [
    {"command_id":"tab.duplicate_to_column","args":{"column":1},"label":"Duplicate to","key":null,"icon":"duplicate","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"tab.duplicate_to_column","args":{"column":1},"label":"Column 1 · BSB","key":null,"icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"tab.duplicate_to_column","args":{"column":2},"label":"Column 2 · SBLGNT","key":null,"icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"tab.duplicate_to_column","args":{"column":3},"label":"Column 3 · Lexicon","key":null,"icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"tab.duplicate_to_column","args":{"new_column":true},"label":"New column","key":null,"icon":"new-column","active":false,"value":null,"disabled_text":null}
    ] },
    {"command_id":"tab.move_to_column","args":{"column":2},"label":"Move to","key":null,"icon":"move","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"tab.move_to_column","args":{"column":1},"label":"Column 1 · BSB","key":"Alt+Shift+1","icon":"column","active":false,"value":null,"disabled_text":"The tab is already in column 1"},
      {"command_id":"tab.move_to_column","args":{"column":2},"label":"Column 2 · SBLGNT","key":"Alt+Shift+2","icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"tab.move_to_column","args":{"column":3},"label":"Column 3 · Lexicon","key":"Alt+Shift+3","icon":"column","active":false,"value":null,"disabled_text":null},
      {"command_id":"tab.move_to_column","args":{"new_column":true},"label":"New column","key":null,"icon":"new-column","active":false,"value":null,"disabled_text":null}
    ] }
  ] },
  { "label": "Link", "items": [
    {"command_id":"tab.pin","args":{},"label":"Pin","key":"Cmd+Shift+P","icon":"pin","active":true,"value":null,"disabled_text":"The tab is already pinned"},
    {"command_id":"tab.follow","args":{},"label":"Follow active passage","key":null,"icon":"follow","active":false,"value":null,"disabled_text":null},
    {"command_id":"tab.link","args":{"group":"1"},"label":"Link group","key":null,"icon":"link","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"tab.link","args":{"group":"1"},"label":"Group 1","key":"Cmd+Shift+L","icon":"link","active":false,"value":null,"disabled_text":null},
      {"command_id":"tab.link","args":{"group":"2"},"label":"Group 2","key":null,"icon":"link","active":false,"value":null,"disabled_text":null}
    ] }
  ] }
] }
```

#### column
```json
{ "target_kind": "column", "title": "Column 2", "focus": {"command_id":"column.focus","args":{"column":2}}, "context": {"has_selection":false,"focus":"tab"}, "sections": [
  { "label": "Column", "items": [
    {"command_id":"column.focus","args":{"column":2},"label":"Focus column 2","key":"Alt+2","icon":"focus","active":false,"value":null,"disabled_text":null},
    {"command_id":"tab.new","args":{},"label":"New tab","key":"Alt+T","icon":"new-tab","active":false,"value":null,"disabled_text":null}
  ] },
  { "label": "Move", "items": [
    {"command_id":"column.move_left","args":{},"label":"Move left","key":null,"icon":"move-left","active":false,"value":null,"disabled_text":null},
    {"command_id":"column.move_right","args":{},"label":"Move right","key":null,"icon":"move-right","active":false,"value":null,"disabled_text":null}
  ] },
  { "label": "Layout", "items": [
    {"command_id":"column.equalize","args":{},"label":"Equalize widths","key":null,"icon":"equalize","active":false,"value":null,"disabled_text":null},
    {"command_id":"column.close","args":{},"label":"Close column","key":null,"icon":"close-column","active":false,"value":null,"disabled_text":null}
  ] }
] }
```

#### background
```json
{ "target_kind": "background", "title": null, "focus": null, "context": {"has_selection":false,"focus":"tab"}, "sections": [
  { "label": "Tab", "items": [
    {"command_id":"tab.new","args":{},"label":"New tab","key":"Alt+T","icon":"new-tab","active":false,"value":null,"disabled_text":null},
    {"command_id":"nav.paste_passage","args":{},"label":"Paste passage","key":null,"icon":"paste-passage","active":false,"value":null,"disabled_text":null}
  ] },
  { "label": "View", "items": [
    {"command_id":"view.text_size_up","args":{},"label":"Text size","key":null,"icon":"text-size","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"view.text_size_up","args":{},"label":"Larger","key":null,"icon":"text-larger","active":false,"value":null,"disabled_text":null},
      {"command_id":"view.text_size_down","args":{},"label":"Smaller","key":null,"icon":"text-smaller","active":false,"value":null,"disabled_text":null},
      {"command_id":"view.text_size_reset","args":{},"label":"Actual size","key":null,"icon":"text-reset","active":false,"value":null,"disabled_text":null}
    ] },
    {"command_id":"view.theme","args":{"text":"system"},"label":"Theme","key":null,"icon":"theme","active":false,"value":null,"disabled_text":null, "submenu": [
      {"command_id":"view.theme","args":{"text":"system"},"label":"System","key":null,"icon":"theme","active":true,"value":null,"disabled_text":null},
      {"command_id":"view.theme","args":{"text":"light"},"label":"Light","key":null,"icon":"theme","active":false,"value":null,"disabled_text":null},
      {"command_id":"view.theme","args":{"text":"dark"},"label":"Dark","key":null,"icon":"theme","active":false,"value":null,"disabled_text":null},
      {"command_id":"view.theme","args":{"text":"sepia"},"label":"Sepia","key":null,"icon":"theme","active":false,"value":null,"disabled_text":null}
    ] }
  ] }
] }
```

## Conformance

`context-menu` (223 cases) runs through `run_all` on every host. 30 menu cases: words in a
translation (whole menu), without a host selection, in Greek in another column, in Hebrew in
the Genesis workspace, without a passage, with a custom and with the alternate swatch, in a
tab that is not open; selections in one verse, across verses, in Greek, without a passage;
note spot (whole menu), note selection in the dock and in a Notes tab; passage links in the
dock, an Old Testament link in a Notes tab, and one outside notes (Remove link refused); tabs
hidden and pinned, linked in another column, active, alone in their column, and one not open;
first, last, only and missing columns; background (whole menu), of the lexicon column, and at
the largest text size in dark. Each checks what applies of the header (title, focus change,
dispatch context), the outline (every section and row with disabled words, active marks and
submenus), rows in full (arguments, keys, icons, values) and the whole menu (3 cases); 20
choices run the focus change and the row and check the session after (compare from Greek
then undo; word study keeps focus; close other tabs then undo; move, duplicate and close
column; theme; host hand-offs; Remove link refused outside notes). Every menu also checks its
shape: registered commands, known icons, reasons in words, at most 12 top-level rows, no empty
section. Three scenarios cover the new commands: `tab.open_edition` (reuse, explicit column,
new column, the two refusals, three undos), `tab.close_others` (one-tab refusal, reopen,
undos) and every host hand-off with its refusal (`invalid_value`, `needs_argument`,
`unavailable`, `needs_selection`, `wrong_context`). `shell-commands`: registry ids extended,
and one palette expectation changed (below). Total conformance: 1594 cases (was 1371).

## Bench (`just bench-shell`, release, 100 rounds)

The menu is built after each column focus for all seven target kinds: a word and a
selection in the active tab, a note spot and a passage link in the dock, a hidden tab in the
next column (the menu works on a copy of the 120-tab session with that tab focused, the
slowest kind), column 2 and the background. Budget: shown on the next frame.

| interaction | specs/22 §7 budget | samples | p50 µs | p95 µs | max µs |
|---|---:|---:|---:|---:|---:|
| column focus (column.focus N) | 16 ms | 300 | 0.6 | 0.7 | 1.1 |
| tool well for the focused tab or dock (Shell::tool_well) | 16 ms | 600 | 8.2 | 8.7 | 15.2 |
| context menu for every target kind (Shell::context_menu) | 16 ms | 2100 | 11.4 | 83.9 | 107.5 |

Other rows of the same run: palette keystroke p95 290.4 µs, session view (120 tabs) p95
96.7 µs, workspace switch p95 0.6 µs, undo p95 116.5 µs.

## Lanes

| Command | Tree | Result |
|---|---|---|
| `nix develop .#core -c just ci` | `a784b37` | exit 0. `Summary [   6.409s] 312 tests run: 312 passed, 0 skipped`; clippy and the wasm check clean |
| `nix develop .#apple -c just test-swift` | `a784b37` | exit 0. `Executed 9 tests, with 0 failures (0 unexpected)`; `testContextMenuComesFromTheCore` passed; `conformance cases: 1594 passed, 0 failed` |
| `nix develop .#android -c just test-kotlin` (test-results cleared first) | `a784b37` | exit 1: `compileTestKotlin FAILED`, `Unresolved reference 'ShellContextTarget'` (missing import in the test) |
| same, after `ebd5258` | `ebd5258` | exit 0. `tests="9" skipped="0" failures="0"`; `contextMenuComesFromTheCore() PASSED`; `conformance cases: 1594 passed, 0 failed`; `BUILD SUCCESSFUL` |
| `nix develop .#web -c just test-web` | `d1b72e3` | exit 0. `tests 9, pass 9, fail 0`; `conformance cases: 1594 passed, 0 failed (api 0.1.2)` |
| `nix develop .#web -c just test-wasm` | `d1b72e3` | exit 0. `running 8 tests`, `shell_handle_returns_the_context_menu ... ok`, `conformance_suite_passes ... ok`, `test result: ok. 8 passed; 0 failed` |
| `nix develop .#core -c just bench-shell` | `d1b72e3` | exit 0; table above |
| `nix develop .#core -c python3 tools/validate.py` | `d1b72e3` | PASS: 33 schema/example validations; 77 acyclic tasks; 20 conformance fixtures; 125 negative tests |

`ebd5258` changes only the Kotlin test and `d1b72e3` only documentation, the backlog and
README counts, so the `ci` and Swift results on `a784b37` stand for the Rust and Swift code.

## Decisions to review

1. **Focus first, not per-row tab ids.** Tab, column and note commands act on the active tab,
   so rather than giving every command a `tab_id`, the menu carries the `tab.focus` /
   `column.focus` a click on the target would run, and its rows are computed on a copy of the
   session with that focus. Hosts apply it before the chosen row (not on open), so dismissing
   the menu changes nothing.
2. **The host selects a right-clicked word**, as macOS does, so the word menu's Highlight,
   Collect and Note on it are the selection commands; the menu's context is
   `has_selection: true` for words and selections whatever the host passed.
3. **Word study on a translation is shown disabled** ("Word study works on Greek and Hebrew
   text") by the menu, though `tab.open_lexicon` itself runs anywhere; alignment-based study
   from a translation is not in scope.
4. **Compare is `tab.open_edition`.** Editions are the resources with a registered language
   (hosts already register the installed editions' languages for the well). An original-language
   edition that cannot hold the passage is left out (Greek outside the NT, Hebrew/Aramaic outside
   the OT; an LXX would be wrongly left out of OT passages). One other edition gives a plain
   "Compare in SBLGNT" row, more give "Compare in ▸". The new tab takes the source tab's link
   state (so a compare tab of a group-1 tab is in group 1 and keeps pace); a reused tab of that
   edition in the target column is moved alone, its group not dragged.
5. **Submenu parents carry their first enabled child's command**, so every row is a registered
   command and a web host may run the parent on click; Mac submenus just open.
6. **Copy is a submenu on words** (Copy word, Copy reference, Copy verse with reference) to
   keep the word menu at 12 rows; on selections Copy (`host.copy`) and Copy with reference are
   plain rows. `copy.passage` covers both "verse with reference" (no text: the host reads the
   verse from `resource_id`) and "selection with reference" (`text` given).
7. **Clipboard items are registered (`host.cut/copy/paste`) without registry keys**, so no
   host binds ⌘C globally from the registry; the menu shows `Cmd+X/C/V` for them.
8. **Highlight ▸ offers the four swatches of design/tokens/shell.json** (a custom current
   swatch first as "Current swatch") through a new `selection.highlight_in` with the swatch,
   rather than changing `selection.highlight`'s argument shape.
9. **Escape hatch: ⇧-right-click on web, ⌥-right-click on Mac**; keyboard: ⇧F10 / context-menu
   key on web, ⌃-click, VO-⇧-M and a PC keyboard's menu key on Mac. Opening a menu is not a
   registered command (it is not an action), like Esc closing the palette.
10. **A missing target is an error**; over UniFFI it is `InvalidDocument` rather than a new
    error case, because the Mac app switches over `ShellCommandError` exhaustively.
11. **Open in ▸ leaves out the clicked text's own column and columns showing a note**; Move to ▸
    lists every column with the tab's own disabled ("The tab is already in column 1"), as the
    rule is to show disabled rows with words.
12. **One existing palette expectation changed**: `>highlight` now ranks "Highlight in colour"
    third, above "Highlighter pen tool" (a whole-word title prefix outranks a partial one).
    The case's point, the alias row first, holds.
13. **New commands are appended** after `tab.open_lexicon`; `search.lemma` and
    `nav.cross_references` are registered but always disabled, so the palette lists them
    with the data they need.
14. Section labels (`Search`, `Open`, `Mark`, `Copy`, `Edit`, `Format`, `Link`, `Tab`, `Move`,
    `Column`, `Layout`, `View`) are in the menu for hosts that want headings; separators alone
    are fine.

## For the host parts

- Replace the platform menu in reading text, notes, tab strips, identity rows and column
  backgrounds; let ⇧-right-click (web) / ⌥-right-click (Mac) through to the platform menu.
- Build the target from the hit: for a word pass `text`, `reference`, `segment_id`, the
  scalar range, `resource_id` and the tab's `tab_id`, and select the word as the menu opens;
  for a selection the start and end references; in notes `tab_id` only for a Notes tab.
- Call `contextMenu` / `context_menu` with `has_selection`; draw sections with separators,
  submenus one level, `active` as a check, `value` as a swatch, disabled rows with their words.
- On choose: apply `focus` if set, then the row under the menu's `context`.
- Perform the host commands: `search.text` (palette with the query and scope: `edition` with
  `resource_id`, `chapter` with `reference`, `all`, `workspace`), `copy.text`,
  `copy.reference` (`formatHuman`), `copy.passage` (text or the verse from `resource_id`, then
  the reference), `host.cut/copy/paste` (the platform action), `nav.paste_passage` (parse the
  clipboard, navigate), `selection.highlight_in` (highlight the selection in the swatch),
  `note.remove_link`, and the existing `selection.*` / `note.*`.
- Open the same menu from the keyboard (⇧F10 / context-menu key on web; ⌃-click, VO-⇧-M on
  Mac) at the selection, the caret's word, the focused tab or the column background.

## Skipped

No host UI (the host agents' part). No screenshots: nothing renders in the core part. No
palette expansion of `tab.open_edition` per edition (typing "open in edition" without an
edition shows it disabled with "Choose an edition").
