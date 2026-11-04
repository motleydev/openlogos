# B074 evidence (core part): workspaces, the user store and session restore

Recorded 4 November 2026 on the Mac mini (`office-hub`, Apple M1). A Claude Code subagent did
the work overnight while the owner slept, in the git worktree `openlogos-b074` (branch
`b074-core-workspaces`, from `b078-core-reading` 2e53c87; B078 is PR #32, not merged yet).
Nothing was pushed or merged. No simulator, device, keychain or macOS user was touched. No
Bible text was read, fetched or committed beyond the conformance fixtures already in the
repository. This is the core part; the web and Mac hosts build on this branch in their own
B074 parts (see "For the host parts" below). Spec: specs/24-persistence.md.

## Why

Before this work nothing survived a quit. The session (workspaces, columns, tabs, scroll
positions), the notes (B076: HTML on the web, attributed strings on the Mac), the highlights
(B072/B078 segment spans), the ink (B078: pinned to verses by a host-only anchor) and the
lexicon language all lived in host memory. B074 asks for workspaces at size, cross-workspace
palette rows, a full session restore, and undo for a tab drag and a column resize. The
B072/B076 evidence also promised that B074 would persist notes, highlights and ink.

## What was built

| Item | Where |
|---|---|
| Audit of the workspace commands against the acceptance | §Audit below |
| `tab.move`: undoable drag of any tab to a column and slot, or into a new column | `crates/workspaces/src/commands.rs` (`tab_move_check`, `TabMove`, argument shape `tab_move`, `CommandArgs.index`) |
| The lexicon tab's language | `Tab.lexicon_language`, set by `tab.open_lexicon` from the clicked edition's registered language; `TabView`/`ShellTab.lexicon_language`; workspace 2.0 schema (optional field) |
| The user store | `crates/workspaces/src/store.rs`: `UserStore`, `Annotations` (`Note`/`NoteBlock`/`NoteSpan`, `Highlight`/`SegmentSpan`, `InkMark`), `StoreError`, `RestoreReport`/`StoreIssue`, `migrate_store` (forward-version hook), `PlaceholderCheck`; on `Shell`: `export_store(_json)`, `import_store_json`, `put_note`/`delete_note`/`note`, `add_highlight`/`remove_highlight`/`highlights_in`, `add_ink`/`remove_ink`/`ink_in`, and `Shell.changes` (the save counter) |
| Placeholder check | `ReadingLibrary::segment_is_placeholder` (crates/text) |
| Ink stroke check | `openlogos_ink::Stroke::validate` is public |
| Bindings | UniFFI `crates/ffi/src/native/store.rs` (methods on `ShellHandle`, `Store*` records, `StoreCallError`, `storeConstants`); `InkStroke` ⇄ core stroke conversions; wasm methods on `ShellHandle`, `storeConstants`, TypeScript types |
| Schema | `schemas/user-store.schema.json` (session and 2.0 workspaces by `$ref`, notes, highlights, ink with the ink-document stroke); `examples/user-store.json`; `tools/validate.py` `check_user_store`, kind `user_store`, eleven negative cases |
| Conformance | `crates/conformance/fixtures/user-store.json` (kind `user_store`) with `crates/conformance/src/store.rs`; `shell-commands` gains a drag-and-resize undo scenario and a palette row for another workspace's tab |
| Host tests | Swift `testUserStoreSavesAndRestoresTheSessionAndAnnotations`, Kotlin `userStoreSavesAndRestoresTheSessionAndAnnotations`, Node `B074: the user store …`, wasm worker `shell_handle_saves_and_restores_the_user_store` |
| Speed | `openlogos-shell-bench` B074 rows (save, restore, workspace new/rename/duplicate/close/switch, tab drag, chapter annotations) |
| Spec, backlog | specs/24-persistence.md (new); specs/22 §2 and §7 point to it; B074 acceptance gains the store wording, `depends_on` gains B078 |
| API | `API_VERSION` 0.1.3 → 0.1.4 (additive) |

### Commits

| Commit | Subject |
|---|---|
| `1e31768` | B074: tab.move (undoable drag with a slot), lexicon_language, the user store in core |
| `698feb4` | B074: user-store conformance fixture (round trip, restores, damage, placeholders); API 0.1.4 |
| `1440364` | B074: user-store schema, lexicon_language in workspace 2.0, validate.py checks the store |
| `de9b991` | B074: UniFFI surface for the user store, notes, highlights and ink |
| `0510c0f` | B074: wasm surface for the user store; host tests in Swift, Kotlin, Node and the worker |
| `bb0c008` | B074: the menu test allows B074's tab.move after B077's commands |
| `6859a05` | B074: bench rows for save, restore, the workspace commands, a tab drag and chapter annotations at the acceptance size |
| `826dd2b` | B074: specs/24 persistence (stored forms, save policy, restore order, failures, host storage); specs/22 and backlog B074 point to it |
| `1810d22` | B074: a restored stroke carries its mark's id |
| `206fc18` | B074: rustfmt |
| `54641c6` | B074: evidence (core part); specs/24 states the save budget, not an unmeasured number |
| `7fd5f6f` | B074: compact ink in the store, cached part JSON; saves under a millisecond |
| `e6e0cf5` | B074: specs/24 describes the compact ink form, the cached save and exportStore on the main thread |

plus the evidence commit.

## Lanes

Disk was the limit tonight. All lanes ran with `CARGO_INCREMENTAL=0`. With the lead's
approval I deleted only this worktree's `target/debug/incremental`, and the lead freed more of
my `target` later.

**First run, at `206fc18`, before the size and speed rework.** The lead ran test-kotlin and
bench-shell after I stopped at the disk line.

| Lane | Exit | Summary lines |
|---|---|---|
| `nix develop .#core -c just ci` | 0 | `Summary [   6.747s] 336 tests run: 336 passed, 0 skipped`; `PASS: 37 schema/example validations; … 22 conformance fixtures; … 138 negative tests …` |
| `nix develop .#web -c just test-web` | 0 | `openlogos_ffi_bg.wasm: 3487840 bytes before wasm-opt -Oz (758117 gzipped), 3228376 after (852909 gzipped)`; `tests 11`, `pass 11`, `fail 0`; `conformance cases: 1844 passed, 0 failed (api 0.1.4)` |
| `nix develop .#web -c just test-wasm` | 0 | `test result: ok. 10 passed; 0 failed` |
| `nix develop .#apple -c just test-swift` | 0 | `Executed 11 tests, with 0 failures (0 unexpected)`; `conformance cases: 1844 passed, 0 failed` |
| `nix develop .#android -c just test-kotlin` (lead, at `54641c6`) | 0 | `conformance cases: 1844 passed, 0 failed`; `BUILD SUCCESSFUL` |
| `just bench-shell` (lead, at `54641c6`) | 0 | store 3,019,800 bytes; save p95 13.06 ms, restore p95 31.36 ms (see Bench) |

**Second run, after the rework (compact ink, cached part JSON), at `7fd5f6f`/`e6e0cf5`.** Code
is unchanged between those two commits; only specs/24 changed.

| Lane | Exit | Summary lines |
|---|---|---|
| `nix develop .#core -c just ci` | 0 | `Summary [   6.647s] 337 tests run: 337 passed, 0 skipped`; `PASS: 37 schema/example validations; … 22 conformance fixtures; … 139 negative tests …`; clippy, rustfmt and check-wasm clean |
| `nix develop .#web -c just test-web` | 0 | `openlogos_ffi_bg.wasm: 3500949 bytes before wasm-opt -Oz (763435 gzipped), 3239689 after (859313 gzipped)`; `tests 11`, `pass 11`, `fail 0`; `conformance cases: 1844 passed, 0 failed (api 0.1.4)` |
| `nix develop .#web -c just test-wasm` | 0 | headless Chrome, dedicated worker: `test result: ok. 10 passed; 0 failed` |
| `nix develop .#apple -c just test-swift` | 0 | `Executed 11 tests, with 0 failures (0 unexpected)`; `conformance cases: 1844 passed, 0 failed` |
| `just bench-shell` (at `7fd5f6f`) | 0 | the table under Bench |
| `nix develop .#android -c just test-kotlin` | **not run** | after test-swift, free space on `/` was 4,163,796 KiB (3.97 GiB), below the 4 GiB stop line, and both `incremental` directories were already empty, so by the rule I stopped. The Kotlin test passed at `54641c6`. It exercises only the API, which did not change (`exportStore` returns the same string type; the stroke records are unchanged) |

## Audit of the workspace commands

| Acceptance | Before | Now |
|---|---|---|
| Create, rename, duplicate, close, switch workspaces | `workspace.new`, `.rename`, `.duplicate`, `.close`, `.switch`, `.next`, `.previous`: all present and undoable where they change the session (Snapshot undo) | Unchanged; each measured at the acceptance size (Bench) |
| Palette finds tabs and workspaces in every workspace and switches to them | Present: the Tabs section indexes every workspace (`PaletteIndex`). A tab row (`BSB · Jn 3:16`, subtitle `Genesis · column 1`) dispatches `tab.focus {workspace_id, tab_id}`, which switches the workspace and focuses the tab. A workspace row (`Genesis`, subtitle `Workspace · 1 column, 1 tab`) dispatches `workspace.switch`. `>go to tab` lists every other tab | Unchanged. The wording stays the B070/B072 wording that both hosts already render (decision 8). One more fixture case: `@wlc` from workspace 1 finds workspace 2's tab |
| Tab drag between columns is undoable | Only `tab.move_to_column`: the active tab, appended to the end, no slot. A drag could not drop a tab that is not active, or drop between two tabs | `tab.move` (new): any tab (`tab_id`) to `column` at slot `index`, within its own column too (a reorder), or `new_column` at a given number; undoable; a drop that changes nothing is refused (`already_in_column`) |
| Column resize is undoable | `column.resize` is undoable (B072) | Unchanged; hosts send it once, when the rule drag ends |
| Restore reproduces every workspace, column, tab, scroll anchor and tool | The records existed (`records()`, `fromRecords`), but no host saved them, and quiet commands (`tab.record_scroll`) did not bump anything a host could save on | The user store, with `Shell.changes` counting quiet commands too |

## API

Rust (`openlogos_workspaces::store`, methods on `commands::Shell`), Swift/Kotlin (UniFFI
camelCases them), wasm. Errors: Rust `StoreError` (serde tag `kind`); UniFFI
`StoreCallError.Refused(kind, reason)` (Kotlin `StoreCallException.Refused`; no
`message`/`cause` fields); wasm throws an `Error` with `kind`, `message` and `details`. Kinds:
`empty`, `not_json`, `not_a_store`, `unsupported_version`, `invalid_note`,
`invalid_highlight`, `placeholder_text`, `invalid_ink`, `not_found`.

| Rust | UniFFI (Swift / Kotlin) | wasm |
|---|---|---|
| `Shell::export_store_json() -> String` | `exportStore() -> String` | `exportStore(): string` |
| `Shell::import_store_json(&str) -> Result<RestoreReport, StoreError>` | `importStore(json:) throws -> StoreRestoreReport` | `importStore(json): StoreRestoreReport` |
| `Shell.changes: u64` | `changeCount() -> UInt64` | `changeCount(): number` |
| `Shell::note(id)`, `annotations.notes` | `note(id:) -> StoreNote?`, `notes() -> [StoreNote]` | `note(id)`, `notes()` |
| `Shell::put_note(Note)`, `delete_note(id)` | `putNote(note:)`, `deleteNote(id:)` | `putNote(note)`, `deleteNote(id)` |
| `Shell::add_highlight(Highlight, Option<&dyn PlaceholderCheck>) -> id` | `addHighlight(highlight:reading:) -> String` (`reading: ReadingHandle?`) | `addHighlight(highlight, reading)` (a `ReadingHandle`, required) |
| `Shell::remove_highlight(id)`, `highlights_in(resource, chapter_key)` | `removeHighlight(id:)`, `highlightsIn(resourceId:chapterKey:)` | `removeHighlight(id)`, `highlightsIn(resource, key)` |
| `Shell::add_ink(InkMark) -> id`, `remove_ink(id)`, `ink_in(resource, chapter_key)` | `addInk(mark:)`, `removeInk(id:)`, `inkIn(resourceId:chapterKey:)` | `addInk(mark)`, `removeInk(id)`, `inkIn(resource, key)` |
| `SAVE_DEBOUNCE_MS`, `STORE_FORMAT`, `STORE_SCHEMA_VERSION` | `storeConstants() -> StoreConstants` | `storeConstants()` |
| `CommandArgs.index`, command `tab.move` | `ShellCommandArgs.index`, `apply("tab.move", …)` | `args.index`, `apply({command_id: "tab.move", …})` |
| `Tab.lexicon_language`, `TabView.lexicon_language` | `ShellTab.lexiconLanguage` | `view() …tabs[i].lexicon_language` |

Records: `StoreNote {id, blocks: [StoreNoteBlock {kind: .paragraph | .heading | .listItem |
.quote, spans: [StoreNoteSpan {text, bold = false, italic = false, link = nil}]}]}`,
`StoreHighlight {id = "", resourceId, contentRevision, spans: [StoreSegmentSpan {segmentId,
start, end}], color}`, `StoreInkMark {id = "", resourceId, segmentId, stroke: InkStroke}`,
`StoreRestoreReport {schemaVersion, sessionRestored, workspaces, tabs, notes, highlights, ink,
issues: [StoreIssue {part, id?, detail}]}`, `StoreConstants {format, schemaVersion,
saveDebounceMs}`. wasm takes and returns the same objects in the core's snake_case JSON.

## JSON examples

From the `user-store` fixture's round trip (whole document: `examples/user-store.json`).

### shell.apply({command_id: "tab.move", args: {tab_id: "t2", column: 2, index: 1}})

On the `shell-commands` session (`[[*t1, t2], [*t3], [*t4]]`) it gives
`[[*t1], [t3, *t2], [*t4]]`, focus `{w1, column 2, t2}`, `recorded: true`. `edit.undo` puts
`t2` back at column 1, position 2. Dropping the only tab of column 3 into column 1 removes
column 3 and gives column 1 and 2 widths of 0.5 each (`[400, 400, 200]` → `[500, 500]`).

### exportStore() (excerpt, laid out for reading; the file is compact JSON)

```json
{
  "format": "openlogos-user-store",
  "schema_version": "1.0",
  "fixture_only": true,
  "core_api_version": "0.1.4",
  "session": {
    "schema_version": "1.0", "fixture_only": true,
    "workspace_ids": ["w1", "w3", "w2"], "active_workspace_id": "w1",
    "tool": "pen", "next_serial": 4,
    "reading": {"text_size_step": 1, "line_spacing": 1.5, "theme": "system"},
    "ink": {"width": 4.0, "swatch": "#F5D90A", "alternate_swatch": "#4CC38A"}
  },
  "workspaces": [
    {"schema_version": "2.0", "id": "w1", "title": "John study",
     "columns": [
       {"id": "c1", "shown_tab_id": "t1", "width_fraction": 0.3, "tabs": [
         {"id": "t1", "kind": "reading", "content_id": "bsb", "content_revision": "0",
          "reference": "John.3.17", "scroll_anchor": {"segment_id": "John.3.17", "scalar": 9},
          "link_group": "1", "pinned": false}]},
       {"id": "c2", "…": "t3 (SBLGNT, followed to John.3.17@0), then the dragged t2"},
       {"id": "c3", "tabs": [
         {"id": "t4", "kind": "lexicon", "content_id": "fixture:lexicon-1", "reference": "John.3.17",
          "pinned": true, "lexicon_word": "ἀγάπη", "lexicon_language": "grc", "…": "…"}]}],
     "active_column_id": "c2", "…": "…"},
    "… w3 Greek, w2 Genesis"
  ],
  "notes": [
    {"id": "fixture:note:1", "blocks": [
      {"kind": "heading", "spans": [{"text": "New birth "}, {"text": "Jn 3:3", "link": "John.3.3"}]},
      {"kind": "paragraph", "spans": [{"text": "Not by "}, {"text": "works", "italic": true}, {"text": " but "}, {"text": "from above", "bold": true}, {"text": "."}]},
      {"kind": "list_item", "spans": [{"text": "Compare "}, {"text": "Jn 1:12–13", "link": "John.1.12-John.1.13"}]},
      {"kind": "quote", "spans": [{"text": "A fixture quote, not Scripture."}]},
      {"kind": "paragraph", "spans": []}]},
    {"id": "n2", "blocks": [{"kind": "paragraph", "spans": [{"text": "Second note"}]}]}
  ],
  "highlights": [
    {"id": "h1", "resource_id": "bsb", "content_revision": "bsb-2024",
     "spans": [{"segment_id": "John.3.16", "start": 20, "end": 44}], "color": "#F5D90A"},
    {"id": "h2", "resource_id": "bsb", "content_revision": "bsb-2024",
     "spans": [{"segment_id": "John.3.16", "start": 100, "end": 137}, {"segment_id": "John.3.17", "start": 0, "end": 12}],
     "color": "#4CC38A"},
    "… h3 (SBLGNT John 3:16), h4 (WLC Genesis 1:1)"
  ],
  "ink": [
    {"id": "k5", "resource_id": "bsb", "segment_id": "John.3.17",
     "tool": "pen", "brush_version": "pf-1.2.3/pen-1", "width": 4.0, "color": "#1F6FEB", "opacity": 1.0,
     "xy": [120, -35, 285, 55, 475, 43], "t": [0, 16, 17], "pressure": [null, 50, 12]},
    "… k6 (WLC Genesis 1:2, highlighter)"
  ],
  "next_serial": 6
}
```

`k5` is the compact stroke form (specs/24 §2.3). The columns are delta-encoded fixed point:
tenths of a point, milliseconds and hundredths. It decodes to (12.0, −3.5, no pressure, 0 ms),
(40.5, 2.0, 0.50, 16 ms) and (88.0, 6.3, 0.62, 33 ms). The fixture drew it at (12.04, −3.46),
16.4 ms and pressure 0.623; `addInk` rounded it on the way in, and `inkIn` returns the point
objects.

The fifth highlight in the fixture, on BSB John 3:1 (placeholder text), was refused with
`placeholder_text` and minted no id. The serial still moved past 5, because the ink got `k5`.

### importStore(json) on a fresh fixture shell

```json
{"schema_version": "1.0", "session_restored": true, "workspaces": 3, "tabs": 6,
 "notes": 2, "highlights": 4, "ink": 2, "issues": []}
```

### importStore of a store with a dangling active workspace and bad items

```json
{"schema_version": "1.0", "session_restored": false, "workspaces": 2, "tabs": 5,
 "notes": 1, "highlights": 1, "ink": 0,
 "issues": [
  {"part": "session", "id": null, "detail": "active workspace nowhere does not exist"},
  {"part": "note", "id": "n2", "detail": "note n2: the link `not a passage` is not a passage"},
  {"part": "highlight", "id": "h1", "detail": "highlight h1: a highlight has at least one span"},
  {"part": "highlight", "id": "h2", "detail": "highlight h2: `yellow` is not a #RRGGBB colour"},
  {"part": "ink", "id": "k4", "detail": "ink k4: stroke `k4` has 1 points; at least 2 are required"},
  {"part": "ink", "id": "k5", "detail": "4 coordinates for 1 points"}]}
```

### importStore of a truncated file throws (the fixture's `a truncated file`)

```json
{"kind": "not_json", "message": "the store is not JSON: EOF while parsing an object at line 1 column 72",
 "details": {"kind": "not_json", "detail": "EOF while parsing an object at line 1 column 72"}}
```

## Conformance

`crates/conformance/fixtures/user-store.json` (kind `user_store`) runs through
`crates/conformance/src/store.rs`, so every host runs it through `run_conformance`. It has
**77 cases**, all passing. `shell-commands` grew from 295 to 345 checks: the new
"tab drag and column resize (B074)" scenario has 14 steps and there is one new palette case.
The whole suite is **1,844 cases, 0 failed (api 0.1.4)**; it was 1,733 at B078's last commit.

| Group | Cases | What |
|---|---:|---|
| Round trip | 27 | Nine commands run on the `shell-commands` session with the fixture languages registered: a scroll into John 3:17 that carries the linked SBLGNT tab, focus column 2, look up `ἀγάπη` (the lexicon tab records `grc`), the pen, drag `t2` into column 2 at slot 1, column 1 resized to 0.3, text size up, a new workspace "Greek", switch back. Then two notes in the block form, five highlights (four stored; one refused on placeholder text) and two strokes. Checks: the minted ids; the exact document written; `export_store` equals `export_store_json`; restored into a fresh shell, nothing is set aside, the session and annotations are equal, saving again writes the same bytes, nothing is left to undo; focus, workspace order, tool, reading settings, widths `[300, 467, 233]`, lexicon languages, anchors and references of the restored session; highlight and ink queries for five chapters |
| Restores | 42 | A store from before B074's fields: no `tool`/`reading`/`ink` in the session, no annotation arrays, so the defaults apply and nothing is due to save. A 1.0 workspace inside a store is migrated: three columns at 333 each, tool `eraser`. A newer minor `1.7` with unknown fields at the top and on a note is read. A bad session beside good and bad items: the session is set aside and the shell keeps its own, the good note and highlight are kept, five items are set aside (one of them a stroke whose columns disagree) with their ids, and a save is due. A repeated note id. Six documents are refused whole and leave the shell exactly as it was: empty, truncated, an array, another format, no version, a newer major |
| Placeholders | 10 | Over the placeholder library: BSB 3:1, SBLGNT 3:15 and WLC Gen 1:3 are refused. The fixture verses BSB 3:16, SBLGNT 3:16 and WLC Gen 1:2 are stored. A selection from a fixture verse into a placeholder is refused. An edition the library does not have is stored. A bad colour and an empty span are `invalid_highlight` |
| shell-commands: drag and resize | 14 steps | Drag t2 into column 2 at slot 0; reorder t3 to slot 0 of its column; drag the only tab of column 3 into column 1 (column 3 closes, widths renormalise); undo; drag t2 into a new column; resize column 1; undo twice; no-op drop refused; unknown tab refused; new column from a one-tab column refused; drag to the end of column 3 (column 1 closes); undo; redo |
| shell-commands: palette | 1 | `@wlc` in workspace 1 finds workspace 2's WLC tab, which switches workspace and focuses it |

## Bench

`just bench-shell` (release, 100 rounds) on the Mac mini. The B074 rows use ten workspaces ×
twelve tabs (every tab scrolled, with history), 200 notes of five blocks, 1,000 highlights
and 200 strokes of 120 points with pressure, altitude and azimuth on every point.

**Before the rework** (the lead's run at `54641c6`): the store was 3,019,800 bytes. Save
(`export_store`) p50/p95/max was 12,798.8 / 13,064.2 / 13,267.6 µs and restore was 31,085.2 /
31,363.4 / 33,745.5 µs. Workspace new 22.9 / 27 / 28.5, rename 3.7 / 6.5 / 13.8, duplicate
16 / 19.5 / 21.5, close 7.4 / 13.4 / 18.4, switch 0.5 / 1.4 / 4.5; `tab.move` 9.8 / 13.2 /
16.9; `highlights_in` + `ink_in` 10.6 / 18.7 / 28.7. Point objects written at full float
precision made up most of the store: about 2.4 MB of the 3 MB.

**After the rework** (at `7fd5f6f`). The lead's targets were export p95 ≤ 4 ms and a store
≤ 1 MB; both are met.

User store part session and workspaces: 91892 bytes
User store part notes: 159591 bytes
User store part highlights: 143053 bytes
User store part ink: 370830 bytes
User store at the B074 size: 765540 bytes.

| interaction | specs/22 §7 budget | samples | p50 µs | p95 µs | max µs |
|---|---:|---:|---:|---:|---:|
| palette keystroke: passage, commands, tabs (index kept) | 16 ms | 11800 | 141.9 | 403.5 | 1119.6 |
| palette keystroke with the text section (fixture provider) | 50 ms | 3200 | 43.6 | 405.5 | 728.6 |
| palette index build after a command (120 tabs) | 16 ms | 200 | 249.6 | 260.8 | 905.4 |
| tab switch within a column (tab.next) | 16 ms | 1200 | 0.6 | 0.9 | 18.1 |
| column focus (column.focus N) | 16 ms | 300 | 0.6 | 0.7 | 1.3 |
| tool well for the focused tab or dock (Shell::tool_well) | 16 ms | 600 | 8.2 | 8.7 | 12.6 |
| context menu for every target kind (Shell::context_menu) | 16 ms | 2100 | 11.9 | 84.4 | 111.6 |
| workspace switch (workspace.next, 10 × 3 × 4) | 100 ms | 1000 | 0.5 | 0.6 | 1 |
| tool change (tool.*) | 16 ms | 300 | 0.9 | 1.3 | 1.7 |
| go to passage, group and follow tabs move (nav.navigate) | 100 ms | 200 | 71.5 | 81 | 112.5 |
| move tab to column 2, with undo record | 16 ms | 200 | 36.1 | 54.8 | 64.9 |
| undo | 16 ms | 200 | 105.4 | 121.2 | 159 |
| session view with tab labels (Shell::view, 120 tabs) | 1 ms | 200 | 99.6 | 105 | 358.8 |
| locate a typed passage (ranges, chapters, books) | 5 ms | 800 | 41.3 | 81 | 96.2 |
| chapter content, placeholder pack (Ps 119 among them) | 5 ms | 700 | 10.7 | 63 | 65.8 |
| chapter content, file pack on disk (read, SHA-256, parse) | 5 ms | 220 | 392.3 | 656.2 | 1647.3 |
| chapter neighbours | 5 ms | 700 | 0.4 | 0.5 | 0.5 |
| scroll into the next verse: tab, group and follower move (tab.record_scroll) | 16 ms | 2000 | 8.3 | 8.5 | 18.6 |
| save after a scroll: export the user store (10 × 12 tabs, 200 notes, 1,000 highlights, 200 strokes; annotation parts cached) | 4 ms | 100 | 373.8 | 409 | 966.5 |
| save after a stroke is drawn (the ink part rewritten) | 4 ms | 100 | 500 | 550 | 1262.5 |
| save after a note edit (the notes part rewritten) | 4 ms | 100 | 877.8 | 983.1 | 1485.8 |
| save with nothing cached (every part written; a restore does this once) | 16 ms | 100 | 4359.4 | 5040 | 7833.3 |
| restore: import that store into a fresh shell (core share) | 100 ms | 100 | 16524.8 | 19706.7 | 29146.2 |
| workspace new (workspace.new, at that size) | 100 ms | 100 | 18.4 | 29 | 69.8 |
| workspace rename (workspace.rename) | 100 ms | 100 | 3 | 6.4 | 9.5 |
| workspace duplicate (workspace.duplicate, 12 tabs with history) | 100 ms | 100 | 15.8 | 21.3 | 56.2 |
| workspace close (workspace.close) | 100 ms | 200 | 8 | 13.9 | 37.5 |
| workspace switch (workspace.switch, 10 × 12 with annotations) | 100 ms | 1000 | 0.5 | 1.3 | 5.4 |
| tab drag into another column (tab.move, with undo record) | 16 ms | 200 | 9.8 | 14.7 | 35.8 |
| highlights and ink of one chapter (highlights_in + ink_in) | 1 ms | 700 | 10.7 | 16.8 | 42 |

How the save got cheap:

1. **Compact ink.** The stroke columns are delta-encoded fixed point, at 0.1 pt, 1 ms and
   0.01; `addInk` rounds on the way in, so a restore is exact. The ink part went from about
   2.4 MB to 371 KB. That alone took a full save from 13 ms to about 4.5 ms.
2. **Cached part JSON.** The notes, the highlights and each stroke keep their written JSON
   until they change, and a restore writes them once. A save then serializes the session
   (92 KB, about 0.35 ms) and the parts that changed. A new stroke serializes only that
   stroke; a note edit rewrites the notes part.
3. Compact JSON throughout (it already was), and the document is assembled by
   concatenation.

The restore went from 31.4 ms to 19.7 ms p95. That includes writing the part caches once.
The older rows are at their B078 levels.

## Decisions for the owner

1. **One document, not per-part files.** The store holds the session, the workspaces, the
   notes, the highlights and the ink in one JSON document, written whole. At the acceptance
   size it is 766 KB. A save after a scroll costs 0.41 ms p95, because the unchanged parts'
   JSON is cached (Bench). Per-part files would add partial writes and the need to keep the
   parts consistent, for no measurable gain. The
   workspace records keep their own schema (ADR 0003), so a workspace can still be exported
   alone.
2. **Notes are stored as blocks and spans, not as host markup.** The block kinds are
   paragraph, heading, list item and quote; the span marks are bold, italic and a passage
   link in OSIS. Both hosts convert at the edge (specs/24 §2.1). HTML would make the web the
   reference implementation, and attributed strings would do the same for the Mac. This form
   keeps markup out of the file and gives the core the links. Anything an editor holds
   beyond these formats is not stored, such as pasted colours or fonts.
3. **Highlights store offsets only, no quoted text.** A user's file then never carries Bible
   text, which matters once licensed editions arrive (B071). Reanchoring across a revision
   uses lineage (specs/03) instead of a TextQuoteSelector. If you would rather have the quote
   kept for robustness, that is a minor version with an optional field.
4. **Ink is stored compactly and rounded** (lead request after the first bench). Points are
   kept to a tenth of a point, a millisecond and a hundredth of pressure or angle, which is
   below what a stylus resolves on screen, and written as delta-encoded integer columns. The
   API still speaks point objects; only the file differs. The schema stays 1.0, since
   nothing has shipped.
5. **Ink belongs to the edition and the verse, not to a tab.** A stroke drawn on BSB John
   3:17 shows in every BSB tab at that verse, and duplicating or closing a tab keeps it. Its
   points are offsets from the verse's top, which is B078's host anchor made durable. Ink
   does not re-flow when the text size or column width changes.
6. **Placeholder rule enforced when a highlight is added, not on restore.** A highlight stored
   on a real edition is kept even if that edition is later missing (it simply doesn't paint).
   Ink may sit on placeholder text (it quotes nothing).
7. **Damage handling: refuse the whole file, or set parts aside.** A file that is not a 1.x
   store is refused whole: the shell is untouched and the host backs the file up and starts
   from its seed. In a readable store, a bad session falls back to the host's session but
   keeps the notes, highlights and ink, and bad items are skipped and named. Nothing is dropped
   silently, and the host keeps a backup whenever anything was set aside.
8. **Palette wording unchanged.** The brief sketched rows such as "BSB Jn 3 — in Study" and
   "Workspace: Greek". The core already gives tab rows the resource and passage as the title
   and `Workspace · column N` as the subtitle, and gives workspace rows the title and
   `Workspace · N columns, M tabs`. Both hosts render these and their screenshots show them,
   so I kept them rather than churn host tests. Say if you want the one-line form.
9. **`tab.move` is a new command, appended last to the registry.** Making
   `tab.move_to_column` take a slot would have changed a command the hosts already call. The
   index is the drop slot in the column as it is before the move (what a drop indicator
   shows); the core adjusts for a reorder within the column. Appending keeps every existing
   palette and menu order unchanged.
10. **The save counter is separate from the revision.** `revision` stays "something to
   redraw" (quiet scroll records do not bump it). `changes` is "something to save", and
   scroll records do bump it. A restore does not bump `changes` unless it set something
   aside.
11. **Debounce of 1 s, save on quit or hide.** Saving is cheap, so the debounce exists only to
    batch a burst of edits. Web storage is IndexedDB (one `put` per save is atomic). The Mac
    store is a file in Application Support, written atomically, with up to five timestamped
    backups (specs/24 §7).
12. **Not persisted: undo/redo stacks, closed tabs, registrations, collections.** No host
    holds a study collection yet; it joins the store as a 1.x minor when the drawer exists.
13. **Undo of annotation edits stays out of scope.** Highlights, ink and notes are not on the
    command undo stack, as in B072. This could be a later step.
14. **API 0.1.4**: additive, so a PATCH bump while the MAJOR is 0.

## For the host parts

Both hosts build the same behaviour from the same calls. The core is on branch
`b074-core-workspaces`; rebuild the bindings (`just test-swift`, `just wasm-build`). Read
specs/24 first.

**1. Launch and restore** (specs/24 §5).
1. Create the shell as today (`ShellHandle.fixture()` for the fixture-backed shells) and
   `ReadingHandle.placeholder()`, then register titles, languages and
   `setResourceSchemes(reading.schemes())`.
2. Read the store. Mac: `~/Library/Application Support/OpenLogos/user-store.json`. Web:
   IndexedDB database `openlogos`, object store `user-store`, key `current`. If there is
   none, skip to step 4.
3. `importStore(json)`. If it throws (`not_json`, `not_a_store`, `unsupported_version`):
   keep the seed session, copy the bytes to a backup (Mac
   `user-store.backup-<ISO8601>.json`; web key `backup-<ISO8601>`; keep the newest five), and
   show one quiet notice ("Your saved workspace could not be read; a copy was kept."). If
   `report.issues` is non-empty: make the same backup, then show a notice naming what was set
   aside ("1 note could not be restored"; for `session`, "Your workspaces were reset").
4. Set `lastSaved = changeCount()`. Render the active workspace. For each reading tab,
   `locate(content_id, reference)`, load the chapter, and scroll
   `scroll_anchor.segment_id` to the top. Then paint `highlightsIn(content_id, chapter_key)`
   and `inkIn(content_id, chapter_key)` for every rendered block (§4 below). The tool well
   comes from the restored `tool`.
5. Measure **file read → first painted text of the active workspace** over at least 20
   launches with the acceptance store (ten workspaces of twelve tabs, 200 notes, 1,000
   highlights, 200 strokes; build it by script through the same API). Budget ≤ 100 ms;
   record p50/p95 in your evidence.

**2. Saving** (specs/24 §4).
1. After every call that may change state (`apply`, `undo`, `redo`, `putNote`,
   `addHighlight`, `removeHighlight`, `addInk`, `removeInk`, `deleteNote`), compare
   `changeCount()` with `lastSaved`. If it differs, (re)start a timer of
   `storeConstants().save_debounce_ms` (1000 ms).
2. When the timer fires, and at once on quit or hide, run `json = exportStore()` and write
   it, then set `lastSaved` to the count read before `exportStore`. **Call `exportStore()`
   on the main thread** (the Mac shell's main actor, the page's main thread). It costs p95
   0.41 ms after a scroll and under 1 ms after a stroke or a note edit, so no snapshot or
   hop to another thread is needed. Only the write of the returned string goes to the
   background.
   - Mac: write on a serial background queue to `user-store.json.tmp`, then
     `FileManager.replaceItemAt`. Save synchronously in `applicationWillTerminate` and when
     the last window closes.
   - Web: `put(json, "current")` in a `readwrite` transaction. Save on `visibilitychange` to
     `hidden` and on `pagehide`. Call `navigator.storage.persist()` once.
3. On a failed write, keep the old file, retry on the next change, and say so if it fails
   three times in a row.

**3. Notes: the core is the truth.**
1. Delete the host's note map: the web `host.notes` (HTML by id) and the Mac's per-note
   attributed strings as the truth. Load an editor from `note(id)`. A missing note shows the
   seed text and is saved on its first edit.
2. Convert at the edge. A block becomes `<p>`, `<h2>`, `<li>` (inside one `<ul>` per run of
   list items) or `<blockquote>` on the web. On the Mac it is a paragraph with the B076
   paragraph style for that kind. A span becomes text with bold, italic and link (`a[data-ref]`
   on the web; the link attribute on the Mac). Going back: walk the editor's blocks and runs,
   merging adjacent runs with the same marks; links keep their OSIS reference. Round-trip
   test: load the fixture note, save it unchanged, and `note(id)` must be equal.
3. Call `putNote` when an edit settles: on input debounced to at most 300 ms, and on blur.
   The tab and the dock showing one note both read the core, so they stay in step.

**4. Highlights.**
1. Replace `host.highlights` (web) and `ReaderMark.spans` (Mac) with the core. On a highlight
   action, build `{resource_id, content_revision, spans: [{segment_id, start, end}], color}`
   from the selection's anchors (scalar offsets, one span per segment, in text order) and call
   `addHighlight(h, reading)`. The `placeholder_text` refusal replaces the host's own
   placeholder check for this action; show its reason as today.
2. When a chapter block renders, paint `highlightsIn(resource_id, chapter_key)`. When a block
   is released, its paint goes. Remove by id: an eraser on a highlight, or "Remove highlight"
   if the host offers it.

**5. Ink.**
1. Replace `host.ink` (web, keyed by view) and `InkSheet.anchors` (Mac). When a stroke ends,
   find the verse under its first point (`segment_id`) and that verse's top in the column
   (`top`). Convert the points to `x` from the text's leading edge and `y - top`, then call
   `addInk({resource_id, segment_id, stroke})`. Keep the returned id on the drawn stroke.
2. Draw `inkIn(resource_id, chapter_key)` for each rendered block at `verseTop(segment_id) +
   y`. The eraser calls `removeInk(id)` for every stroke it touches.

**6. Drag and resize.**
1. Tab drag: on drop, send `tab.move {tab_id, column, index}`, where `index` is the slot
   under the drop indicator in the target column as drawn before the move. To drop into a new
   column, send `{tab_id, new_column: true, column: N}`, with N the number the new column will
   have. `already_in_column` means the drop changed nothing; send nothing in that case. ⌘Z
   undoes it, and `undo_title` reads "Undo Move tab to position".
2. Column resize: keep the live drag in the host, and send one `column.resize {column,
   number}` when the drag ends. ⌘Z undoes it.

**7. Lexicon language.** Read `lexicon_language` from the tab view (`grc`, `hbo`) instead of
working out the language from the column where the word was clicked, so a restored lexicon tab
looks the word up in the right lexicon.

**8. Host evidence to record.** Quit with three workspaces and relaunch: the same columns,
tabs, scroll positions to the verse, tool, notes, highlights and ink, with screenshots before
and after. Damage the file and relaunch: the notice, a backup on disk or in IndexedDB, and a
fresh session. Measure save, restore and workspace new/rename/duplicate/close/switch on screen
at the acceptance size, and show that a drag can be undone.

## Skipped

- Host UI, host storage and the on-screen restore numbers: these are the web and Mac B074
  parts.
- Study collections in the store (no host holds one yet; decision 12).
- Undo of note, highlight and ink edits (decision 13).
- Reanchoring stored highlights and ink when an edition's revision changes (B071 brings real
  revisions; lineage already exists in core).
- Sync of the store between devices (crates/sync's operation log is the later path).
