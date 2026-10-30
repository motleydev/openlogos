# B076 evidence (Mac) — the contextual tool well in the Mac shell

Recorded 30 October 2026 on the Mac mini (Apple M1, Xcode 26.6) in the worktree
`openlogos-b076mac`, branch `b076-mac-well`, cut from `b076-core-well` (a8f7433) with
`b076-core-well` merged in at 3702fc5 (`tab.open_lexicon`, `ShellTab.lexiconWord`). Work was
done by a Claude Code subagent. Nothing was pushed or merged; no simulator, keychain or macOS
user was touched. Only fixture text is on screen. The web half is `b076-web-well`
(B076-web.md); the core part is B076-core.md.

## What works

| Item (B076 host brief) | Where |
|---|---|
| The well is drawn from `toolWell(context:)` groups in core order, rebuilt on every focus, tab, selection and session change. Every command carries `ShellCommandContext(hasSelection:, focus:)` (`applyIn`, `commandsIn`, palette `focus`). `setResourceLanguages` is called beside `setResourceTitles` (`bsb en`, `sblgnt grc`, `wlc hbo`). | `Sources/Shell/ShellModel.swift` (`focus`, `well`, `commandContext`, `refreshWell`), `TextSource.swift` |
| Groups: tools; a divider and the ink group (two overlapping swatches, X or click swaps, a context menu picks one of the four token swatches; the width dot) on translation and original; a divider and the selection group while text is selected. The well's top is placed by B072's well height, so a context with more or fewer tools and the selection group grow or shrink it downward only. Buttons are keyed by command id, so the tools that remain do not relayout; only the selection group fades (0.1 s). | `Sources/Shell/ToolWell.swift` |
| Active = filled accent button, accessibility value "current tool". Tooltip `Label (⌘B)`, `Label` without a key, `Label (P) — reason` when disabled; disabled items at 40 % and not clickable. | `ToolWell.ToolButton`, `ToolWell.tooltip` |
| Effective tool: drag mode, ink and the active button follow `well.effectiveTool`; the stored tool stays stored and comes back. A bare key outside its context shows the core's words as a toast (`Word study works on Greek and Hebrew text`). ⌘B / ⌘I pass on unless a note editor has focus. | `ShellModel.effectiveTool`, `handle(chord:)` |
| Focus: a click in a column or its text is `tab`; the notes dock's editor taking focus is `notes_dock`; closing the dock gives focus back to the tab. | `ShellModel.setFocus`, `NoteEditorView.onFocus` |
| Word study: in an original column with W, a click hands the word under it (through the anchor round trip) to the host, which runs the core's `tab.open_lexicon` {text: the word, reference: its verse}. The core opens or reuses the pinned Lexicon tab in the next column (or a new column) and keeps focus on the clicked column, so Word study goes on. The tab shows the core's `lexiconWord` (reading serif, 1.5×, semibold), `Greek · John 3:16` (language from the word's script) and "Lexicon entries arrive with the lexicon import." The strip label is the core's: `Lexicon Jn 3`. | `ReaderTextView.swift` (`ReaderDragMode.word`), `ShellModel.openWordStudy`, `Sources/Shell/LexiconView.swift` |
| Notes: notes tabs and the dock are `NSTextView` editors (TextKit 2), in memory until B074, one document per note shared by its editors. Bold, Italic (⌘B/⌘I while editing), Heading (1.25× the reading size, semibold), List (`• ` bullets toggle, hanging indent), Link passage (well button opens the palette with `link passage `; the palette's row runs `note.link_passage` with the passage; selected words become the link, else `John 3:16` is inserted). Links are accent + underline; a click moves the last-focused reading column (column 1 when none has had focus). Edits are undoable in the editor. | `Sources/Shell/NoteEditor.swift` |
| Selection actions: Highlight now, Collect, Study as in B072; Note on it appends `“quoted text” — John 3:16` as a quote paragraph (indented 12, a 2 pt rule in the rules colour on the leading edge, not italic, the passage a link) to the open note (dock if open, else a shown notes tab, else it opens the dock); Search this (⌘E) puts the text in the palette and opens it; Copy reference puts `John 3:16` (`John 3:16–17` for a range) on the pasteboard and toasts `Copied John 3:16`. | `ShellModel` (MARK: Selection actions and notes) |
| Launch hooks for the pictures: `-OPENLOGOS_SELECT`, `-OPENLOGOS_WORD`, `-OPENLOGOS_NOTE_DEMO`, `-OPENLOGOS_FOCUS dock`; speed rows in `ShellMeasure`. | `ShellLaunch.swift`, `ShellMeasure.swift`, `scripts/b076-mac-shots.sh` |
| Fix found on the way: tab ids repeat across workspaces (`t1`), and a released workspace's reading surface could re-register under the shown tab's id after a workspace switch, so the host read the old text. `register` now ignores a surface whose content is not the tab's resource. | `ShellModel.register` |

### Icons (core name → SF Symbol → meaning the web SVG draws)

| Core icon | SF Symbol | Meaning |
|---|---|---|
| `select` | `cursorarrow` | arrow pointer |
| `word-study` | `character.book.closed` | closed book with a letter |
| `highlight` | `highlighter` | highlighter marker |
| `pen` | `pencil.tip` | pencil tip |
| `highlighter-pen` | `paintbrush.pointed` | pointed brush |
| `eraser` | `eraser` | eraser |
| `collect` | `tray.and.arrow.down` | tray with a down arrow |
| `type` | `character.cursor.ibeam` | letter with an I-beam |
| `bold` | `bold` | B |
| `italic` | `italic` | slanted I |
| `heading` | `textformat.size` | small and large A |
| `list` | `list.bullet` | bulleted list |
| `link-passage` | `link` | chain link |
| `swatch`, `swatch-alternate` | drawn | two overlapping rounded squares in the swatch colours |
| `brush-width` | drawn | filled dot sized by the width |
| `highlight-now` | `highlighter` | highlighter marker (in the selection group) |
| `note` | `note.text.badge.plus` | note with a plus |
| `search` | `magnifyingglass` | magnifier |
| `copy-reference` | `doc.on.doc` | two stacked pages |
| `study` | `text.book.closed` | closed book with lines |

## Core

This branch changes nothing under `crates/` beyond the merge of `b076-core-well` (3702fc5):
`git diff b076-core-well -- crates` is empty. An earlier host-made `tab.open_lexicon` commit
was dropped from the branch history on the lead's ruling in favour of the core's.

## Speed (specs/22 §7, Release, 10 × 3 × 4 session, 240 samples per row)

`work` = the command through a synchronous layout and display of the window (B072's harness).

| Interaction (budget) | 1440 × 900 work p50 / p95 / max ms | next frame p95 | 1024 × 768 work p50 / p95 / max ms | next frame p95 |
|---|---:|---:|---:|---:|
| **Column focus, original ↔ translation (the well changes)** (16) | 10.5 / 11.0 / 12.7 | 16.7 | 10.2 / 10.7 / 10.9 | 16.7 |
| **Translation → notes dock (the well changes)** (16) | 7.0 / 7.9 / 8.1 | 22.4 | 7.0 / 7.4 / 9.0 | 22.5 |
| **Selection appears → selection group drawn** (16) | 8.2 / 8.9 / 9.8 | 24.5 | 8.7 / 9.4 / 12.7 | 23.7 |
| Palette keystroke (16) | 4.1 / 12.0 / 20.2 | 16.7 | 4.2 / 12.1 / 19.8 | 16.7 |
| Tab switch within a column (16) | 11.6 / 14.1 / 14.3 | 16.7 | 11.6 / 14.0 / 17.0 | 16.7 |
| Column focus, columns 1–3 (16) | 7.8 / 9.0 / 10.4 | 16.7 | 7.8 / 7.9 / 9.2 | 16.7 |
| Workspace switch (100) | 36.5 / 39.5 / 40.6 | 56.5 | 37.0 / 40.1 / 43.2 | 56.4 |
| Tool change (16) | 8.2 / 9.8 / 10.1 | 16.7 | 8.2 / 9.8 / 10.0 | 16.7 |
| Open a chapter, warm (100) | 11.5 / 11.7 / 14.3 | 16.7 | 11.5 / 11.7 / 13.2 | 16.7 |

Every p95 is within budget, and the B072 rows are at B072's level (B072 p95 at 1440: palette
8.6, tab switch 14.9, column focus 9.6, workspace 48.7, tool 10.0, chapter 12.6). Palette p95 at
1024 is 12.1 (B072 12.4); at 1440 it varies between 9.4 and 12.0 over today's runs (B072 8.6).
Single runs vary by 1–3 ms p95 on this machine (an earlier run today had tab switch at 1024 at
15.6). The next-frame p95 of the two rows with a `prepare` step is
above 16.7 because the reset (back to the tab / clearing the selection) is drawn in the same
display-link frame just before the timed action; the work column is the interaction itself.
Raw JSON: `artifacts/b076/measure-1440.json`, `measure-1024.json` (gitignored).

## Tests (`just test-reader mac`)

44 cases in OpenLogosMacTests: 8 reader, 10 ink, 16 shell (B072, two updated: the token-file
test compares the token file's tools with the core's translation well; the TextKit 2 test
checks reading columns only, the notes column is an editor now) and 10 new in
`Tests/ShellTests/ToolWellTests.swift`: `testTheWellFollowsTheFocusedTab` (translation,
original Greek and Hebrew, notes tab, notes dock, dock closing) ·
`testIconsMapToSymbolsAndTooltipsCarryKeys` (every symbol exists) ·
`testAStoredToolSurvivesAContextThatDoesNotOfferIt` (Pen → Type in notes → Pen; Word study →
Select → Word study) · `testABareKeyOutsideItsContextSaysWhyInAToast` ·
`testTheNewKeysComeFromTheRegistry` (W, T, ⌘B, ⌘I, ⌘E) ·
`testASelectionAddsItsGroupBelowWithoutMovingTheTools` ·
`testWordStudyOpensTheWordInALexiconTabInTheNextColumn` (open, reuse, new column, undo) ·
`testCopyReferenceAndSearchThis` · `testNoteOnItAppendsTheQuoteToTheOpenNote` (notes tab; dock
opened in a workspace without notes) · `testNotesTakeBoldItalicHeadingListAndPassageLinks`
(⌘B in the editor, toggles, heading, list, link insert, link click navigates column 1, the
palette's Link passage row).

## Screenshots (`docs/evidence/B076/`, 6 PNGs, 1440 × 900)

`mac-translation-well` (BSB focused: six tools and ink) · `mac-original-word-study` (SBLGNT with
Word study active, ἠγάπησεν in the Lexicon tab of column 3) · `mac-notes-editing` (the notes
column being edited: a heading, **so loved** in bold, a John 3:16 link and a quote paragraph
with its leading rule; the notes well) ·
`mac-selection-group` (a selection in BSB, the selection group under the ink) ·
`mac-fallback-pen-notes` (Pen stored, the dock focused: Type active) ·
`mac-original-selection-dark` (dark theme, Greek selection, original well with the selection
group). `scripts/b076-mac-shots.sh` retakes them.

## Lanes (committed tree)

| Command | Result |
|---|---|
| `nix develop .#core -c just ci` | exit 0. `Summary [   6.900s] 306 tests run: 306 passed, 0 skipped` |
| `nix develop .#apple -c just test-reader mac` | exit 0. `Executed 44 tests, with 0 failures (0 unexpected)` |
| `nix develop .#apple -c just test-swift` | exit 0. `Executed 8 tests, with 0 failures (0 unexpected)`; `conformance cases: 1355 passed, 0 failed` |
| `nix develop .#core -c python3 tools/validate.py` | PASS: 33 schema/example validations; 76 acyclic tasks; 19 conformance fixtures; 125 negative tests |

## Deviations

1. **The note editor is rich text, not markdown**: bold, italic, headings, quotes and links are
   attributes; nothing is persisted (B074 decides the stored form). Same as web.
2. **The swatch item's value** is chosen from a context menu on the swatch pair; a click
   still swaps (B072 behaviour). The width dot still cycles widths on click.
3. **Selection group in notes**: a reading selection stays the selection when focus moves to
   the note, so its group shows there and Note on it can be used from the note.

## For the owner

- Should Word study's Lexicon tab sit in the next column even when that column is the notes
  column (it does today: it becomes a tab beside the note)?
- Should ⌘B / ⌘I do anything outside a note (today they pass on to the system)?

## Skipped

iPad (B075): the note editor is AppKit-only; iOS keeps B072's read-only note through a stub.
No persistence of notes (B074). Lexicon content (the lexicon import).
