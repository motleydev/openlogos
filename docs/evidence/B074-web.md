# B074 evidence (web): the user store, session restore, drag and resize undo

Recorded 4 November 2026 on the Mac mini (Apple M1, Chrome 156, headless) in the worktree
`openlogos-b074web`, branch `b074-web-store`. That branch is `b078-web-reading` with
`b074-core-workspaces` (a138a6d) merged by the lead. A Claude Code subagent did the work while
the owner slept. Nothing was pushed or merged. This branch does not change `crates/`. No
simulator, keychain or macOS user was touched. No Bible text appears beyond the conformance
fixtures. The Mac half is `b074-mac-store` and has its own evidence. Specs: specs/24-persistence.md, and specs/22 and specs/23
for the shell and the stream. The step list followed is docs/evidence/B074-core.md, "For the
host parts".

## What was built

| Item | Where |
|---|---|
| **The store on the page**: IndexedDB database `openlogos`, object store `user-store`, key `current`. The value is the string `exportStore()` wrote. A `put` in a `readwrite` transaction (atomic) saves it. Backups are kept under `backup-<ISO8601>`, the newest five. `navigator.storage.persist()` is asked once. `?store=off` keeps nothing; this is the tests' default under `autotest=1`. `?store=<name>` uses `openlogos-<name>`, which the tests and benches use, one database per case | `apps/web/shell/store.js`, `openStore` |
| **Restore at launch** (specs/24 §5): the seed session, titles, languages and schemes, then the store read and `importStore`. If the core refuses the file whole, the seed session is kept, the file is moved aside (backup written and `current` deleted in one transaction) and one notice is shown. A restore with `issues` keeps a backup of what was read and names what was set aside. Then `lastSaved = changeCount()`. If something was set aside, the count is taken from before the import, so the repaired store is saved. After that the active workspace renders and each stream opens at its `scroll_anchor`. Other workspaces render when switched to | `restoreStore`, `main` |
| **Saving** (specs/24 §4): after every `run()` (so every apply, undo and redo), every highlight, stroke, erase and settled note edit, `changed()` restarts a timer of `storeConstants().save_debounce_ms` (1000 ms) if the count moved. `saveNow()` runs `exportStore()` on the main thread and hands the string to IndexedDB. If a write fails, the save stays due. Three failures in a row are reported once | `changed`, `saveNow` |
| **Save on hide**: `visibilitychange` to `hidden` and `pagehide` call `saveNow()`. If a save has not committed yet, the string is also written to `localStorage` (`<db>:current:pending`) synchronously. The next launch takes that copy first, writes it to IndexedDB and clears it. Why: in Chrome, an IndexedDB transaction started as the page is closed or reloaded is dropped when the page is torn down. The test `store-saves-after-debounce-and-on-hide` showed a change made 0.2 s before a reload was lost without this copy. See deviation 1 | `saveOnHide`, `UserStore.keepPending` |
| **Notes: the core is the truth.** The editor loads `note(id)`. A missing note shows the fixture note's text and is saved on its first edit. Each block becomes `<p>`, `<h2>` (the Heading button now makes `h2`; `h3` from before still reads as a heading), `<li>` in one `<ul>` per run, or `<blockquote>`. Spans become `<b>`, `<i>` and `a.passage-link[data-reference]`. Going back walks the editor's blocks: a `<div>` is a paragraph, a `<br>` between text breaks the block, and adjacent runs with the same marks merge. `putNote` runs 300 ms after the last input and on blur, and before a save | `note-form.js`, `loadNote`, `noteChanged`, `flushNote` |
| **Highlights in the core**: a highlight action builds `{resource_id, content_revision, spans: [{segment_id, start, end}], color}` from the selection's anchors (scalar offsets, one span per segment) and calls `addHighlight(h, reading)`. The core's `placeholder_text` refusal replaces the host's check for this action, with the same words ("Placeholder text can’t be quoted or collected"). Each rendered block paints `highlightsIn(resource, chapter)`. The ranges are computed straight on the verse's text node from the scalar offsets, once per block | `highlightSelection`, `blockHighlights`, `paintHighlights` |
| **Ink in the core**: when a stroke ends, the verse under its first point is found from the line boxes of the verse spans (the point's own line, not the union box of a verse wrapping across lines). `x` is measured from the text's leading edge (mirrored in a right-to-left text) and `y` from the verse's top, then `addInk`. Each rendered block draws `inkIn` at `verseTop + y`. The eraser calls `removeInk` | `attachInk`, `verseAnchorAt`, `repaintInk` |
| **Tab drag** (new on the web): a pointer drag on a tab, after 5 px, shows a ghost label and a drop indicator. On a strip it is a bar at the slot, counted as drawn before the move. Over another column's text, that column is outlined (drop at its end). In the last 56 px of the trailing column, a new-column zone appears. On drop the page sends `tab.move {tab_id, column, index}` or `{tab_id, new_column: true, column: N+1}`. A drop back into the tab's own slot, or a new column from a column holding only that tab, sends nothing and draws no indicator. ⌘Z undoes it ("Move tab to position") | `startTabDrag`, `dropTarget`, `noOpDrop`, `showDrop` |
| **Column resize**: unchanged from B072 (one `column.resize` when the drag ends). ⌘Z now has a test, and the widths come back on screen | `startResize` |
| **Lexicon language**: the lexicon page reads `tab.lexicon_language` (`grc`, `hbo`), not the host's map from where the word was clicked, so a restored lexicon tab is in Greek | `lexiconPage` |
| **Palette across workspaces**: the core's (`@` rows of every workspace, which switch to them). Checked after a restore | the test below |
| **At the acceptance size**: the speed run first measured restore at p50 229 ms and a workspace switch at 200 ms. Three things were slow: `inkIn` / `highlightsIn` called for every block of every column on each render (serde of thousands of point objects), `anchorRange` walking every text node of a chapter for each highlight, and the ink canvas repainted several times in one frame. Now `inkIn` and `highlightsIn` are cached per edition and chapter and dropped only for the chapter that changed. Highlight ranges are cached per block. Each stroke's outline is computed once, in its own coordinates, and placed with the canvas transform. Ink repaints at most once a frame (requestAnimationFrame) | `chapterInk`, `blockHighlights`, `outlineOf`, `scheduleInk` |

## The six "must work" items

All six are driven by trusted WebDriver input in `just test-shell-web`. A quit is a page reload
with nothing waited for, so the save on hide is what keeps the state.

1. **Everything comes back** (`store-restores-everything`). The setup: a highlight drawn on BSB Jn 3:16 ("eternal life"),
   a pen stroke on Jn 3:17, a note typed with ⌘B "grace" and a Link passage to Jn 3:17, a
   lexicon tab opened from Greek ἠγάπησεν, a third workspace navigated to Acts 2:4 and scrolled
   so an Acts 2 verse is on the reading line, then back to workspace 1 and the Highlight tool.
   After the reload, the workspace order and titles, each column's tabs, shown tab,
   reference, link mode, widths and anchors, the tool (well pressed on Highlight), the
   highlight (painted on the same words), the ink (painted at Jn 3:17 within 25 % of its pixels
   before), the note (bold span and link in the core and in the editor) and the lexicon tab
   (`lang="grc"`, "Greek · John 3:16") all match. Nothing is left to undo (`undo_title` null),
   and no notice is shown. Screenshots: `web-store-before-quit.png` / `web-store-after-relaunch.png`.
2. **Placeholder highlight refused with the same words**: `reading-placeholder-actions`
   (B078's case) passes against the core's refusal: the toast reads "Placeholder text can’t be
   quoted or collected" and nothing is stored.
3. **Drag undo and resize undo** (`tab-drag-and-resize-undo`). WLC Gen 1 dragged onto column 2's strip
   goes to `[[BSB], [SBLGNT, WLC], [Note]]`. The undo title is "Move tab to position", and ⌘Z
   puts it back. Dragged into the new-column zone, it becomes a fourth column, and ⌘Z removes it.
   A drop back into its own slot sends nothing and shows no toast. A plain click still shows the
   tab. A rule drag sends one `column.resize`, and ⌘Z restores the widths in the session and in
   the columns' `flex-grow`. Screenshots: `web-store-drag-in-flight.png`, `web-store-drag-undone.png`.
4. **The palette across workspaces**: after the relaunch, `@acts` typed in workspace 1 lists
   "BSB · Acts 2:4 — Workspace 3 · column 1". Enter switches to that workspace, and the anchored
   Acts verse is on the reading line (within 40 px). Screenshot: `web-store-palette-workspaces.png`.
5. **A damaged store** (`store-damaged-file`, `store-backups-keep-five`). A truncated file gives
   "Your saved workspace could not be read; a copy was kept." on the seed session ("Study").
   One `backup-…` key is kept, `current` is gone, and the next change saves a fresh store that
   restores quietly. A newer major (`2.0`) gives "Your saved workspace was written by a newer
   version of OpenLogos; a copy was kept.". A store with a dangling active workspace and one bad note gives
   "Your workspaces were reset; 1 note could not be restored; a copy was kept.". The good note is
   kept and the repaired store is due to save. Seven damaged launches leave five backups, the
   newest being the last damaged file. Screenshot: `web-store-damaged-notice.png`.
6. **A restored lexicon tab looks up in Greek**: item 1 (`lexicon_language` `grc` from the tab).
   The B076 case `word-study-lexicon` also passes on the core's language: "Greek · John 3:16".

Two more cases: `store-note-round-trip` loads `examples/user-store.json`'s fixture note into the
editor, sends it back unchanged, and `note(id)` is equal. The DOM is `<h2>…<a>`, `<i>works</i>`,
`<b>from above</b>`, `<ul><li>`, `<blockquote>`, `<p><br></p>`. A keystroke reaches the core
within the 300 ms settle. `store-saves-after-debounce-and-on-hide` checks a change saved 1 s
later from IndexedDB with nothing pending, and a change 0.2 s before a reload restored from
the copy kept on hide and then written to IndexedDB.

## Lockstep with the Mac

Proposed to the Mac agent (b074mac) at the start of the work. There was no reply by the time of this
report, so the lead should check that the two hosts match:

- Refused store: "Your saved workspace could not be read; a copy was kept." (B074-core's
  words). For `unsupported_version`, specs/24 §6 asks that a newer version be named: "Your saved
  workspace was written by a newer version of OpenLogos; a copy was kept."
- Issues: parts in the order session, notes, highlights, ink: "Your workspaces were reset",
  "N note(s) / highlight(s) / ink stroke(s) could not be restored", joined by "; ", then
  "; a copy was kept."
- Three failed writes in a row: "Your changes could not be saved; the last saved copy is kept."
- A notice is shown once and stays about 6 s (a toast on the web); it is never modal.
- Undo titles are the core's, as given ("Move tab to position").
- Ink `x` is measured from the text's leading edge, growing into the text (mirrored for
  right-to-left), and `y` from the verse's top.
- Drop targets: a strip slot (index as drawn before the move); another column's text (its end);
  the trailing column's last 56 px (a new column). Backups: the newest five.

## Speed

Headless Chrome 156, 1440 × 900, M1, on this branch. Mac agent builds were running on the
same machine. `just test-shell-web --bench --samples 200 --launches 30`. "work" is the
synchronous script, style and layout of the change; "frame" runs to the next
`requestAnimationFrame` (and so includes that wait). Both are p50 / p95 ms.

**Acceptance store**, built through the API by `buildAcceptance()`: the bench session's ten
workspaces of twelve tabs (120 tabs), 200 notes, 1,000 highlights on the fixture verses and
200 strokes of 120 points with pressure across John 3 and Genesis 1. That is 539,130 bytes as
written. It is smaller than the core bench's 766 KB because these strokes have pressure but no
altitude or azimuth.

| Row (budget) | p50 / p95 | n |
|---|---:|---:|
| Restore: IndexedDB read | 1.0 / 1.8 | 30 launches |
| Restore: read + `importStore` | 16.3 / 17.7 | 30 |
| Restore: read → active workspace rendered and laid out | 44.5 / 46.5 | 30 |
| **Restore: read → first painted text** (next frame) (≤ 100 ms) | **54.8 / 57.1** | 30 |
| `exportStore` after a scroll | 0.2 / 0.3 | 200 |
| `exportStore` after a stroke | 0.5 / 0.7 | 200 |
| `exportStore` after a note edit | 0.8 / 0.9 | 200 |
| **Save on the main thread** (`saveNow`: export plus the queued put; never a frame) | **0.3 / 0.4** | 100 |
| Save → IndexedDB committed (off the main thread) | 6.7 / 11.8 | 100 |
| `workspace.new`, work / frame (≤ 100 ms) | 6.6 / 7.1 · frame p95 21.5 | 200 |
| `workspace.rename` | 1.4 / 3.4 · 16.6 | 200 |
| `workspace.duplicate` | 15.2 / 22.3 · 28.4 | 200 |
| `workspace.close` | 15.1 / 23.5 · 29.3 | 200 |
| `workspace.switch` (`workspace.next`) | 14.3 / 17.2 · 24.8 | 200 |

Before the caching described above, the first run of the same rows measured: first painted text
p50 229 ms, `workspace.next` p50 185 ms work, `duplicate` / `close` about 190 ms.

**B072–B078 rows, B078 evidence → this branch** (work p95 ms, B078 run 1 → B074; budget 16 ms
unless stated). There is no regression beyond run-to-run noise:

| Interaction | seed | bench |
|---|---:|---:|
| Palette keystroke | 1.0 → 2.7 | 2.9 → 2.9 |
| Keystroke with the Text section | 0.7 → 1.4 | 1.4 → 1.4 |
| Trusted keystroke → palette rendered (handler) | 2.1 → 2.6 | 2.3 → 2.1 |
| Tab switch (`tab.next`) | 3.8 → 5.2 | 6.6 → 6.9 |
| Column focus | 3.2 → 2.5 | 4.3 → 3.0 |
| Workspace switch (≤ 100 ms) | 8.6 → 8.7 | 14.8 → 14.0 |
| Tool change | 7.8 → 7.3 | 8.7 → 8.0 |
| B076 translation ↔ original / → notes | 2.6 / 2.5 → 2.6 / 2.5 | — |
| B076 selection group | 3.2 → 3.5 | 3.1 → 2.5 |
| B077 menu opens (word / tab / background) | 4.5 / 3.2 / 2.6 → 4.9 / 3.2 / 2.5 | 4.6 / 3.2 / 2.3 → 3.1 / 2.5 / 2.4 |
| B077 choose → Compare / Search | 2.1 / 3.1 → 2.0 / 2.8 | 4.2 / 3.2 → 4.8 / 3.2 |
| B077 trusted right-click → menu drawn (handler) | 5.2 → 4.1 | 5.7 → 5.1 |
| Open a chapter, warm (≤ 100 ms) | 7.0 → 6.2 | 10.1 → 11.6 |
| B078 typed jump → stream painted (≤ 100 ms) | 7.7 → 7.1 | 13.2 → 12.5 |
| B078 scroll frame work p95 / frames over 25 ms | 2.0 / 0 → 1.9 / 0 | 0.9 / 0 → 0.8 / 0 |
| B078 linked follow | 1.5 → 1.5 | 2.2 → 3.8 |

The seed palette keystroke p95 (1.0 → 2.7) is the only row that moved by more than 1 ms. The
palette code path is unchanged on this branch; its frame p95 is 16.4 ms either way.

## Lanes

Run on the committed code at ceed83a. This evidence file is the only later commit.

| Lane | Summary line | Exit |
|---|---|---:|
| `nix develop .#core -c just ci` | `Summary [8.947s] 337 tests run: 337 passed, 0 skipped`; `PASS: 37 schema/example validations; … 22 conformance fixtures; … 139 negative tests …` | 0 |
| `just test-web` (`.#web`) | `conformance cases: 1844 passed, 0 failed (api 0.1.4)` | 0 |
| `just test-wasm` | `test result: ok. 10 passed; 0 failed` | 0 |
| `just test-dom-reader` | `DOM reader: 13/13 cases passed` | 0 |
| `just test-shell-web` | `Summary: Web shell: 60/60 cases passed` (54 before B074 plus 6 new; `registry-bindings` now counts 85 commands with `tab.move`) | 0 |
| `just test-shell-web --bench --samples 200 --launches 30` | `Summary: Web shell: 60/60 cases passed; B074 store bench: 30 launches, 200 samples per row; bench: headless Chrome, 200 samples per interaction` | 0 |

Disk stayed between 2.9 and 7.6 GiB free, because the Mac agent was building at the same time.
Nothing was deleted, and this worktree's `target/` is 2.1 GiB.

## Deviations

1. **A synchronous copy on hide** (beyond specs/24 §7). IndexedDB stays the store, but a save
   that has not committed when the page is hidden or unloaded is also put in `localStorage`
   (`openlogos:current:pending`). The next launch prefers it, writes it to IndexedDB and removes
   it. Without this, Chrome dropped an IndexedDB transaction started in `pagehide`, and a change
   in the last second before a reload or tab close was lost (shown by the test). The copy is
   only written at hide, never on the 1 s save. It is bounded by localStorage's quota (about 5
   M UTF-16 units); a quota error is ignored and the IndexedDB write still runs. specs/24 §7
   could name this copy; that change was not made here (no spec edits from a host part).
