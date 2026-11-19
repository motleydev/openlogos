# B074 evidence (Mac): the user store, session restore, drag and resize undo in the Mac shell

Recorded 4 November 2026 on the Mac mini (Apple M1, macOS 26.4, Xcode 26.6). A Claude Code
subagent did the work overnight while the owner slept, in the worktree `openlogos-b074mac` on
branch `b074-mac-store`. That branch is `b078-mac-reading` with `b074-core-workspaces`
(a138a6d) merged in by the lead. Nothing was pushed or merged. No simulator, keychain or macOS
user was touched. Every run used a store directory under `artifacts/b074/` or the test's
temporary directory, never `~/Library/Application Support/OpenLogos`. Only placeholder text and
the conformance fixtures' verses are on screen. The core part is B074-core.md and the spec is
specs/24-persistence.md. The web half is `b074-web-store`.

## What works (the owner's six items)

| # | Item | How |
|---|---|---|
| 1 | Quit with three workspaces, column 2 scrolled to Acts 2, the Highlight tool, a note with a heading, a bold phrase and passage links, a highlight on BSB John 3:16 and ink on John 3:17. Relaunch and everything is back: the workspaces, columns and tabs; Acts 2:14 on the reading line; the tool; the note with its formatting and links; the highlight on its words; the ink on its verse, drawn where it was. | `ShellModel(store:)` restores before anything renders (specs/24 §5): `importStore`, then the seed session's place is taken. The streams open at each tab's `scroll_anchor`. Highlights come from `highlightsIn` for every block in a window. Ink comes from `inkIn` a frame after the text. The note editor loads `note(id)`. ⌘Q runs `applicationWillTerminate`, which saves at once. Pictures `mac-store-before-quit.png` and `mac-store-after-relaunch.png`. Test `testAQuitAndRelaunchBringsEverythingBack` checks every part, and also that saving again writes the same bytes. |
| 2 | Highlighting placeholder text is still refused with the same words, "Placeholder text can’t be quoted or collected". | The core refuses it (`addHighlight(…, reading:)` with `placeholder_text`), and the host shows its own words for that kind. The well and the menu still disable the action ahead of time, as in B078. `testRealFixtureVersesKeepTheirTextAndPlaceholdersCannotBeQuoted` now also checks that nothing was stored. |
| 3 | Dragging a tab to another column, or into a new column, then ⌘Z puts it back. Resizing a column, then ⌘Z restores the width. | You drag a tab by its label. While you drag, a 2 pt accent bar marks the slot in the target strip, the label follows the pointer, and a "New column" band appears at the last column's trailing edge. The drop sends one `tab.move {tab_id, column, index}` or `{tab_id, new_column, column}`. A drop that changes nothing sends nothing. The palette's Undo row reads "Undo · Move tab to position" (the core's title). A rule drag sends one `column.resize` when it ends, as in B072. Pictures `mac-store-drag-slot.png` and `mac-store-drag-undo.png`. Test `testATabDragAndAColumnResizeUndo`. |
| 4 | The palette finds tabs and workspaces in every workspace and switches to them. | The core's rows render as before. "Rom" lists the Romans workspace and its BSB Rom 8:1 tab from inside Study. Picture `mac-store-palette-workspaces.png`. Test `testThePaletteSwitchesToAnotherWorkspacesTabAndToAWorkspace`. |
| 5 | A damaged file shows the notice on relaunch, a backup exists, and the app works. | A refused file (`not_json`, `not_a_store`) gives "Your saved workspace could not be read; a copy was kept." A newer major gives "…written by a newer version of OpenLogos; a copy was kept." Parts set aside give, for example, "Your workspaces were reset; 2 highlights could not be restored; a copy was kept." Each is shown once, for 6 s. The bytes go to `user-store.backup-<ISO 8601>.json` (newest five kept). A refused file is moved aside, so the next launch does not repeat the notice. The seed session opens. Picture `mac-store-damaged-notice.png`; the backup is left in the store directory. Tests `testADamagedFileShowsTheNoticeKeepsABackupAndTheAppWorks`, `testANewerVersionAndSetAsideParts`, `testBackupsKeepTheNewestFive`, `testAFailingWriteKeepsTheOldFileAndSaysSoOnTheThirdTime`. |
| 6 | A restored lexicon tab looks up in the right language. | `LexiconView` reads the tab's `lexiconLanguage` (from the clicked edition) before guessing from the word's script. Test `testARestoredLexiconTabLooksUpInTheClickedEditionsLanguage`. |

### What was built

| Item | Where |
|---|---|
| `UserStoreFile`: Application Support path; atomic write (`user-store.json.tmp`, then `replaceItemAt`) on a serial queue, with a synchronous path for quitting; backups | `Sources/Shell/ShellStore.swift` |
| `ShellPersistence`: restore, notice wording, debounce (`storeConstants().saveDebounceMs`), save on quit or window close, failure count, the store a launch uses | `Sources/Shell/ShellStore.swift` |
| Restore before the first render; `storeChanged()` after `run`, scroll reports, notes, highlights and ink | `ShellModel.init(…, store:)`, `ShellModel.storeChanged` |
| Notes as stored blocks: `NoteStore.attributed(_:)` and `storeNote(id:from:)` (heading and quote paragraph marks, `• ` list items, bold and italic traits, passage links); `putNote` 300 ms after the last edit and on blur, flushed before every save | `Sources/Shell/NoteEditor.swift` |
| Highlights in the core: UTF-16 ⇄ Unicode scalar spans (`ReaderLayout.storeSpan`, `segmentSpan`); one span per segment; painted from `highlightsIn` for every block in a window (`ReadingStream.storedMarks`) | `ShellModel.highlight`, `storedMarks`, `ChapterLayout.swift`, `ContinuousReader.swift` |
| Ink in the core: a finished stroke is pinned to the verse under its first point (`x` from the text's leading edge, mirrored for right-to-left editions; `y` from the verse's top), stored with `addInk` and kept under the core's id. Every tab of that edition shows it. The eraser calls `removeInk`. A window change asks only for chapters it has not seen. Outlines are cached across canvases. | `ShellModel.storeInk`, `syncStoredInk`, `InkSheet.onCommit/onErase/syncStored`, `InkCanvasEngine.sharedPath` |
| Tab drag: frames in the columns' coordinate space, `tabDrop` (slot, end of a column, new column, nil for a no-op), `TabDropOverlay` | `ColumnsView.swift`, `ShellModel` (Tab drag) |
| Lexicon language from the tab | `LexiconView.language(_:word:)` |
| Launch arguments for pictures and measurements (`-OPENLOGOS_STORE`, `_BUILD_STORE`, `_STORE_DEMO`, `_DRAG`, `_RESTORE_MEASURE`, `_STORE_MEASURE`); the acceptance store built through the API | `Sources/Shell/StoreLaunch.swift`, `scripts/b074-mac-shots.sh` |

## Speed

Release build (`artifacts/b074/release`, arm64), window 1440 × 900. The acceptance store is
built through the API (`-OPENLOGOS_BUILD_STORE`): ten workspaces of twelve tabs, every tab
scrolled; 200 notes of five blocks; 1,000 highlights; 200 strokes of 120 points with
pressure, altitude and azimuth. It comes to 617,381 bytes. "Drawn" rows time the command plus
the window laid out and drawn synchronously, as in B072's `ShellMeasure`.

### Restore (20 launches each, `-OPENLOGOS_RESTORE_MEASURE`)

| From the file read (in `ShellModel.init`) to … | p50 ms | p95 ms | max ms |
|---|---:|---:|---:|
| file read | 0.13 | 0.17 | 0.17 |
| `importStore` (core) | 33.75 | 33.95 | 34.95 |
| highlights as marks (main-thread share) | 7.30 | 7.36 | 7.44 |
| **the active workspace's text laid out in the view** | **135.08** | **137.93** | **138.11** |
| the first display frame after it | 266.35 | 273.11 | 274.15 |
| *The same launch with no file:* text laid out | 96.29 | 99.77 | 102.71 |
| *The same launch with no file:* first display frame | 185.25 | 200.09 | 206.55 |

**Over the 100 ms budget, end to end.** Launching the Mac shell spends about 96 ms between
creating the model and laying out text, even with no file (SwiftUI builds the window, the
toolbar and the three columns). The restore adds about 39 ms to that: 34 ms of `importStore`
and 7 ms of highlight marks. The window then reaches the screen about 130 ms later (with no
file, about 90 ms). The ink is held until after that first frame (`whenTextIsOnScreen`). It used
to cost about 110 ms before the frame: fetching and converting 60 strokes, plus their outlines.
The cold-launch budget (specs/22 §7, ≤ 1 s) is met. See owner decision 1.

### Saves, workspace commands, drag and resize at the acceptance size (200 samples each, `-OPENLOGOS_STORE_MEASURE`)

| Interaction (budget) | p50 ms | p95 ms | max ms |
|---|---:|---:|---:|
| Save after a scroll: `exportStore` on the main thread (16) | 0.36 | 0.39 | 0.79 |
| … the write, off the main thread | 1.01 | 2.42 | 13.16 |
| Save after a stroke: `exportStore` on the main thread (16) | 1.27 | 2.26 | 2.35 |
| … the write, off the main thread | 1.30 | 1.74 | 6.53 |
| Save after a note edit: `exportStore` on the main thread (16) | 0.91 | 2.65 | 2.69 |
| … the write, off the main thread | 0.80 | 1.71 | 1.80 |
| Workspace new, drawn (100) | 42.23 | 43.70 | 55.63 |
| Workspace rename, drawn (100) | 14.42 | 14.86 | 15.54 |
| Workspace close, drawn (100) | 25.88 | 26.67 | 27.88 |
| Workspace duplicate (12 tabs), drawn (100) | 76.16 | 82.63 | 91.77 |
| Workspace switch at the store's size, drawn (100) | 57.12 | 71.41 | 92.24 |
| Tab drag into another column (`tab.move`), drawn (next frame, 16) | **16.13** | **18.84** | 26.82 |
| Column resize (`column.resize` at the drag's end), drawn | 35.18 | 35.79 | 40.87 |

A save never costs a frame. The export runs on the main thread, at most 2.7 ms p95 against a
16.7 ms frame, and the write runs on the file's queue. Before the outline cache and the
per-chapter ink sync, a duplicate was 112 / 114 ms and a switch 91 / 109 ms. **The tab drag is
at the edge of its frame**, 16.1 p50 and 18.8 p95: one re-render of two columns and the
moved tab's stream. I left it as measured.

### B072 / B076 / B077 / B078 rows (no store, the 10 × 3 × 4 session, 240 samples, `-OPENLOGOS_SHELL_MEASURE`)

| Interaction (budget) | B078 p50 / p95 | B074 p50 / p95 |
|---|---|---|
| Palette keystroke (16) | 4.95 / 14.99 | 5.18 / 15.08 |
| Tab switch within a column (16) | 11.83 / 14.67 | 10.95 / 11.71 |
| Column focus (16) | 7.53 / 9.97 | 7.40 / 7.67 |
| Workspace switch (100) | 38.54 / 40.92 | 39.02 / 41.42 |
| Tool change (16) | 7.31 / 8.94 | 7.28 / 8.92 |
| Column focus, original ↔ translation (16) | 9.27 / 9.46 | 9.46 / 9.63 |
| Translation → notes dock (16) | 7.53 / 8.16 | 7.57 / 8.16 |
| Selection appears → selection group drawn (16) | 9.24 / 9.59 | 9.16 / 9.71 |
| Open a chapter in a tab, warm (100) | 11.83 / 12.10 | 11.93 / 12.04 |
| Open a chapter: typed jump outside the window (100) | 23.57 / 38.42 | 23.00 / 38.28 |
| Scroll into the next verse (16) | 4.93 / 6.52 | 4.96 / 6.01 |
| Linked follow, host side (16) | 0.73 / 0.98 | 0.72 / 0.87 |
| Labels after a scroll (16) | 8.89 / 9.28 | 9.07 / 9.38 |
| Window edit, whole: add a block below (16) | 2.60 / 3.60 | 2.53 / 3.50 |
| Deferred work, one item per frame (16) | 3.26 / 4.88 | 3.24 / 4.74 |

No regression. Every row is within noise of B078's.

## Tests

`Tests/ShellTests/StoreTests.swift` (new, 12 cases). Every file goes in a temporary directory;
`testTestsAndScriptedRunsNeverUseTheRealStore` checks that XCTest and scripted runs get no store.

| Case | What |
|---|---|
| `testAQuitAndRelaunchBringsEverythingBack` | The owner's scenario through the UI path, `saveNow`, then a new model over the same file. Checks workspaces, tool, scroll anchor and reading line, the highlight painted on its words, ink on John 3:17 within 1 pt of where it was drawn, note heading, bold and link, an empty undo stack, and that saving again writes the same bytes |
| `testSavesWaitForTheDebounceAndCountQuietChanges` | A scroll is a change. Nothing is written before 1 s, then one save with the new anchor; the `.tmp` file is gone |
| `testTheEraserTakesAStoredStrokeOutOfTheCore` | The eraser causes `removeInk` |
| `testADamagedFileShowsTheNoticeKeepsABackupAndTheAppWorks` | Truncated JSON: the notice once, one backup with the same bytes, the file moved aside, the seed session, a command runs, and the next save writes a valid store |
| `testANewerVersionAndSetAsideParts` | `unsupported_version` words; a dangling active workspace and two bad highlights give "Your workspaces were reset; 2 highlights could not be restored; a copy was kept.", a backup, and a save due |
| `testBackupsKeepTheNewestFive` | Eight backups, five kept, newest first |
| `testAFailingWriteKeepsTheOldFileAndSaysSoOnTheThirdTime` | The store directory is a file; the failure notice comes on the third failure |
| `testANoteSurvivesTheStoredFormExactly` | Every block kind and mark, both ways; through the editor (restyled) unchanged; an edit is put into the core 300 ms later |
| `testATabDragAndAColumnResizeUndo` | Drag by frames to a slot (and the no-op drop), undo; drag into a new column, undo; a no-op `tab.move` says nothing; resize and undo |
| `testThePaletteSwitchesToAnotherWorkspacesTabAndToAWorkspace` | A workspace row and `@wlc` from another workspace |
| `testARestoredLexiconTabLooksUpInTheClickedEditionsLanguage` | `grc` kept across a restore |
| `testTestsAndScriptedRunsNeverUseTheRealStore` | `launchFile` under XCTest, a snapshot, and with `-OPENLOGOS_STORE` |

Changed: four B072/B077/B078 cases now read highlights from the core (`model.highlights(tabID)`)
instead of the host's `marks`. The placeholder case no longer expects `run` to return false:
the command runs, and the core refuses and says why.

## Screenshots (`docs/evidence/B074/`, 6 PNGs, 1440 × 900)

| File | What |
|---|---|
| `mac-store-before-quit.png` | The scenario before ⌘Q: Study with BSB John 3 (the "loved the world" highlight, the ink under "For God did not send"), SBLGNT pinned at Acts 2:14, the note, and the Highlight tool in the well; 1/3 workspaces |
| `mac-store-after-relaunch.png` | The same after relaunch |
| `mac-store-palette-workspaces.png` | "Rom" from Study: the Romans workspace row and its BSB · Rom 8:1 tab row |
| `mac-store-drag-slot.png` | WLC Gen 1 dragged over column 2's strip: the slot bar, the label at the pointer, the New column band |
| `mac-store-drag-undo.png` | After the drop: WLC in column 2; the palette's Undo row reads "Undo · Move tab to position ⌘Z" |
| `mac-store-damaged-notice.png` | Relaunch over a truncated file: "Your saved workspace could not be read; a copy was kept." over the seed session |

## Lanes

Run by the lead on 38aecd7 (6d01b63 with main merged in after #32–#36), `CARGO_INCREMENTAL=0`, in order:

| Lane | Exit | Summary |
|---|---|---|
| `nix develop .#core -c just ci` | 0 | `Summary [7.404s] 337 tests run: 337 passed, 0 skipped` |
| `nix develop .#apple -c just test-swift` | 0 | `Executed 11 tests, with 0 failures (0 unexpected)`; `conformance cases: 1844 passed, 0 failed` |
| `nix develop .#apple -c just test-reader mac` | 0 | `Executed 81 tests, with 0 failures (0 unexpected)` (12 new StoreTests) |

The agent's own run on 6d01b63 gave the same results. An earlier run at fe35ce5 failed `ci` on
shellcheck SC2115 in scripts/b074-mac-shots.sh; 6d01b63 guards the `rm -rf`.

## Decisions (for the owner's review)

1. **The restore budget, as measured on the Mac.** File read → the active workspace's text
   laid out is 135 ms p50, and → the first frame on screen is 266 ms. With no file the same
   path takes 96 and 185 ms. That is SwiftUI building the window, which the store does not
   change. The restore's own share is about 39 ms: `importStore` takes 34 ms, more than the
   core bench's warm 17 ms, because a cold first call into the library is slower. Meeting ≤
   100 ms end to end would mean importing off the main thread while the window is built, or
   cutting the Mac launch itself. I did neither tonight; tell me if you want it.
2. **Ink is drawn a frame after the restored text.** Strokes cost the first frame about 110
   ms at the acceptance size, so the text and highlights come first and the strokes follow.
   specs/24 §5's order ("paint highlightsIn and inkIn … then the tool well") is kept within the
   frame budget rather than in one frame.
3. **The stored ink `x` is measured from the text's leading edge, growing into the text** (the
   text container's edge inside the line-fragment padding; the right edge for WLC). This is
   the lead's lockstep ruling, and the web does the same.
4. **A save that would write the same bytes writes nothing.** Every handled command moves the
   change count, quiet no-ops included. Without this, every launch would rewrite the file
   once a second after it settled.
5. **Notice wording (the lead's lockstep ruling, the same on the web):** "…written by a newer
   version of OpenLogos; a copy was kept." for a newer major. For issues, the parts are joined
   with "; " and end with "; a copy was kept." After three failed writes in a row: "Your
   changes could not be saved; the last saved copy is kept." The notice shows once and stays
   6 s (a toast stays 2.4 s). A refused file is moved aside (backed up, then removed).
6. **The real store is used only by a plain launch** (`just preview-mac`, opening the app).
   XCTest, every snapshot or measurement, and the store's own scripts keep no store unless
   `-OPENLOGOS_STORE <dir>` names one.

## Deviations and skipped

- **Restore over 100 ms end to end** (decision 1), and **the tab drag at 16.1 / 18.8 ms**
  against the next frame.
- **No "Remove highlight" yet.** The Mac host offers no command to remove a highlight, and the
  eraser takes ink strokes only. `removeHighlight` exists in the core and nothing calls it.
- **Undo of notes, highlights and ink** stays out of scope (specs/24 §9). ⌘Z in a note is the
  text view's own undo, as before.
- **Strokes drawn before B074** (B072's demo strokes) are not stored, because they have no
  stream to pin them to. Strokes on tabs that are not reading streams stay host-only, as
  before.
- A restored column opens with its verse on the reading line. It does not restore the exact
  pixel offset within that verse's first line: the stored scalar offset is not applied, so a
  column can sit up to one line away from where it was left.
- iPad: the shell there is still B075's work. iOS compiles the new code, but nothing on iPad
  saves.
