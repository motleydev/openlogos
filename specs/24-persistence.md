# Persistence: the user store, save policy and session restore

Revision 1, 2026-11-04 (B074). This file adds to specs/22 (the shell) and specs/23
(continuous reading). It says what survives a quit, in what form, when hosts save, how a
launch restores, and what happens to a damaged file. The core defines the form and the
rules (`crates/workspaces/src/store.rs`); hosts only read and write bytes.

## 1. What is kept

One document, the **user store**, holds everything a reader would miss after a quit:

| Part | What | Form |
|---|---|---|
| Session | workspace order, the active workspace, the tool, text size, line spacing, theme, brush width and swatches | the session record, schemas/session.schema.json 1.0 |
| Workspaces | columns with their widths, the active column, the notes dock, per-class column profiles; every tab's kind, edition, passage, **scroll anchor** (B078's stream position: segment and scalar offset), link mode, back/forward history, lexicon word and **lexicon language**, and `highlight_range` | schemas/workspace-2.schema.json 2.0, one record per workspace, in the session's order |
| Notes | every note by id | the block form of §2.1 |
| Highlights | every highlight | segment spans and a colour, §2.2 |
| Ink | every stroke | a B039 stroke pinned to a verse, §2.3 |

Not kept, by design: the undo and redo stacks (a restored shell has nothing to undo), the
recently closed tabs, the palette index, and what the host registers at launch (resource
titles, languages and versification schemes, which come from the installed editions).
Study collections are not kept yet: no host holds one (the Collect tool counts and toasts);
when the collection drawer lands, collections join the store as a new part in a 1.x minor.

## 2. Stored forms

The document (schemas/user-store.schema.json):

```json
{
  "format": "openlogos-user-store",
  "schema_version": "1.0",
  "fixture_only": false,
  "core_api_version": "0.1.4",
  "session": { "schema_version": "1.0", "workspace_ids": ["w1", "w3"], "active_workspace_id": "w1", "tool": "pen", "...": "..." },
  "workspaces": [ { "schema_version": "2.0", "id": "w1", "columns": ["..."], "...": "..." } ],
  "notes": [ { "id": "fixture:note:1", "blocks": ["..."] } ],
  "highlights": [ { "id": "h1", "...": "..." } ],
  "ink": [ { "id": "k5", "...": "..." } ],
  "next_serial": 6
}
```

`examples/user-store.json` is a whole one (the conformance round trip). Ids of highlights and
strokes are minted by the core (`h1`, `k2`, one serial for both) and never reused within a
store. Notes keep the id their tabs and the dock name (`content_id`).

### 2.1 Notes

A note is a list of **blocks**, each a kind and a list of **spans**:

```json
{ "id": "fixture:note:1", "blocks": [
  { "kind": "heading", "spans": [ { "text": "New birth " }, { "text": "Jn 3:3", "link": "John.3.3" } ] },
  { "kind": "paragraph", "spans": [ { "text": "Not by " }, { "text": "works", "italic": true }, { "text": " but " }, { "text": "from above", "bold": true } ] },
  { "kind": "list_item", "spans": [ { "text": "Compare " }, { "text": "Jn 1:12–13", "link": "John.1.12-John.1.13" } ] },
  { "kind": "quote", "spans": [ { "text": "…" } ] },
  { "kind": "paragraph", "spans": [] }
] }
```

Block kinds are exactly the B076 editors' formats: `paragraph`, `heading` (1.25× the reading
size), `list_item` (one bullet each) and `quote` (a rule on the leading edge). A span has
`text` and optionally `bold`, `italic` and `link`, a passage as OSIS (the link opens it in the
last-focused reading column, B076). An empty line is a block with no spans. The core checks
the id and that every link parses as a passage.

