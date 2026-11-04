# B078 evidence (Mac) — continuous reading in the Mac shell

Recorded 4 November 2026 on the Mac mini (Apple M1, macOS 26.4, Xcode 26.6) in the worktree
`openlogos-b078mac`, branch `b078-mac-reading` (from `b078-core-reading` at ad10225, bindings
regenerated). Work was done by a Claude Code subagent while the owner slept. Nothing was
pushed or merged; no simulator, keychain or macOS user was touched. Only placeholder text and
the conformance fixtures' verses are on screen. The core part is B078-core.md; the web half
is `b078-web-reading`.

## What works (the brief's eight items)

| # | Item | Where |
|---|---|---|
| 1 | Typing `John 3:16` (palette, Enter) jumps the column to John 3 with verse 16's first line 8 pt under the sticky header, flashes it once (accent 35 % easing to 12 % over 0.9 s) and keeps a quiet mark (accent 12 %). The linked SBLGNT column goes to the same verse, marked too. A chapter's first verse (or a Psalm's title) opens with the chapter heading in view. | `Sources/Shell/ContinuousReader.swift` (`ReadingStream.open`, `jump`, `flash`) |
| 2 | Scrolling runs John 3 → 4 … 21 → Acts 1 and back up John 2 → 1 → Luke 24 without a jump: the verse at the top keeps its place on screen while blocks are added or released above it (the clip view's origin moves by exactly what changed; the tests check every 40 pt step to within 2 pt). The header (`Jn 4 · BSB`) follows at once; tab labels (`BSB Jn 4`) and identity rows (`John 4:7`) follow at once through `ReadingLabels` (only those views re-render), and the session is refreshed when scrolling stops. Flick scrolling at 120 pt/frame through 32 chapters never stalls (no step moved less than asked) and dropped no frame at 1440 × 900. | `ReadingStream.maintain`, `trim`, `anchored`, `ReadingScheduler`, `ReadingLabels` |
| 3 | Blocks (ChapterLayout.swift): the book title on a book's first chapter, the chapter label, the placeholder notice, verse numbers (none for a Psalm title), a new paragraph at `paragraph_start`, a line of its own for poetry (indented 1.5 em), the superscription italic and muted. | `Sources/Reader/ChapterLayout.swift` (`ReaderBlockPiece`, `ReaderLayout(pieces:)`) |
| 4 | Group 1 (BSB + SBLGNT) scrolls together verse for verse: each top-verse change is reported (`tab.record_scroll`) and each follower scrolled to its mapped verse without animation and without reporting back. A WLC tab in the group follows by its own numbering (English Malachi 4:5 → Hebrew 3:23; Joel 2:30 → 3:3). | `ShellModel.recordScroll`, `apply(positions:)`, `ReadingStream.apply`, `follow` |
| 5 | Back / Forward return to typed passages (Back to the verse the reader had scrolled to) and clear the mark; `John 3:16-18` marks the three verses; `Jude 3`, `Ps 3` (its title), `Genesis` (chapter 1, first verse) open; `John 22` shows the not-in-edition state (`John has no chapter 22`), as does an SBLGNT tab following into Psalms (`Psalms is not in SBLGNT`) and WLC on John (`John is not in WLC`). | `NotInEditionView` (ColumnsView.swift), `ShellModel.readingError`, `withBookRow` |
| 6 | BSB John 3:16–17, SBLGNT John 3:16 and WLC Genesis 1:1–2 show their real text with the fixtures' licence line under their block, and keep highlights, ink, selection, Word study, the context menu and Find in this chapter (which keeps to the chapter it was opened in). Placeholder verses are muted; a selection or word that touches one has Highlight, Collect, Note on it, Study, Highlight in…, Copy with reference and Word study disabled with `Placeholder text can't be quoted or collected` (well, context menu, a toast if run, drags refused); copying the text itself and searching stay. | `ShellModel.placeholderBlocked`, `contextMenu(for:)`, `refreshWell`, `dragEnded` |
| 7 | Highlights are kept as segment spans and painted in whatever window is shown; ink strokes are pinned to the verse under them (host-only anchor): they move with the text, hide while the verse is out of the window and come back on it. | `SegmentSpan`, `ReaderMark.spans`, `InkSheet.anchors` / `InkCanvasEngine.syncOffsets` |
| 8 | Only a window of chapters is in the text view: 5 blocks when the reader stops, at most 12 while scrolling (releases wait for the reader to stop, or go several at once at the cap). Longest window in the 32-chapter scrolls: 12 blocks, 531 verses, 19,830 UTF-16 units; process footprint 109–119 MB before and after each scroll. Hidden tabs keep their stream (40 at most). | `ReadingStream.maxBlocks`, `hardMaxBlocks`, `ShellModel.pooledStream` |

## Speed (specs/22 §7, Release, 10 × 3 × 4 session, 240 samples per row)

Tree dc17a62, `scripts/b078-mac-shots.sh measure`, raw JSON in `artifacts/b078/measure-1440.json`
and `measure-1024.json` (gitignored). Rows as B072: the work is the command through a
synchronous layout and display of the window. In the measurement session column 2 shows BSB
in link group 1 with column 1 (SBLGNT) and column 3 (BSB), so every move also moves two
followers.

| Interaction (budget ms) | 1440 × 900 p50 / p95 / max | 1024 × 768 p50 / p95 / max |
|---|---:|---:|
| **Open a chapter: typed jump outside the window** (locate, chapter blocks, layout, verse at top, two followers, drawn) (100) | 22.73 / 38.03 / 39.47 | 21.38 / 37.10 / 37.85 |
| **Scroll into the next verse** (record_scroll, two followers scrolled, labels, drawn) (16) | 5.64 / 7.06 / 8.38 | 5.76 / 7.99 / 9.07 |
| **Linked follow, host side** (core record_scroll and both followers scrolled; 245 samples) (16) | 0.66 / 0.84 / 0.94 | 0.66 / 0.82 / 0.89 |
| **Labels after a scroll** (session refresh, shell re-rendered, drawn; on settle) (16) | 9.00 / 10.70 / 11.75 | 8.79 / 8.99 / 9.30 |
| Window edit, whole: add a block below (62 / 61) | 3.04 / 4.05 / 7.24 | 3.23 / 4.27 / 4.79 |
| Window edit, whole: add a block above (6 / 4) | 2.67 / 2.75 / 2.75 | 2.61 / 2.64 / 2.64 |
| Window edit, whole: release blocks above (8 / 8) | 3.68 / 4.48 / 4.48 | 3.78 / 4.82 / 4.82 |
| Deferred work, one item per frame (a block's load, its layout, a release; one column) (157 / 153) | 3.63 / 5.31 / 9.38 | 3.95 / 6.05 / 7.69 |
| Palette keystroke (16) | 5.59 / 13.77 / 20.43 | 5.65 / 14.51 / 23.00 |
| Tab switch within a column (16) | 10.81 / 12.51 / 14.38 | 10.60 / 12.25 / 14.94 |
| Tab switch after reading (16) | 11.17 / 11.96 / 12.92 | 11.08 / 11.82 / 12.37 |
| Column focus (16) | 7.30 / 9.08 / 10.55 | 7.43 / 7.75 / 8.62 |
| Workspace switch (100) | 37.93 / 41.17 / 59.59 | 37.38 / 38.88 / 54.38 |
| Tool change (16) | 7.24 / 8.82 / 10.23 | 7.77 / 9.57 / 10.28 |
| Column focus, original ↔ translation (16) | 9.20 / 9.42 / 10.16 | 9.07 / 9.44 / 10.27 |
| Translation → notes dock (16) | 7.46 / 7.74 / 8.54 | 7.25 / 7.64 / 10.38 |
| Selection appears → selection group drawn (16) | 8.96 / 10.08 / 12.25 | 10.08 / 10.68 / 11.94 |
| Open a chapter in a tab, warm (100) | 11.66 / 11.79 / 13.90 | 11.47 / 11.63 / 12.51 |

**Scripted continuous scrolls** (one step per display-link frame from Matthew 1 until 32
chapters have passed the top; driver plus two followers; frame = display-link interval):

| Run | frames | frame p50 / p95 / max ms | frames > 17.5 ms | > 34 ms | step p50 / p95 ms | stalls |
|---|---:|---:|---:|---:|---:|---:|
| 1440, 40 pt/frame (2,400 pt/s) | 1015 | 16.67 / 16.67 / 91.56 | 8 | 1 | 6.66 / 8.73 | 0 |
| 1440, 120 pt/frame (7,200 pt/s) | 337 | 16.67 / 16.67 / 16.67 | 0 | 0 | 6.09 / 6.95 | 0 |
| 1024, 40 pt/frame | 1361 | 16.67 / 16.67 / 33.33 | 8 | 0 | 6.77 / 9.39 | 0 |
| 1024, 120 pt/frame | 452 | 16.67 / 16.67 / 33.33 | 5 | 0 | 6.08 / 7.05 | 0 |

Every budget holds. Every window change during scrolling cost at most one dropped frame
except one 91.6 ms frame in the 1440 40 pt run, whose own work was a 9.4 ms step and a 3.7 ms
deferred item (the rest of that frame was outside the app's handlers; it is recorded as
measured). Compared with B077 (p95 at 1440): palette 13.77 (14.40), tab switch 12.51 (13.61),
column focus 9.08 (8.70), workspace 41.17 (41.78), tool 8.82 (9.17), original ↔ translation
9.42 (11.02), dock 7.74 (8.57), selection 10.08 (8.98), chapter warm 11.79 (11.68): no
regression beyond run-to-run noise. The next-frame p95 of the dock and selection rows is above
16.7 for the reason B076-mac.md gives (a `prepare` step).

How the numbers were reached (first runs on this branch had 40–70 ms window edits and a 27 ms
tab switch): blocks are laid out once and shared by every stream (`ReaderBlockPiece` cache);
a hidden tab keeps its stream, so tab and workspace switches lay nothing out again; a jump
loads its chapter and the next, the one before arrives a frame later; TextKit 2 is laid out
exactly only from the window's start down to the view (an edit above the reader re-lays out
everything down to the view, so releases wait for the reader to stop); a block added below is
laid out in a frame of its own before it scrolls in; loads, layouts and releases that are not
urgent run one per frame (`ReadingScheduler`); the header draws itself (a label's text change
asked SwiftUI for a whole-window layout at every chapter); tab labels and identity rows follow
through `ReadingLabels` instead of a session refresh (which re-rendered the whole shell, 25 ms,
at every chapter).

## Tests

`just test-reader mac`: 68 cases in OpenLogosMacTests (58 from B077, one changed, and 10 new
in `Tests/ShellTests/ReadingTests.swift`):
`testATypedPassageOpensItsChapterWithTheVerseAtTheTopFlashedAndMarked` (palette `Jn 3:16`, verse
at 8 pt, the chapters around it, header, highlight_range, flash then the quiet 12 % painted, the
linked SBLGNT column at 3:16 marked, `BSB Jn 3`) ·
`testRangesBooksChaptersAndOneChapterBooksOpenWhereTheyShould` (3:16–18 marked as one range,
Jude 3, Ps 3 at its title with nothing marked, Genesis at 1:1 with nothing before it) ·
`testAPassageNotInTheEditionShowsTheNotInEditionState` (John 22, back to John 3:16, WLC on John) ·
`testScrollingCrossesChaptersAndBooksBothWaysWithoutMovingTheText` (John 21 → Acts 1 with the
book heading, header, live labels for both linked tabs, then the session's labels, up into
John 19, Malachi 4 → Matthew 1, Genesis 2 → 1; every step within 2 pt) ·
`testOnlyAWindowOfChaptersIsEverRendered` (32 chapters from Matthew 1: never more than 12
blocks, under 40,000 units, releases happen, back to 5 when the reader stops) ·
`testLinkedTabsFollowVerseForVerseWithoutFeedingBack` (SBLGNT at the same verse at 8 pt, the
follower's scroll not reported, WLC by its numbering for Malachi and Joel) ·
`testBackAndForwardReturnToTypedPassages` · `testBlocksCarryHeadingsParagraphsPoetryAndTitles`
(Ps 3 heading and notice, title without number in italic, poetry indented on its own line,
Genesis' book title only on chapter 1) ·
`testRealFixtureVersesKeepTheirTextAndPlaceholdersCannotBeQuoted` (real BSB and Greek text,
the well and the menu disabled with words, the toast, the refused drag) ·
`testHighlightsAndInkStayOnTheirVersesAsBlocksComeAndGo` (a highlight and a pen stroke on
John 3:16; the stroke scrolls with the text, hides while John 3 is released and returns on its
verse; the highlight is back on exactly "loved the world").
Changed: `testColumnWidthsAndScrollAnchorsLiveInTheSession` (a scroll into the next verse now
moves the passage and the revision, still without undo; within the verse it is quiet) and
`testCompareOpensTheOtherEditionInTheNextColumnAndUndoes` (revealing the word scrolls column
1, and its group follows, so undo restores column 2 to where it was before Compare).

## Screenshots (`docs/evidence/B078/`, 8 PNGs, 2.1 MB, 1440 × 900)

`mac-read-jump-john-3-16` (flash at its peak, both columns) · `mac-read-john-acts-boundary`
(the end of John 21 above the `Acts` heading, both columns) · `mac-read-linked-mid-scroll`
(BSB and SBLGNT aligned at 3:17 mid-scroll) · `mac-read-psalm-3` (title, poetry, Psalm 4's
heading and notice; SBLGNT `Psalms is not in SBLGNT`) · `mac-read-not-in-edition` (`John 22`)
· `mac-read-dark-range` (dark, `John 3:16–18` marked, identity rows in words) ·
`mac-read-placeholder-menu` (the menu on placeholder text, rows disabled with words) ·
`mac-read-genesis-wlc` (WLC Genesis 1: the book heading, real Hebrew 1:1–2, placeholders,
licence line). `scripts/b078-mac-shots.sh` retakes them (`-OPENLOGOS_TYPE` types a passage
after the columns lay out; `-OPENLOGOS_SCROLL 1:-160` scrolls column 1 as the reader would).

## Lanes (committed tree dc17a62)

| Command | Result |
|---|---|
| `nix develop .#core -c just ci` | exit 0. `Summary [   7.125s] 330 tests run: 330 passed, 0 skipped` |
| `nix develop .#apple -c just test-reader mac` | exit 0. `Executed 68 tests, with 0 failures (0 unexpected)` |
| `nix develop .#apple -c just test-swift` | exit 0. `Executed 10 tests, with 0 failures (0 unexpected)`; `conformance cases: 1717 passed, 0 failed` |

Nothing under `crates/` changed: `git diff ad10225 -- crates` is empty.

## Decisions (for the owner's review)

- **Jump placement**: the typed verse's first line 8 pt under the sticky header, no context
  above; a chapter's first verse (or a titled Psalm, or a whole book) opens with the chapter's
  heading at the top instead. Proposed to b078web for lockstep (no reply received during the
  run).
- **Flash and mark**: accent 35 % easing to 12 % over 0.9 s, the quiet mark covering the verses
  from the first one's number to the last one's end.
- **Sticky header**: a 28 pt bar `Jn 3 · BSB` (chapter semibold, edition muted) over a 1 pt
  rule; the text sits below it, not under it.
- **Top verse** is the verse whose text begins the first line at the top of the text; a jump's
  verse stays the top verse until the reader scrolls, even when its first line starts with the
  verse before.
- **Labels while scrolling**: tab labels and identity rows follow at once (ReadingLabels); the
  session refresh (whole shell) waits for the scroll to settle (150 ms).
- **Window**: 5 blocks at rest; up to 12 while scrolling (releases mostly when the reader
  stops: an edit above the reader costs TextKit 2 a relayout down to the view). A jump loads its
  chapter and the next; the chapter before comes a frame later.
- **Placeholder refusals**: Highlight, Collect, Note on it, Study, Highlight in…, Copy with
  reference (`copy.passage`) and Word study, with `Placeholder text can't be quoted or
  collected`; Copy and Search stay.
- **Whole book typed** (`Genesis`): the core palette offers no passage row for a bare book, so
  the Mac adds one (`Genesis 1`, “The whole book, from its first chapter”, `nav.navigate
  Gen.1`) when `reading.locate` answers granularity `book`. Reported to team-lead as a core
  gap; a core row would replace it.
- **A reveal** (a selection made in code, Find's next match) scrolls only when the range is out
  of view, and brings it to the top; that scroll is reported like the reader's (linked tabs
  follow).
- **Ink** is pinned to the verse under a stroke's top, host-side only (the ink document is
  unchanged); pinned ink is not drawn over the sticky header.
- **Find in this chapter** in a window searches the chapter the menu was opened in.
- **Nearest-verse hint** (`mapping: nearest`): none shown.

## Deviations and skipped

- The window grows to 12 blocks while scrolling (brief: "a window of chapters"; the core
  suggested "one or two each side") — measured above, back to 5 at rest.
- The ink anchor is not persisted (the ink document has no field for it; strokes are host
  memory until B074 anyway).
- iPad: the continuous surface is Mac-only (`#if os(macOS)`); the iPad shell keeps B072's
  fixture surface. Not built for iOS in this run (no simulator lanes were in scope).
- No reply from b078web was received while working, so visible details were proposed to it and
  recorded here; a mismatch with the web half needs the lead's ruling.
