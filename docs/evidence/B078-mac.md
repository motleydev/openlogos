# B078 evidence (Mac) — continuous reading in the Mac shell

Recorded 4 November 2026 on the Mac mini (Apple M1, macOS 26.4, Xcode 26.6) in the worktree
`openlogos-b078mac`, branch `b078-mac-reading` (from `b078-core-reading` at ad10225; merged
again at 2e53c87 for the core's palette book rows, bindings regenerated). Revision 2 follows
the lead's lockstep rulings (reading line, mark, not-in-edition, window cap, Find, live
follow, placeholder words); the numbers, pictures and lanes below are from that tree. Work was done by a Claude Code subagent while the owner slept. Nothing was
pushed or merged; no simulator, keychain or macOS user was touched. Only placeholder text and
the conformance fixtures' verses are on screen. The core part is B078-core.md; the web half
is `b078-web-reading`.

## What works (the brief's eight items)

| # | Item | Where |
|---|---|---|
| 1 | Typing `John 3:16` (palette, Enter) jumps the column to John 3 with verse 16's first line on the reading line, 24 pt under the sticky header (a sliver of the verse before shows), flashes it once (the accent at 30 % fading out over 1.2 s) and keeps B072's underline mark (2 pt, accent at 60 %) on every verse of `highlight_range`. The linked SBLGNT column goes to the same verse, marked too. | `Sources/Shell/ContinuousReader.swift` (`ReadingStream.open`, `jump`, `flash`) |
| 2 | Scrolling runs John 3 → 4 … 21 → Acts 1 and back up John 2 → 1 → Luke 24 without a jump: the verse at the top keeps its place on screen while blocks are added or released above it (the clip view's origin moves by exactly what changed; the tests check every 40 pt step to within 2 pt). The header (`Jn 4 · BSB`) follows at once; tab labels (`BSB Jn 4`) and identity rows (`John 4:7`) follow at once through `ReadingLabels` (only those views re-render), and the session is refreshed when scrolling stops. Flick scrolling at 120 pt/frame through 32 chapters never stalls (no step moved less than asked) and dropped no frame at 1440 × 900. | `ReadingStream.maintain`, `trim`, `anchored`, `ReadingScheduler`, `ReadingLabels` |
| 3 | Blocks (ChapterLayout.swift): the book title on a book's first chapter, the chapter label, the placeholder notice, verse numbers (none for a Psalm title), a new paragraph at `paragraph_start`, a line of its own for poetry (indented 1.5 em), the superscription italic and muted. | `Sources/Reader/ChapterLayout.swift` (`ReaderBlockPiece`, `ReaderLayout(pieces:)`) |
| 4 | Group 1 (BSB + SBLGNT) scrolls together verse for verse: each change of the verse on the reading line is reported (`tab.record_scroll`, at most once per frame, never behind window work) and each follower scrolled to its mapped verse on its reading line without animation and without reporting back; when scrolling settles the position is reported again with its scalar. The verse on the reading line is the verse that begins on that row, else the verse running through it. A WLC tab in the group follows by its own numbering (English Malachi 4:5 → Hebrew 3:23; Joel 2:30 → 3:3). | `ShellModel.recordScroll`, `apply(positions:)`, `ReadingStream.apply`, `follow` |
| 5 | Back / Forward return to typed passages (Back to the verse the reader had scrolled to) and clear the mark; `John 3:16-18` marks the three verses; `Jude 3`, `Ps 3` (its title), `Genesis` (the core's palette book row, `nav.navigate Gen.1`) open; `John 22` shows the not-in-edition state, titled `Berean Standard Bible · John 22`, body the core's words (`John has no chapter 22.`), buttons `Choose another…` (the palette) and `Close tab`; likewise an SBLGNT tab following into Psalms (`Psalms is not in SBLGNT.`) and WLC on John. | `NotInEditionView` (ColumnsView.swift), `ShellModel.readingError` |
| 6 | BSB John 3:16–17, SBLGNT John 3:16 and WLC Genesis 1:1–2 show their real text with the fixtures' licence line under their block, and keep highlights, ink, selection, Word study, the context menu and Find in this chapter (the chapter of the right-clicked word; from the palette or keyboard, the chapter on the reading line). Placeholder verses are muted, each block carries `Placeholder text — this edition is not installed`; a selection or word that touches one has Highlight (its submenu removed), Collect, Note on it, Study, Highlight in…, Copy with reference, Word study and Search lemma disabled with `Placeholder text can’t be quoted or collected` (well, context menu, a toast if run, drags refused); copying the text itself and searching stay. | `ShellModel.placeholderBlocked`, `contextMenu(for:)`, `refreshWell`, `dragEnded` |
| 7 | Highlights are kept as segment spans and painted in whatever window is shown; ink strokes are pinned to the verse under them (host-only anchor): they move with the text, hide while the verse is out of the window and come back on it. | `SegmentSpan`, `ReaderMark.spans`, `InkSheet.anchors` / `InkCanvasEngine.syncOffsets` |
| 8 | Only a window of chapters is in the text view: 5 blocks when the reader stops, at most 9 while scrolling (releases wait for the reader to stop, or go several at once at the cap). Longest window in the 32-chapter scrolls: 9 blocks, 433 verses, 16,488 UTF-16 units; process footprint 109–118 MB before and after each scroll. Hidden tabs keep their stream (40 at most). | `ReadingStream.maxBlocks`, `hardMaxBlocks`, `ShellModel.pooledStream` |

## Speed (specs/22 §7, Release, 10 × 3 × 4 session, 240 samples per row)

Tree 666fa7a, `scripts/b078-mac-shots.sh measure`, raw JSON in `artifacts/b078/measure-1440.json`
and `measure-1024.json` (gitignored). Rows as B072: the work is the command through a
synchronous layout and display of the window. In the measurement session column 2 shows BSB
in link group 1 with column 1 (SBLGNT) and column 3 (BSB), so every move also moves two
followers.

| Interaction (budget ms) | 1440 × 900 p50 / p95 / max | 1024 × 768 p50 / p95 / max |
|---|---:|---:|
| **Open a chapter: typed jump outside the window** (locate, chapter blocks, layout, verse on the reading line, two followers, drawn) (100) | 23.57 / 38.42 / 40.10 | 22.29 / 37.28 / 40.52 |
| **Scroll into the next verse** (record_scroll, two followers scrolled, labels, drawn) (16) | 4.93 / 6.52 / 9.67 | 4.96 / 6.24 / 8.79 |
| **Linked follow, host side** (core record_scroll and both followers scrolled; 242 samples) (16) | 0.73 / 0.98 / 1.16 | 0.74 / 0.88 / 1.03 |
| **Labels after a scroll** (session refresh, shell re-rendered, drawn; on settle) (16) | 8.89 / 9.28 / 10.76 | 8.84 / 9.17 / 11.30 |
| Window edit, whole: add a block below (62 / 61) | 2.60 / 3.60 / 4.13 | 2.73 / 3.90 / 4.28 |
| Window edit, whole: add a block above (4 / 4) | 2.66 / 3.11 / 3.11 | 2.32 / 3.07 / 3.07 |
| Window edit, whole: release blocks above (14 / 14) | 3.07 / 3.80 / 3.92 | 3.09 / 3.88 / 5.45 |
| Deferred work, one item per frame (a block's load, its layout, a release; one column) (161 / 161) | 3.26 / 4.88 / 6.26 | 3.50 / 5.51 / 6.89 |
| Palette keystroke (16) | 4.95 / 14.99 / 20.73 | 5.13 / 14.41 / 21.57 |
| Tab switch within a column (16) | 11.83 / 14.67 / 17.37 | 10.64 / 11.28 / 11.88 |
| Tab switch after reading (16) | 11.28 / 12.09 / 14.05 | 11.19 / 11.95 / 14.82 |
| Column focus (16) | 7.53 / 9.97 / 12.20 | 7.18 / 7.61 / 8.49 |
| Workspace switch (100) | 38.54 / 40.92 / 69.85 | 37.48 / 39.30 / 54.34 |
| Tool change (16) | 7.31 / 8.94 / 10.59 | 7.25 / 9.22 / 11.83 |
| Column focus, original ↔ translation (16) | 9.27 / 9.46 / 11.03 | 9.20 / 9.59 / 10.75 |
| Translation → notes dock (16) | 7.53 / 8.16 / 10.56 | 7.42 / 8.76 / 9.68 |
| Selection appears → selection group drawn (16) | 9.24 / 9.59 / 11.00 | 10.33 / 11.46 / 13.04 |
| Open a chapter in a tab, warm (100) | 11.83 / 12.10 / 14.06 | 11.63 / 11.91 / 14.68 |

**Scripted continuous scrolls** (one step per display-link frame from Matthew 1 until 32
chapters have passed the reading line; driver plus two followers; frame = display-link
interval):

| Run | frames | frame p50 / p95 / max ms | frames > 17.5 ms | > 34 ms | step p50 / p95 ms | stalls |
|---|---:|---:|---:|---:|---:|---:|
| 1440, 40 pt/frame (2,400 pt/s) | 1011 | 16.67 / 16.67 / 33.33 | 4 | 0 | 6.61 / 8.27 | 0 |
| 1440, 120 pt/frame (7,200 pt/s) | 335 | 16.67 / 16.67 / 16.80 | 0 | 0 | 5.93 / 6.82 | 0 |
| 1024, 40 pt/frame | 1357 | 16.67 / 16.67 / 33.33 | 4 | 0 | 6.68 / 9.48 | 0 |
| 1024, 120 pt/frame | 451 | 16.67 / 16.67 / 33.33 | 1 | 0 | 5.92 / 6.83 | 0 |

Every budget holds, and no window change during scrolling cost more than one dropped frame (no
frame over 34 ms in any run). Compared with B077 (p95 at 1440): palette 14.99 (14.40), tab
switch 14.67 (13.61), column focus 9.97 (8.70), workspace 40.92 (41.78), tool 8.94 (9.17),
original ↔ translation 9.46 (11.02), dock 8.16 (8.57), selection 9.59 (8.98), chapter warm
12.10 (11.68): within run-to-run noise. The next-frame p95 of the dock and selection rows is
above 16.7 for the reason B076-mac.md gives (a `prepare` step). Revision 1 (dc17a62: 8 pt
margin, 12-block cap) for comparison: typed jump 38.03, next verse 7.06, tab switch 12.51, one
91.6 ms frame in the 1440 40 pt run (now none over 34 ms).

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

`just test-reader mac`: 69 cases in OpenLogosMacTests (58 from B077, two changed, and 11 new
in `Tests/ShellTests/ReadingTests.swift`):
`testATypedPassageOpensItsChapterWithTheVerseAtTheTopFlashedAndMarked` (palette `Jn 3:16`, verse
on the 24 pt reading line, the chapters around it, header, highlight_range, the flash at most
30 % and fading out, then the underline mark painted with no background, the linked SBLGNT
column at 3:16 marked, `BSB Jn 3`) ·
`testRangesBooksChaptersAndOneChapterBooksOpenWhereTheyShould` (3:16–18 marked as one range,
Jude 3, Ps 3 at its title with nothing marked, Genesis at 1:1 with nothing before it) ·
`testAPassageNotInTheEditionShowsTheNotInEditionState` (John 22, back to John 3:16, WLC on John) ·
`testScrollingCrossesChaptersAndBooksBothWaysWithoutMovingTheText` (John 21 → Acts 1 with the
book heading, header, live labels for both linked tabs, then the session's labels, up into
John 19, Malachi 4 → Matthew 1, Genesis 2 → 1; every step within 2 pt) ·
`testOnlyAWindowOfChaptersIsEverRendered` (32 chapters from Matthew 1: never more than 9
blocks, under 40,000 units, releases happen, back to 5 when the reader stops) ·
`testLinkedTabsFollowVerseForVerseWithoutFeedingBack` (SBLGNT at the same verse on its reading line, the
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

`mac-read-jump-john-3-16` (the flash 0.3 s in, the verse on the reading line, both columns) · `mac-read-john-acts-boundary`
(the end of John 21 above the `Acts` heading, both columns) · `mac-read-linked-mid-scroll`
(BSB and SBLGNT aligned at 3:17 mid-scroll) · `mac-read-psalm-3` (title, poetry, Psalm 4's
heading and notice; SBLGNT `Psalms is not in SBLGNT`) · `mac-read-not-in-edition` (`Berean Standard Bible · John 22`, the core's words, the two buttons)
· `mac-read-dark-range` (dark, `John 3:16–18` with the underline mark, identity rows in words) ·
`mac-read-placeholder-menu` (the menu on placeholder text, rows disabled with words) ·
`mac-read-genesis-wlc` (WLC Genesis 1: the book heading, real Hebrew 1:1–2, placeholders,
licence line). `scripts/b078-mac-shots.sh` retakes them (`-OPENLOGOS_TYPE` types a passage
after the columns lay out; `-OPENLOGOS_SCROLL 1:-160` scrolls column 1 as the reader would).

## Lanes (committed tree f09183f)

| Command | Result |
|---|---|
| `nix develop .#core -c just ci` | exit 0. `Summary [   6.964s] 330 tests run: 330 passed, 0 skipped` |
| `nix develop .#apple -c just test-reader mac` | exit 0. `Executed 68 tests, with 0 failures (0 unexpected)`; rerun on c677daf (room below the last chapter): exit 0, `Executed 69 tests, with 0 failures (0 unexpected)` |
| `nix develop .#apple -c just test-swift` | exit 0. `Executed 10 tests, with 0 failures (0 unexpected)`; `conformance cases: 1733 passed, 0 failed` |

Nothing under `crates/` changed on this branch: `git diff --stat b078-core-reading..HEAD -- crates` is empty.

## Decisions (for the owner's review)

Lockstep with web, as the lead ruled (2026-11-04):

- **Reading line** 24 pt under the 28 pt sticky header: a typed verse, a follower's verse and
  the reported position all sit there; the position is the verse that begins on that row, else
  the verse running through it with the row's scalar.
- **Mark**: B072's underline on every verse of `highlight_range`, plus a one-time accent flash
  (30 %) fading out over 1.2 s.
- **Not in edition**: `<edition name> · <passage>`, the core's words with a full stop,
  `Choose another…` and `Close tab`.
- **Window**: two chapters each side at rest; at most 9 blocks while scrolling.
- **Find in this chapter**: the right-clicked word's chapter; else the reading line's.
- **Linked follow** live (at most once per frame) plus a settle report.
- **Placeholder**: muted verses, the per-block notice, `Placeholder text can’t be quoted or
  collected`.
- **Whole books** come from the core's palette rows (2e53c87); revision 1's host row is gone.
- **Room below an edition's last chapter**: 80 % of the view, as on the web, so Revelation
  22:21 and WLC Malachi 3:24 reach the reading line; on the Mac a little more when the last
  verse needs it (a one-line last verse needs the view less the reading line and that line,
  about 92 % at 1440 × 900). `testAnEditionsLastVerseCanReachTheReadingLine`.

The Mac's own choices: releases wait for the reader to stop where the cap allows (an edit
above the reader costs TextKit 2 a relayout down to the view); a block below is laid out in a
frame of its own before it scrolls in; tab labels and identity rows follow a scroll through
`ReadingLabels` and the session refresh waits for the scroll to settle (150 ms); a reveal (a
selection made in code, Find's next match) scrolls only when the range is out of view and is
reported like the reader's scroll; ink is pinned to the verse under a stroke's top (host
only); no nearest-verse hint.

## Deviations and skipped

- The ink anchor is not persisted (the ink document has no field for it; strokes are host
  memory until B074 anyway).
- iPad: the continuous surface is Mac-only (`#if os(macOS)`); the iPad shell keeps B072's
  fixture surface. Not built for iOS in this run (no simulator lanes were in scope).