**Why not host markup.** The web editor holds HTML and the Mac editor attributed strings. A
stored form neither can read natively would still need two converters; one both can convert
to keeps the core the truth, keeps unsafe HTML out of the file, lets the core read links (for
backlinks later), and survives an editor change. Each host converts at the edge: the web from
its `contenteditable` DOM (`<h2>`, `<li>`, `<blockquote>`, `<b>`/`<strong>`, `<i>`/`<em>`,
`a[data-ref]`), the Mac from `NSAttributedString` runs (font traits, its heading font, its
list and quote paragraph styles, its link attribute). Anything else an editor holds (pasted
colours, fonts) is not stored.

### 2.2 Highlights

```json
{ "id": "h2", "resource_id": "bsb", "content_revision": "bsb-2024",
  "spans": [ { "segment_id": "John.3.16", "start": 100, "end": 137 }, { "segment_id": "John.3.17", "start": 0, "end": 12 } ],
  "color": "#4CC38A" }
```

Offsets are Unicode scalar values, `[start, end)`, in the segment of that edition revision
(specs/03); a selection across verses or chapters is one highlight of several spans, in text
order. The colour is `#RRGGBB`, stored upper case. No quoted text is stored (a highlight never
carries Bible text into the user's file; reanchoring across a revision uses the edition's
lineage, specs/03). **No highlight is stored on placeholder text** (specs/23 §5): the core
refuses one whose spans touch a placeholder segment of the host's `ReadingHandle` with
`placeholder_text`. A segment the reading library does not know (an edition not installed)
is not a placeholder.

A host asks for what to paint when it renders a chapter block: `highlightsIn(resource,
chapterKey)`, oldest first (later ones paint over earlier ones). Removing is by id.

### 2.3 Ink

```json
{ "id": "k5", "resource_id": "bsb", "segment_id": "John.3.17",
  "tool": "pen", "brush_version": "pf-1.2.3/pen-1", "width": 4.0, "color": "#1F6FEB", "opacity": 1.0,
  "xy": [120, -35, 285, 55, 475, 43], "t": [0, 16, 17], "pressure": [null, 50, 12] }
```

In the API a stroke is the ink-document stroke with point objects (`StoreInkMark.stroke`,
schemas/ink-document.schema.json, crates/ink). **In the file it is written compactly**: the
paint flat beside the mark, and the points as columns of fixed-point integers, each
delta-encoded (the first value as is, then the change from the value before; a sensor column
skips its `null`s). `xy` interleaves x and y in tenths of a point; `t` is milliseconds since
the first point; `pressure`, `altitude` and `azimuth` are hundredths and are present only
when some point reported them. The example decodes to (12.0, −3.5, no pressure, 0 ms),
(40.5, 2.0, 0.50, 16 ms), (88.0, 6.3, 0.62, 33 ms). `addInk` rounds a stroke to this
precision on the way in (a tenth of a point is below what a stylus resolves on screen), so
the shell holds exactly what a restore gives back. The form is a fifth of point objects'
size: 200 strokes of 120 points with every sensor are 371 KB instead of about 2.4 MB.

