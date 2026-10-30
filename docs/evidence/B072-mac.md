# B072 evidence (Mac): the workspace shell in the Mac app

Recorded 30 October 2026 on the Mac mini (Apple M1, macOS 26.4.1, Xcode 26.6) in the worktree
`openlogos-b072mac`, branch `b072-mac-shell` cut from `b070-core-shell` (70ce91e), with
`b070-core-shell` merged in at dec4426 (reading, ink, width and scroll commands), a7d41e5
(reason texts, resource titles, `formatShort`, token-aligned sizes and swatches, argument
expansion rows) and 45fe931 (`session()` view fix, chapter labels, concrete keys on expanded
rows). The work was done by a Claude Code subagent under the lead's audit. Nothing was pushed
or merged by the agent, and no simulator was booted, erased or modified. This branch does not
change `crates/`; the merged core commits do. Only fixture text appears on screen. The web half
of B072 is `b072-web-shell` and has its own evidence (B072-web.md).

## What was built

| Item (specs/backlog.json B072, the B072 brief) | Where |
|---|---|
| The shell is the default view of the OpenLogosMac target. The B004 prototype is the Debug menu's "B004 prototype" toggle, and opens directly when `-OPENLOGOS_VIEW` names a prototype view (so `scripts/b005-shots.sh` and `b039-shots.sh` still work). iOS keeps the prototype as its root until B075. | `apps/apple/OpenLogos/Sources/OpenLogosApp.swift` |
| Palette field as the only top bar. It is the window's unified toolbar row beside the traffic lights, at most `palette_dropdown_max_width` (770) wide, with the workspace switcher (`Study · 1/2`) and the text menu (`Aa 19`) at the trailing end. Focus by click, ⌘K or `/` outside a text field. `paletteQuery` runs on every keystroke with no debounce. Results hang under the field at its width (radius 10, at most half the window tall), in the core's sections with headings, dim subtitles, right-aligned key glyphs, disabled rows with the core's reason, and "Show all N ⌥↩". ↑/↓ moves, Enter runs, ⌥Enter or Tab expands a section, Esc closes and returns focus to the tab. | `Sources/Shell/ShellView.swift` (`PaletteField`, `PaletteResults`), `ShellModel.swift` (MARK: Palette), `PaletteState` |
| Numbered columns with 1 pt rules. A drag previews on the host and ends in one `column.resize`. Each column has a tab strip (28 pt), with the shown tab marked by weight and a 2 pt accent underline, `+`, × on hover and horizontal overflow; labels are `resource_title` + `chapter_label` (`BSB Jn 3`). Each column also has an identity row (32 pt) with a number badge (20 pt, filled in the accent when active, outlined otherwise), the resource name, the passage in full (`formatHuman`, `John 3:16`), the link word and a `…` menu of tab and column commands. | `Sources/Shell/ColumnsView.swift` |
| Text: B005's TextKit 2 surface, given a host typography (token size step, line spacing, theme colours) and capped at the core's measure (`layoutConstants().measureCapStandardPt`). Scroll anchors are recorded with `tab.record_scroll` and restored from `scroll_segment_id`. A passage with no fixture text shows specs/21's placeholder. | `Sources/Reader/ReaderTextView.swift` and `ReaderLayout.swift` (additive: `ReaderTypography.fixed`, `ReaderMark`, `ReaderDragMode`, `topSegmentID`, `scroll(toSegment:)`, `focus()`, `onActivate`), `ColumnsView.swift` `TabBody`, `PlaceholderView` |
| Floating tool well (40 pt wide, 12 pt from the leading edge, radius 10, shadow). It holds the six tools as SF Symbols with name and key in the tooltip (the current one is filled and carries an accessibility value), two overlapping swatches (X or a click runs `tool.swap_swatches`) and a width dot (`tool.width_up`, then `tool.width 1` after 16). The grip drags it along the edge; a double-click on the grip or `view.tool_palette` collapses it to a 22 × 22 grip. Column 1's text clears it: 12 + 40 + 8 when expanded, 12 + 22 + 8 when collapsed. | `Sources/Shell/ToolWell.swift` |
| Highlight: a drag paints the range at once, in the current swatch, through the core's anchors. Collect: a toast `Collected: Jn 3:16`. Pen, highlighter pen and eraser: B039's canvas over the column's text, drawing with B039's brushes (pen in the theme accent, highlighter pen in the swatch). | `ShellModel.swift` (MARK: Ink, `dragEnded`), `ReaderNSTextView.mouseDown` |
| Every button, menu item and key dispatches a registered command through `ShellHandle.apply`; `handled: false` is performed by the host. Text size, line spacing and theme render from `session.reading`; brush width and swatches from `session.ink`. | `ShellModel.run`, `performHost` |
| Keys come only from the registry's Mac shortcuts (defaults and native aliases), read as chords by key code; bare keys apply only outside text fields. ⌘Z and ⌘⇧Z go to the shell's undo stack. | `Sources/Shell/ShellKeymap.swift`, `ShellModel.handle(chord:inTextField:)` |
| `TextSource`: resource id and reference in, display content and segments out. The fixture source covers BSB John 3:16–17, SBLGNT John 3:16, WLC Genesis 1:1–2 and the study note, and registers the short names with `setResourceTitles`. | `Sources/Shell/TextSource.swift` |
| Seed session: Study is BSB John 3:16 + WLC Gen 1:1 \| SBLGNT John 3:16 (group 1) \| `fixture:note:1` pinned as a tab, widths 0.4/0.35/0.25. Hebrew is WLC Gen 1:1. The bench session is 10 × 3 × 4. Only `reading.theme: "light"` overrides a core default. | `Sources/Shell/ShellSeed.swift` |
| Tokens: the values of `design/tokens/shell.json` as a Swift enum. The measure caps come from the core. | `Sources/Shell/ShellTokens.swift` |
| XCTest cases in the Mac test target | `Tests/ShellTests/ShellTests.swift` (16 cases), wired into `OpenLogosMacTests` in `project.yml` |
| `just preview-mac` opens the shell | `scripts/preview.sh` (hint text) |
| Pictures and measurement | `scripts/b072-mac-shots.sh`, `Sources/Shell/ShellLaunch.swift`, `ShellMeasure.swift`; `apps/apple/README.md` "The workspace shell (B072)" |

