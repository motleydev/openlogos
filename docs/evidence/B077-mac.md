# B077 evidence (Mac) — the OpenLogos context menu in the Mac shell

Recorded 3 November 2026 on the Mac mini (Apple M1, macOS 26.4, Xcode 26.6) in the worktree
`openlogos-b077mac`, branch `b077-mac-menu` (B076 Mac and `b077-core-menu` merged in at
1afc175). Work was done by a Claude Code subagent. Nothing was pushed or merged; no
simulator, keychain or macOS user was touched. Only fixture text is on screen. The core part
is B077-core.md; the web half is `b077-web-menu` (B077-web.md).

## What works

| Item (B077 host brief) | Where |
|---|---|
| The platform menu is replaced in reading text, notes, tab labels, the tab strip's empty area, identity rows (badge included) and the column's empty area (margins beside the measure, below the text, lexicon and placeholder tabs). Elsewhere (palette field, toolbar menus) the platform menu stays. | `Sources/Reader/ReaderTextView.swift` (`ReaderNSTextView.menu(for:)`), `Sources/Shell/NoteEditor.swift` (`NoteTextView.menu(for:)`), `Sources/Shell/ColumnsView.swift`, `Sources/Shell/ContextMenu.swift` (`MenuCatcherView`) |
| ⌥-right-click keeps the platform menu: in reading text and notes AppKit's own (Look Up, Services, spelling in notes), with none of the OpenLogos rows; SwiftUI chrome has no platform menu, so ⌥-right-click there shows nothing. | `menu(for:)` overrides; `MenuCatcherView.menu(for:)` |
| Keyboard: ⌃-click (AppKit sends it to `menu(for:)`), VoiceOver's VO-⇧-M (`accessibilityPerformShowMenu` on the reading and note text views; `.accessibilityAction(.showMenu)` on tab labels and identity rows) and a PC keyboard's menu key (key code 0x6E in the shell's key monitor). The menu key opens on the selection, else the caret's word (selected first) in reading text, else the caret or selection in a note, else the active tab's label, else the active column's background, at that spot. | `ShellModel.openKeyboardMenu`, `ReaderSurface.keyboardHit`, `NoteTextView.showKeyboardMenu` |
| Targets: a right-click on a word selects it through the anchor round trip, as macOS does, and passes `word` with the text, reference, segment id, scalar range (from the anchor), resource id and tab id; inside an existing selection it passes `selection` (start and end references); in a note a passage link passes `note_link`, else `note_text` (the selected text, `tab_id` only for a Notes tab); tabs pass `tab`, identity rows `column`, empty areas `background` with the column. A gone target is refused by the core and toasted. | `ReaderSurface.contextHit(at:)`, `ShellModel.target(for:tab:column:)`, `NoteTextView.hit(at:)` |
| Drawing: a native NSMenu built from the core menu: its `title` as a section header, sections between separators (section labels not drawn), core labels in core order, one-level submenus (Mac submenus just open; the parent's command is not run), `key` as the item's key equivalent (`Cmd+Shift+C` → ⇧⌘C, `Alt+Shift+1` → ⌥⇧1), `active` as a check mark, `value` as a swatch dot, `disabled_text` as the disabled row's subtitle and tooltip, an SF Symbol per core icon. | `ContextMenuBuilder` |
| Dispatch: on a choice the menu's `focus` is applied first (not on open; dismissing the menu changes nothing), then the row under the menu's `context` (`run(_:args:context:)`). | `ShellModel.choose` |
| `search.text` edition / all / workspace: the palette opens with `#word` (its Text section) and a scope chip left of the text: `This edition · BSB` (× drops the scope), `All editions`, `This workspace`. Edition scope keeps the rows of that edition; workspace scope those of the editions open in the workspace. Results are the existing fixture TextSource (B071 replaces it). Closing the palette drops the scope. | `ShellModel.openSearch`, `scoped(_:)`, `ScopeChip` in `ShellView.swift` |
| Find in this chapter (`search.text` scope `chapter`): no palette; every match in that tab's text is painted (accent at 20 %, the current one at 45 %), the first revealed, toast `4 matches in John 3` (`No matches for “x” in John 3`); ⌘G / ⇧⌘G step and wrap (`2 of 4 in John 3`); Esc clears. | `findInChapter`, `findNext`, `readerMarks`, `handleMenuKey` |
| Copies: `copy.text` (`Copied “loved”`), `copy.reference` (`formatHuman`, `Copied John 3:16`), `copy.passage` (`<text> — John 3:16 (BSB)`: the selection's text, or the verse's text read from the edition; a range as `John 3:16–17`; toast `Copied John 3:16 with its text`). | `performMenuHost` |
| `host.cut / copy / paste`: in a note the editor's own cut, copy and paste (undoable); `host.copy` on a reading selection copies its text (`Copied`). `nav.paste_passage`: the clipboard parsed with the core's `parseReference` and opened in the active tab with `nav.navigate`; nothing parsable toasts `No passage on the clipboard`. `selection.highlight_in`: the selection highlighted in the chosen swatch. `note.remove_link`: the link attribute removed from the clicked link's run (undoable), toast `Removed link to …`. `tab.open_edition`, `tab.close_others`: the core's; the host re-renders. | `performMenuHost`, `clipboardEdit`, `pastePassage`, `removeNoteLink` |
| Editions: languages are registered with the core only for the three editions (`bsb en`, `sblgnt grc`, `wlc hbo`). Before, the note and the lexicon resource had `en` too, so Compare on SBLGNT was `Compare in ▸ BSB / Note / Lexicon`; now it is the plain `Compare in BSB`. The well is unchanged (note and lexicon tabs have their own kinds). | `FixtureTextSource.resourceLanguages` |
| Launch hooks: `-OPENLOGOS_MENU kind:…` opens a menu on a target where a right-click would; `-OPENLOGOS_MENU_SUBMENU` walks it with ↓ and → to a submenu; `-OPENLOGOS_MENU_CHOOSE` chooses a row; with `-OPENLOGOS_SNAPSHOT` the picture is taken while the menu is open, its windows composited over the app's. | `ShellLaunch.openMenu`, `snapshot(to:)`, `scripts/b077-mac-shots.sh` |

### Icons (core name → SF Symbol)

`search-all` magnifyingglass.circle · `lemma` textformat.abc · `find` text.magnifyingglass ·
`compare` rectangle.split.2x1 · `open` arrow.up.forward.square · `column` rectangle.portrait ·
`new-column` rectangle.portrait.badge.plus · `cross-references` arrow.triangle.branch · `copy`
doc.on.doc · `copy-passage` doc.on.clipboard · `cut` scissors · `paste` clipboard ·
`remove-link` personalhotspot.slash · `close` xmark · `close-others` xmark.square ·
`duplicate` plus.square.on.square · `move` arrow.right.square · `pin` pin · `follow`
arrow.triangle.2.circlepath · `link` link · `focus` scope · `move-left` arrow.left ·
`move-right` arrow.right · `new-tab` plus.square · `equalize` rectangle.split.3x1 ·
`close-column` rectangle.portrait.slash · `paste-passage` list.clipboard · `text-size`
textformat.size · `text-larger` textformat.size.larger · `text-smaller`
textformat.size.smaller · `text-reset` textformat · `theme` circle.lefthalf.filled; the
well's icons keep B076's symbols; `swatch` rows draw the swatch's colour as a dot.

## Core

Nothing under `crates/` changed: `git diff b077-core-menu -- crates` is empty.

## Speed (specs/22 §7, Release, 10 × 3 × 4 session, 240 samples per row, 1440 × 900)

Tree f06174e. The B072 rows use B072's harness (`work` = the command through a synchronous
layout and display of the window). The menu rows: **built** = from the right-click to the
NSMenu AppKit shows (the hit test, which selects the word through the round trip, the core's
`contextMenu`, the NSMenu with its symbols and swatches; the word's selection is drawn while
the menu is up, so it is not in this number); **window open** = from the right-click until
`popUp` has the menu window up and its tracking loop runs (a timer in the event-tracking run
loop mode stops the clock and closes the menu); the two **AppKit floor** rows time `popUp` of
a plain menu with 3 and with 18 rows (the word menu's size) the same way; **choose → effect**
= the menu's focus, the row and the window drawn.

| Interaction (budget ms) | samples | p50 / p95 / max ms | next frame p95 |
|---|---:|---:|---:|
| **Right-click → menu built, word** (hit test selects it, core, NSMenu) (16) | 240 | 2.10 / 2.15 / 2.32 | — |
| **Right-click → menu built, selection** (16) | 240 | 2.06 / 2.15 / 3.89 | — |
| **Right-click → menu built, tab · column · background · note text · note link** (16) | 240 | 0.43 / 0.86 / 0.99 | — |
| **Right-click → menu window open and tracking** (word menu, end to end) (16) | 240 | 53.18 / 64.82 / 70.80 | — |
| AppKit floor: popUp of a plain 3-row NSMenu (16) | 240 | 31.49 / 40.16 / 43.79 | — |
| AppKit floor: popUp of a plain 18-row NSMenu (16) | 240 | 50.27 / 56.84 / 62.70 | — |
| **Choose → effect: Compare in BSB** (`tab.open_edition`, drawn) (16) | 240 | 7.38 / 7.52 / 8.37 | 9.19 |
| **Choose → effect: Search in all editions** (palette Text section drawn) (50) | 240 | 6.20 / 6.38 / 7.13 | 12.51 |
| Palette keystroke (16) | 240 | 6.09 / 14.40 / 23.22 | 16.71 |
| Tab switch within a column (16) | 240 | 11.96 / 13.61 / 15.37 | 16.68 |
| Column focus, columns 1–3 (16) | 240 | 7.64 / 8.70 / 11.54 | 16.68 |
| Workspace switch (100) | 240 | 40.30 / 41.78 / 54.12 | 54.62 |
| Tool change (16) | 240 | 7.75 / 9.17 / 10.59 | 16.68 |
| Column focus, original ↔ translation (16) | 240 | 9.90 / 11.02 / 12.34 | 16.68 |
| Translation → notes dock (16) | 240 | 7.28 / 8.57 / 9.63 | 23.80 |
| Selection appears → selection group drawn (16) | 240 | 7.64 / 8.98 / 13.52 | 25.16 |
| Open a chapter in a tab, warm (100) | 240 | 11.39 / 11.68 / 13.65 | 16.68 |

What the host does on a right-click is within the frame (≤ 2.2 ms p95; the core's part is the
B077-core bench's 84 µs p95). The menu window itself is not: AppKit's `popUp` of an 18-row
menu is 50 / 57 ms on this machine with nothing of ours in it, and ours is 53 / 65 ms (its
symbols, subtitles and a header). Over the day's runs the gap between ours and the plain
18-row menu was 1–19 ms p50. An experiment in an earlier run (not kept) showed the symbols
account for about 7 ms p50 of it and the subtitles for none. Every B072 / B076 row is within
budget and at B076's level (B076 p95 at 1440: palette 12.0, tab switch 14.1, column focus 9.0,
workspace 39.5, tool 9.8, original ↔ translation 11.0, dock 7.9, selection 8.9, chapter 11.7);
the palette p95 varies between 11.7 and 14.9 over today's runs (B076 9.4–12.0), the B072
budget holds. The next-frame p95 of the two rows with a `prepare` step is above 16.7 for the
reason B076-mac.md gives. Raw JSON: `artifacts/b077/measure-1440.json` (gitignored).

At 1024 × 768 (same tree, 240 samples): menu built word 2.12 / 2.16, selection 2.11 / 2.15,
other targets 0.43 / 0.86; window open 54.48 / 63.44 (floors 3 rows 32.95 / 43.34, 18 rows
53.68 / 59.92); choose → Compare 7.30 / 7.48, → Search 6.24 / 6.53; B072 rows within budget
(palette 15.04, tab switch 13.56, column focus 7.93, workspace 41.46, tool 9.15, original ↔
translation 9.96, dock 7.39, selection 9.37, chapter 12.81; p50 / p95 ms). Raw JSON:
`artifacts/b077/measure-1024.json`.

## Tests (`just test-reader mac`)

58 cases in OpenLogosMacTests: B072/B076's 44 unchanged and 14 new in
`Tests/ShellTests/ContextMenuTests.swift`:
`testAWordMenuSelectsTheWordAndIsDrawnFromTheCore` (Greek: the word selected, the target's
fields, focus first, header, separators per section, rows in core order and labels, Word
study enabled, plain Compare in BSB, disabled Search lemma with subtitle and tooltip, ⇧⌘C on
Collect, Highlight ▸ four swatches with Yellow checked, an icon on every row; BSB: Word study
disabled with its words, Compare in SBLGNT) · `testIconsMapToSymbols` ·
`testInsideASelectionAMarginAndThePlatformMenu` (selection menu, background below the text,
⌥-right-click gives AppKit's menu without our rows, a plain right-click ours) ·
`testChromeAnswersRightClicksAndLetsClicksThrough` (tab labels, identity row, strip, column
background: a right-click hit-tests to the catcher, a click goes through; the hidden tab's
menu has focus first, ⌥W on Close tab, Move to ▸ its own column disabled with words and
⌥⇧1; Column 2's header; no OpenLogos menu on ⌥-right-click) ·
`testFocusIsAppliedOnChooseNotOnOpen` · `testCompareOpensTheOtherEditionInTheNextColumnAndUndoes`
(BSB → SBLGNT reused in column 2 at the word's verse, focus stays, undo; SBLGNT → BSB in
column 3) · `testSearchOpensThePaletteWithItsScope` (edition chip, only BSB rows, × and
close drop it; all editions) · `testFindInThisChapterPaintsMatchesAndSteps` (4 matches of
"world", marks, ⌘G, ⇧⌘G wrap, Esc) · `testCopiesPutPassagesOnThePasteboard` (reference,
verse with reference, a two-verse selection with its range, host.copy) ·
`testHighlightInASwatch` · `testPastePassageOpensTheClipboardsPassage` (and the refusal) ·
`testTabMenuClosesOtherTabsAndUndoes` (focus first on the hidden tab, then close others,
undo) · `testNoteMenusCutCopyPasteAndRemoveALink` (link target, Copy reference, Remove link
and its toast, Cut / Copy / Paste and Bold on note text) ·
`testTheMenuKeyOpensTheMenuForWhatHasFocus` (the caret's word, then the selection, then the
focused tab, then a note).

## Screenshots (`docs/evidence/B077/`, 8 PNGs, 1440 × 900)

`mac-menu-word-greek` (ἠγάπησεν in SBLGNT: Word study enabled, Compare in BSB) ·
`mac-menu-word-bsb-highlight` (loved in BSB: Search lemma, Word study and Cross references
disabled with their words; Highlight ▸ open with the four swatches, Yellow checked) ·
`mac-menu-selection` ("loved the world": Study, ⌘C Copy, Copy with reference) ·
`mac-menu-tab-move` (the hidden WLC tab: Pin disabled "The tab is already pinned", Move to ▸
open with column 1 disabled "The tab is already in column 1") · `mac-menu-note-link` (the
John 3:16 link in the note: Open in ▸, Compare in ▸, Copy reference, Remove link) ·
`mac-menu-find-in-chapter` (four matches of "world" painted, the toast) ·
`mac-menu-search-scope` (the palette with the `This edition · SBLGNT` chip and the SBLGNT
row) · `mac-menu-dark` (dark theme, κόσμον, Copy ▸ open). `scripts/b077-mac-shots.sh`
retakes them.

The menus in the pictures are AppKit's own menu windows, drawn by the app into the PNG while
open (no screen capture). One part is substituted: the menu's Liquid Glass backdrop
(backdrop and SDF layers) draws as plain white through `cacheDisplay`, which made the dark
menu white on white, so the picture hides those layers and fills the menu's shape with the
appearance's window background. On screen the menu has the system's glass.

## Lanes (committed tree f06174e)

| Command | Result |
|---|---|
| `nix develop .#core -c just ci` | exit 0. `Summary [   6.681s] 312 tests run: 312 passed, 0 skipped`; clippy, shellcheck and the wasm check clean (an earlier run failed shellcheck SC1111 on curly quotes in the shots script; fixed in f06174e) |
| `nix develop .#apple -c just test-reader mac` | exit 0. `Executed 58 tests, with 0 failures (0 unexpected)` |
| `nix develop .#apple -c just test-swift` | exit 0. `Executed 9 tests, with 0 failures (0 unexpected)`; `conformance cases: 1594 passed, 0 failed` |
| `nix develop .#core -c python3 tools/validate.py` | exit 0. PASS: 33 schema/example validations; 77 acyclic tasks; 20 conformance fixtures; 125 negative tests |
| iOS target, `xcodebuild -scheme OpenLogos -destination "generic/platform=iOS Simulator" build` (no simulator booted) | the Swift compiles for iOS; the link fails for x86_64 only, as the XCFramework carries the arm64 simulator slice alone (not a B077 change) |

## Deviations

1. **Menu window on screen is above 16 ms, and that is AppKit's.** Building the menu (hit
   test, word selection, core, NSMenu) is ≤ 2.7 ms p95 for every target; showing an NSMenu
   is AppKit's presentation, and a plain NSMenu of the same 18 rows costs about as much as
   ours (table above). Meeting 16 ms would mean drawing our own menu in a panel instead of a
   native NSMenu, against the brief's "Mac = native NSMenu". Owner decision below.
2. **No section labels drawn.** The core's labels (`Search`, `Open`, …) are optional; macOS
   menus separate groups with separators, and the header row is the word. Same as agreed for
   web (pending b077web's confirmation; see below).
3. **Mac submenu parents only open** their submenu (core decision 5 allows it).
4. **⌥-right-click on SwiftUI chrome shows nothing**, as the tab strip and identity row have
   no platform menu to fall back to.
5. **A second right-click on the word just selected is a selection menu** (the click is
   inside the selection), as macOS does.

## For the owner

- The NSMenu takes about 50 ms to come up on this machine (AppKit, with the system's glass);
  keep the native menu (looks and behaves like every Mac menu, VoiceOver included), or draw a
  custom menu that could open on the next frame?
- Should Find in this chapter also list its matches in the palette, or stay in the column as
  now?

## Skipped

iPad (B075): the context menu is Mac-only; iPad keeps the platform edit menu (the iOS target
compiles; `ReaderTextView.onContextMenu` is nil there). Lemma search and cross references
stay disabled (data not imported). Text search over the real editions waits for B071; the
scope filters the fixture TextSource's rows.