The stroke is the ink-document stroke (schemas/ink-document.schema.json, crates/ink). It is
**pinned to the verse it was drawn at** (B078's host anchor, now stored): `segment_id` is
the verse under the stroke's first point, and the points are in the reading column's
coordinates with `y` measured from that verse's top and `x` from the text's leading edge. A
host draws it at `verseTop(segment_id) + y`, so it moves with its verse as chapters load and
unload, and is hidden while the verse is not rendered. Strokes belong to the edition, not the
tab: every tab showing that edition at that verse shows them. The eraser removes whole
strokes by id. `inkIn(resource, chapterKey)` answers a chapter block.

A stroke drawn at one text size or column width and shown at another keeps its offset from
its verse but not its place in the words; re-flowing ink is out of scope (specs/13 keeps the
snapshot form for that).

## 3. Versions and migration

- `format` must be `openlogos-user-store`; `schema_version` is `MAJOR.MINOR`.
- A core reads **every 1.x**: a later minor only adds fields, which an older core ignores
  (serde defaults). A field renamed or moved inside 1.x gets a step in `STORE_STEPS`
  (`migrate_store`), the forward-version hook; there is none yet.
- Another major is refused (`unsupported_version`): a newer app wrote it. The host keeps the
  file and starts fresh (§6).
- Workspace records inside a store go through the workspace migration
  (`migrate_workspace_to_2`), so a 1.x workspace is accepted and comes back at 2.0. Fields
  added to 2.0 since B070 (`history`, `lexicon_word`, `highlight_range`, `lexicon_language`)
  and the session's `tool`, `reading` and `ink` are optional; a store written before them
  restores with their defaults.

## 4. Save policy

- `ShellHandle.changeCount()` is bumped by every change the store would write: every handled
  command (quiet ones included, so a scroll anchor counts), undo and redo, and every note,
  highlight and ink edit. It is not bumped by registering titles, languages or schemes, or by
  a restore that set nothing aside.
- A host saves when the count differs from the count at its last save, **1 s after the last
  change** (`storeConstants().save_debounce_ms`), and at once when the app quits or the page
  is hidden: Mac on `applicationWillTerminate` and when the window closes; web on
  `visibilitychange` to `hidden` and on `pagehide`. A save writes `exportStore()` as is.
- `exportStore()` writes compact JSON (no whitespace). The notes, the highlights and each
  stroke keep their written JSON until they change, and a restore writes them once, so a save
  rewrites only the session and what changed since the last save. At ten workspaces of
  twelve tabs with 200 notes, 1,000 highlights and 200 strokes the store is 766 KB and a save
  costs p95 0.41 ms after a scroll, 0.55 ms after a stroke and 0.98 ms after a note edit
  (`just bench-shell`); writing every part (only when nothing was restored) is about 5 ms.
- **Hosts call `exportStore()` on the main thread** (the Mac shell's main actor, the page's
  main thread) and hand the string to a background writer; no snapshot of the session is
  needed. Only the file or IndexedDB write runs off the main thread.
- A note editor saves into the core (`putNote`) as its text settles (on input, debounced by
  the editor at most 300 ms, and on blur), so the store's debounce sees it.

## 5. Restore order

At launch:

1. Create the shell the host always starts with (its seed session; `ShellHandle.fixture()`
   for the fixture-backed hosts today) and the `ReadingHandle`.
2. Register resource titles, languages and schemes (`setResourceTitles`,
   `setResourceLanguages`, `setResourceSchemes(reading.schemes())`).
3. Read the store file. No file: stop here; the seed session is the fresh session.
4. `importStore(json)`. Refused (§6): keep the seed session, move the file aside as a backup,
   tell the reader once. Issues: keep a backup of the file read, tell the reader once what
   was set aside.
5. Remember `changeCount()` as saved (a restore that set something aside moved it, so the
   next save writes the repaired store).
6. Render the active workspace's shown tabs first: for each reading tab `locate` its
   `reference`, load the chapter, scroll `scroll_anchor.segment_id` (and its scalar offset)
   to the top (specs/23 §7), paint `highlightsIn` and `inkIn` for each rendered block; then
   the tool well from the restored tool. Other workspaces render when switched to.

End to end, launch to the restored workspace's text is the specs/22 §7 cold-launch budget
(≤ 1 s); restore from file read to the first painted text of the active workspace is held to
≤ 100 ms (host numbers in the host evidence).

## 6. Failure behaviour

A damaged or foreign file never crashes the app and is never silently dropped.

| Input | Core | Host |
|---|---|---|
| No file | `empty` (if read anyway) | fresh session; nothing to back up |
| Not JSON (truncated, overwritten) | `not_json`, shell unchanged | fresh session; move the file aside as a backup; tell the reader |
| JSON, not a store (no `format`, another format, no version, not an object) | `not_a_store`, shell unchanged | as above |
| A newer major | `unsupported_version`, shell unchanged | as above; the message says a newer version wrote it |
| A readable store whose session does not check (dangling ids, invalid workspace) | report: `session_restored: false`, issue `session`; the shell keeps its seed session; notes, highlights and ink restored | keep a backup; tell the reader the workspaces were reset |
| One note, highlight or stroke that does not check, or repeats an id | report: that item set aside with its id and reason; everything else restored | keep a backup |
| A write fails | — | keep the previous file (atomic write), retry on the next change, tell the reader if it keeps failing |

Backups: at most five, newest kept, named with the time; never overwritten by a save.

## 7. Host storage

- **Mac**: `~/Library/Application Support/OpenLogos/user-store.json` (inside the sandbox
  container when sandboxed). Atomic write: write `user-store.json.tmp` in the same directory,
  then `FileManager.replaceItemAt` (or `rename(2)`). Backups `user-store.backup-<ISO8601>.json`
  beside it. Saves run on a serial background queue; the quit path saves synchronously.
- **Web**: IndexedDB, database `openlogos`, object store `user-store`, key `current`, value the
  JSON string as written (one `put` per save in a `readwrite` transaction, which is atomic);
  backups under `backup-<ISO8601>`. The page asks `navigator.storage.persist()` once. IndexedDB
  over OPFS: it is available in every target browser's main thread, its transactions give the
  atomic replace for free, and the store is one small string; OPFS's synchronous handles need
  a worker for no gain at this size.

## 8. API

Rust (`openlogos_workspaces::store`, methods on `commands::Shell`), UniFFI (Swift/Kotlin
camelCase), wasm (JSON objects, errors thrown with `kind`).

| Rust | UniFFI | wasm |
|---|---|---|
| `Shell::export_store_json()` / `export_store()` | `ShellHandle.exportStore() -> String` | `exportStore(): string` |
| `Shell::import_store_json(&str) -> Result<RestoreReport, StoreError>` | `importStore(json:) throws -> StoreRestoreReport` | `importStore(json): StoreRestoreReport` |
| `Shell.changes` | `changeCount() -> UInt64` | `changeCount(): number` |
| `Shell::note`, `put_note`, `delete_note`, `annotations.notes` | `note(id:)`, `putNote(note:)`, `deleteNote(id:)`, `notes()` | `note`, `putNote`, `deleteNote`, `notes` |
| `Shell::add_highlight(h, Some(&check))`, `remove_highlight`, `highlights_in` | `addHighlight(highlight:reading:)` (reading optional), `removeHighlight(id:)`, `highlightsIn(resourceId:chapterKey:)` | `addHighlight(h, reading)` (reading required), `removeHighlight`, `highlightsIn` |
| `Shell::add_ink`, `remove_ink`, `ink_in` | `addInk(mark:)`, `removeInk(id:)`, `inkIn(resourceId:chapterKey:)` | `addInk`, `removeInk`, `inkIn` |
| `SAVE_DEBOUNCE_MS`, `STORE_FORMAT`, `STORE_SCHEMA_VERSION` | `storeConstants()` | `storeConstants()` |
| `StoreError` (`kind` tag) | `StoreCallError.Refused(kind, reason)` | `Error` with `kind`, `details` |

Workspace commands added for the acceptance: `tab.move` (argument shape `tab_move`: `tab_id`,
default the active tab; `column` and `index`, the 0-based slot in that column as it is before
the move, default the end; or `new_column` with an optional `column` number for the new
column) is what a tab drag ends in, undoable; a no-op drop is refused (`already_in_column`).
`column.resize` is already undoable; a host sends it once when a column-rule drag ends.
`tab.open_lexicon` records the clicked edition's language on the lexicon tab
(`lexicon_language`).

## 9. Out of scope

Sync of the store between devices (crates/sync's operation log is the later path; the store
is the local snapshot it would start from), undo of highlight, ink and note edits (they keep
their B072 behaviour: not on the command undo stack), study collections (§1), re-flowing ink
across layouts, multi-window sessions, and encryption at rest beyond the platform's.