### Commits

| Commit | Content |
|---|---|
| `00a6d78` | reader surface: host typography, highlight marks, drag modes, focus and scroll hooks (additive) |
| `18da9f7` | the shell: tokens, text source, keymap, seed, model, views, tool well, launch hooks |
| `3fa5867` | XCTest cases for the shell |
| `16bcfdb` | palette state kept apart from the columns, cached command states (palette p95 16.5 → 8.4 ms), `-OPENLOGOS_RUN`, `-OPENLOGOS_SNAPSHOT`, `scripts/b072-mac-shots.sh`, first shots |
| `23421d4` | merge of `b070-core-shell` at dec4426 |
| `36a6426` | reading, ink, resize and scroll on the core's commands; measure cap from `layoutConstants()`; palette field in the toolbar row; anchored 770 pt dropdown; token-file test |
| `5e40926` | `apps/apple/README.md` section |
| `9990f1b` | no glass capsule behind the toolbar items (macOS 26) |
| `daca245` | shots retaken |
| `617eaff` | collapsed well as a 22 × 22 grip that column 1 still clears |
| `c8d69a2`, `98e8b6a` | token test reads `palette_dropdown_max_width` and `tool_well_collapsed` |
| `ea22834` | merge of `b070-core-shell` at a7d41e5 |
| `1210957` | `setResourceTitles`, core row titles, `formatShort`, text size −2…4, seed without the ink override |
| `2333ddd` | key glyphs on expanded rows (host `{n}` fill; removed in `c70b6af`) |
| `f12e27d` | shots retaken |
| `d9c0c26` | merge of `b070-core-shell` at 45fe931 |
| `c70b6af` | tab strip from `chapter_label`; the core's concrete keys on expanded rows |
| `ec1f070` | shots retaken (audited; PR #25) |
| this commit | this file |

## Cases (`just test-reader mac`, Tests/ShellTests)

The cases are hosted in the app, run natively and drive the real UniFFI `ShellHandle`:
`testTokensMatchTheBrief` ·
`testTokensMatchTheSharedTokenFile` (every value of `design/tokens/shell.json` against `ShellTokens` and `layoutConstants()`; it skips until the file is in this tree, and passed against the web branch's copy) ·
`testEveryMacShortcutInTheRegistryParsesAndNoneIsBoundTwice` ·
`testChordsResolveToRegisteredCommands` (⌘K, `/`, ⌥3, ⌘2, ⌥⇧2, ⌥T, ⌘T, ⌃Tab, ⌃⇧Tab, ⌥⇧], ⌘⇧N, ⌘Z, V H P M E C X; nothing outside the registry) ·
`testBareKeysOnlyOutsideTextFields` ·
`testTheSeedSessionIsTheBriefs` (labels `BSB Jn 3` …, link words, the core's default swatches) ·
`testCommandsGoThroughTheCoreAndUndo` ·
`testReadingAndInkSettingsLiveInTheSession` (text size up to 32 pt and disabled beyond, line spacing, theme, swap, `tool.swatch`, width dot, well clearance 60/42, measure cap) ·
`testColumnWidthsAndScrollAnchorsLiveInTheSession` (`column.resize`, `column.equalize`, quiet `tab.record_scroll`) ·
`testThePaletteAnswersEveryKeystrokeInSections` (every prefix, `@2` target, Enter, `>`, ↑/↓ wrap, disabled rows with reasons, expansion rows with `⌥⇧2` and no `{n}`, ⌥Enter) ·
`testTabsAcrossWorkspacesComeFromThePalette` (`@hebrew`, rows `BSB · Jn 3:16`) ·
`testTheTextSourceAnswersFixturesAndNothingElse` ·
`testTheShellRendersEveryShownTabOnTextKit2` (a window at 1440 × 900; every shown tab on TextKit 2, the scroll anchor read without falling back to TextKit 1, a tab switch, a size change re-laid out) ·
`testHighlightAndCollectDragsReachTheModel` ·
`testInkFollowsTheTool` ·
`testEscapeReturnsFocusToTheActiveTab`.

**34 cases** in OpenLogosMacTests: 8 reader, 10 ink and 16 shell. Of the shell cases, the
token-file test is skipped on this tree.

## Speed (specs/22 §7)

The run happens inside the Release app (arm64, unsigned): `-OPENLOGOS_SHELL_MEASURE <json>` over
the bench session (10 workspaces × 3 columns × 4 tabs = 120 tabs), 240 samples per row, one
interaction per display-link frame.

- **work** is from just before the command (or keystroke) to the end of a synchronous
  `layoutSubtreeIfNeeded` + `displayIfNeeded` of the window: the core call, `session()`, the
  SwiftUI update, layout and drawing.
- **next frame** runs from the start of the action to the next display-link callback. At 60 Hz
  its p50 sits at one refresh (16.7 ms); a p95 of 33 ms means some samples missed a frame.

Final tree (ec1f070, core 45fe931):

| Interaction (budget) | 1440 × 900 work p50 / p95 / max ms | next frame p95 | 1024 × 768 work p50 / p95 / max ms | next frame p95 |
|---|---:|---:|---:|---:|
| Palette keystroke: passage, commands, tabs, fixture text (16 ms) | 4.2 / 8.6 / 11.1 | 16.7 | 4.4 / 12.4 / 20.1 | 16.7 |
| Tab switch within a column (next frame) | 12.2 / 14.9 / 16.1 | 33.3 | 12.1 / 12.8 / 13.4 | 16.7 |
| Column focus (next frame) | 7.9 / 9.6 / 12.6 | 16.7 | 8.3 / 9.0 / 10.3 | 16.7 |
| Workspace switch, 10 × 3 × 4 (100 ms) | 39.4 / 48.7 / 53.4 | 61.7 | 38.8 / 41.0 / 42.0 | 55.3 |
| Tool change (next frame) | 8.3 / 10.0 / 12.8 | 16.7 | 8.1 / 9.9 / 11.3 | 16.7 |
| Open a chapter in a tab, warm, group follows (100 ms) | 11.4 / 12.6 / 13.6 | 16.7 | 11.4 / 12.0 / 13.0 | 16.7 |

Every p95 is within its budget. Tab switch at 1440 has the least headroom (14.9 ms p95, 16.1
max), because it rebuilds the revealed tab's text view: specs/22 §7 says hidden tabs release
their rendered text. A Mac frame costs much more than a browser frame (B072-web.md: work p95
under 8 ms) because the measurement includes a full synchronous display of a 1440 × 900 window
with three TextKit 2 views.

**The `session()` regression and its fix** (p95 work at 1440 × 900, the same harness):

| Interaction | before a7d41e5 (core dec4426) | with a7d41e5 | with 45fe931 |
|---|---:|---:|---:|
| Tab switch | 12.4 | **18.9** (1024: 18.4) | 14.9 |
| Column focus | 8.1 | 12.9 | 9.6 |
| Tool change | 10.5 | 14.7 | 10.0 |
| Open a chapter | 11.8 | 18.2 | 12.6 |
| Palette keystroke | 11.0 | 10.3 | 8.6 |
| Workspace switch | 48.9 | 43.6 | 48.7 |

With a7d41e5, every `ShellHandle.session()` call built `AliasTable::english()` and ran the
full reference parser for every tab to fill `reference_label`. A temporary XCTest (not
committed) timed `session()` on the 120-tab session at **5.7 ms per call**, against 0.006 ms
for `apply(tab.next)` and 0.75 ms for `commands()`. The host calls `session()` once per
command, so this cost landed on every frame. The core fix in 45fe931 builds the table once and
reads `Book.C.V` tab references through the OSIS fast path. The lead measured `session()` at
about 0.09 ms after it, and the numbers above returned to their earlier level.

Earlier in the work, before `16bcfdb`, the palette keystroke p95 was 16.5 ms over budget,
because every keystroke re-rendered all the columns and every menu item asked the core for
command states. Moving the palette into its own observable state and caching command states
per revision brought it to 8.4 ms.

Raw numbers with max and next-frame p50: `artifacts/b072/measure-1440.json` and
`measure-1024.json` (gitignored). Cold launch to text is not measured. The text section is
the core's fixture provider, not B071's search over the three editions.

## Screenshots (`docs/evidence/B072/`, 10 PNGs, 1.4 MB)

The app draws its own window into each picture (`-OPENLOGOS_SNAPSHOT`, `NSView.cacheDisplay`
of the frame view, so the toolbar row is included). This needs no screen-recording grant:
`screencapture -l` failed in this agent session on every window and the display alike, with
or without the sandbox. The pictures are at 1× because the display is 1×.

- `mac-light-1440`, `mac-sepia-1440`, `mac-dark-1440`, `mac-light-1024`: the seed session in
  the three themes at both widths.
- `mac-palette-passage`: `Jn 3:16 @2`, with the Passage and Tabs sections.
- `mac-palette-commands-expanded`: `>tab` with Commands expanded, showing expansion rows with
  `⌥⇧2` / `⌥⇧3` and a disabled row with its reason.
- `mac-palette-tabs` (sepia): `@jn`, rows `BSB · Jn 3:16`.
- `mac-palette-text` (dark): `#loved`, the Text section from the fixture provider.
- `mac-tools-demo`: a Highlight-tool highlight, B039 ink over column 2, and a
  `Collected: Jn 3:16` toast, with the Highlight tool current.
- `mac-placeholder-and-size`: group 1 navigated to Rom 8:28 (specs/21 placeholder in both
  columns) at text size 24.

`scripts/b072-mac-shots.sh` takes all 17 into `artifacts/b072/shots/`. The other seven are
`mac-dark-1024`, `mac-palette-commands`, `mac-palette-all`, `mac-tools-pen-dark`,
`mac-tools-collapsed-1024`, `mac-workspace-hebrew` and `mac-notes-dock`. **The full 17-shot set
is waiting to be archived on the NAS** (`/Volumes/office-hub/openlogos/evidence-archive/`). The
volume was not readable from this agent session ("Operation not permitted").

## Commands to reproduce

```sh
nix develop .#apple -c just bindings-swift        # never at the same time as an xcodebuild in the worktree
nix develop .#apple -c just test-reader mac       # Tests/ReaderTests + Tests/ShellTests in the Mac app
just preview-mac                                   # Debug build into artifacts/mac, opens the shell
scripts/b072-mac-shots.sh                          # the 17 pictures (needs the Debug build)
# Release build for the measurement:
nix develop .#apple -c bash -c 'cd apps/apple/OpenLogos && xcodegen generate --quiet && xcodebuild \
  -project OpenLogos.xcodeproj -scheme OpenLogosMac -configuration Release \
  -destination "platform=macOS,arch=arm64" -derivedDataPath ../../../artifacts/mac-release/DerivedData \
  CODE_SIGNING_ALLOWED=NO ARCHS=arm64 build'
scripts/b072-mac-shots.sh measure                  # artifacts/b072/measure-{1440,1024}.json
# iOS compile check, no simulator:
nix develop .#apple -c bash -c 'cd apps/apple/OpenLogos && xcodebuild -project OpenLogos.xcodeproj \
  -scheme OpenLogos -configuration Debug -destination "generic/platform=iOS Simulator" \
  -derivedDataPath ../../../artifacts/ios-compile/DerivedData CODE_SIGNING_ALLOWED=NO ARCHS=arm64 build-for-testing'
```

The `xcodebuild` commands inside `nix develop .#apple` unset a Nix-store `DEVELOPER_DIR` or
`SDKROOT` first, as `scripts/preview.sh` does.

## Lanes on the committed tree (ec1f070)

| Command | Result |
|---|---|
| `nix develop .#core -c just ci` | exit 0. `Summary [   7.670s] 302 tests run: 302 passed, 0 skipped` |
| `nix develop .#apple -c just test-reader mac` | exit 0. `Executed 34 tests, with 1 test skipped and 0 failures (0 unexpected)`; `** TEST SUCCEEDED **` |
| iOS `build-for-testing` (generic iOS Simulator, `ARCHS=arm64`) | exit 0. `** TEST BUILD SUCCEEDED **` |
| `nix develop .#apple -c just bindings-swift` | exit 0 after each core merge |

## Deviations from the brief and decisions to review

1. **Screenshots are drawn by the app** (`-OPENLOGOS_SNAPSHOT`), not taken with
   `screencapture -l`, which had no Screen Recording grant in this session. The approach is
   documented in `apps/apple/README.md`; the lead asked to keep it.
2. **The palette is in the toolbar row** (the lead's design review): one chrome row beside the
   traffic lights instead of a 44 pt bar under a title bar. The field's width is the window
   less 360, capped at 770. On macOS 26 the toolbar items' glass capsule is hidden so the row
   takes the theme's page colour.
3. **Host view state.** Only the tool well's position and collapse (`view.tool_palette` is
   `handled: false`) and a rule drag until its `column.resize` stay outside the session.
   Highlights and ink strokes live in host memory (B074 persists them) and ⌘Z does not undo
   them. Ink is a layer over the column that does not scroll with the text.
4. **Scroll timing differs from web.** The Mac records `tab.record_scroll` for every shown tab
   just before each command; web records 150 ms after scrolling stops. In both, the session
   holds the anchor before anything reads it.
5. **Labels.** By the lead's ruling, tab strips show `resource_title` + `chapter_label`
   (`BSB Jn 3`), the identity row shows `formatHuman` (`John 3:16`), and toasts use
   `formatShort` (`Jn 3:16`). This matches web.
6. **Seed.** Only `reading.theme: "light"` overrides a core default (the core's is
   `"system"`), as on web. The "system" theme follows the app's appearance.
7. **iOS keeps the B004 prototype as its root.** The shell sources compile for iOS, but the
   iPad shell is B075.
8. **`ARCHS=arm64`** is needed for Release Mac builds and for the generic iOS Simulator
   destination, because the XCFramework's slices are arm64 only. This predates the branch.
9. **Disk.** Free space on `/` fell from 28 GiB to 11 GiB over the session (DerivedData for
   Debug, Release, the test lane and the bindings), still above the 4 GiB stop line.

## Skipped or open

- **B072 acceptance not met on this branch:** "real chapters of the three editions" waits for
  B071 (fixture texts only), and the owner feedback session is not held yet.
- No persistence (B074).
- Cold launch to text is not measured.
- Mouse drags on the column rules and the grip were exercised in code (`commitResize`,
  `toolWellPosition`), not with a real pointer. No tablet was attached for ink.
- The token-file test skips until `design/tokens/shell.json` is in this tree (it lands with
  the web branch); it passed against that branch's copy.
- The full 17-shot set waits for NAS archiving (above).