2. **A refused store is moved aside**: it is backed up and `current` is deleted (one
   transaction), so the next launch does not repeat the notice. specs/24 §6 says "move the file
   aside", and this is the IndexedDB form of that.
3. **Headings are `<h2>`** in the note editor as B074-core asks. The B076 editor made `<h3>`, and
   both read back as `heading`. The B076 test now expects `<h2>`.
4. **`undo_title` is "Move tab to position"**. B074-core's step 6 wrote it as "Undo Move tab to
   position", but the core returns the title without the verb. The test checks the core's string.
5. **Snapshot shape** (tests only): `highlights` and `ink` now come from the core's store
   (`ink` per edition, not per tab), and `notes` is each note editor's HTML as shown.

## Decisions for the owner

- Whether to accept the `localStorage` copy on hide (deviation 1) and record it in specs/24 §7,
  or prefer a different mechanism (for example a `beforeunload` prompt, which the product
  should avoid).
- The notice wording for a newer version and for set-aside parts (proposed above; the Mac must
  match).
- The new-column drop zone: the trailing 56 px of the last column. An alternative is a
  visible "+ column" target that appears only during a drag.
- Two tabs of the page in one browser each save their own session, so the last writer wins.
  Multi-window sessions are out of scope (specs/24 §9). A `BroadcastChannel` warning could come
  later.

## Skipped

- Undo of highlight, ink and note edits (specs/24 §9, decision 13 of B074-core).
- Removing a highlight from the UI: there is no host affordance yet (`removeHighlight` is wired
  through nothing). The eraser removes strokes only, as in B072.
- Study collections in the store (no host holds one).
- A notice for a failed IndexedDB *open* (private mode in some browsers): the page logs it and
  runs without saving.
- A headed run: the numbers are headless Chrome.
