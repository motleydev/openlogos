# B078 evidence (web): continuous reading in the browser

Recorded 3–4 November 2026 on the Mac mini (Apple M1, Chrome 156, headless) in the worktree
`openlogos-b078web`, branch `b078-web-reading`, from `b078-core-reading` (ad10225). A Claude
Code subagent did the work while the owner slept. Nothing was pushed or merged, no
simulator was touched, and this branch does not change `crates/`. No Bible text beyond the
conformance fixtures appears: every other verse is a placeholder edition's labelled line. The
Mac half is `b078-mac-reading`, with its own evidence. The visible details below were sent to
the Mac agent for lockstep (see "Lockstep").

Owner direction (2026-11-03): "When I type a Bible passage, I want to be taken to the whole
context, infinite scroll style, of scripture in context." Spec: specs/23-continuous-reading.md.
The core part is docs/evidence/B078-core.md, and its "For the host parts" is the step list
followed here.

## What was built

| Item | Where |
|---|---|
| **The reading handle**: `core.ReadingHandle.placeholder()` at boot. `shell.setResourceSchemes(reading.schemes())` follows `setResourceTitles` / `setResourceLanguages`. A reading tab whose edition the handle has (bsb, sblgnt, wlc) is a stream; the text source keeps names, direction, language, the study note and the fixture licence line | `apps/web/shell/shell.js` `main`, `isStream`; `text-source.js` |
| **Stream of chapter blocks**: `ReadingStream` keeps a window of `chapterContent` blocks around the reading line. Each block has a book title (`heading.book_title`, a book's first chapter only), a chapter heading (`chapter_label`: "Chapter 3", "Psalm 23", "Jude"), a placeholder notice line, then `div.verses`: paragraphs at `paragraph_start`, one `p.line` per poetry verse (hanging indent), `p.superscription` for a Psalm title (italic, chrome colour, no verse number). The B005 markup (`span.verse[data-segment]`, `sup.marker`, `.t`) and the joiner between every two verses are kept, so each block is one core paragraph for `selectionToAnchor` | `apps/web/shell/reading.js` |
| **Window**: the block at the reading line plus two each side, more while the text reaches less than 1.5 viewports past either edge, never more than 9 blocks. Blocks further away are released. Opening paints the chapter with one neighbour each side and loads the rest on the next frame | `ReadingStream.fill`, `WINDOW` |
| **No jump**: the scroller's own scroll anchoring is off (`overflow-anchor: none`). Before any block is inserted or released, the block at the reading line is measured; afterwards `scrollTop` takes up whatever changed above it (one forced layout). Releases are done before loads, so the cap leaves room on the side being read into | `fill` |
| **Reading line**: 24 px below the sticky header. A typed passage lands on it (a sliver of context shows above), a follower's verse is put on it, and the position is read there. The position is the verse that *begins* on the reading line's row; if none begins there, the verse running through it, with the row's first character as the scalar. It is pure geometry (`getClientRects`, a binary search over verses and characters), so a menu or the palette over the text does not change it | `READING_LINE`, `measureTop`, `scrollToSegment` |
| **Sticky header**: a 28 px bar at the top of the reading area: "BSB · Jn 3" (resource short name, `heading.short_label`) for the block at the reading line, blurred over the text | `.reading-header`, `updateHeader` |
| **Jump, flash and mark**: `nav.navigate` (palette, passage link, paste passage, Back/Forward, restore) arrives as positions with `scrolled: false`. Each listed tab's navigation serial goes up, so its column is rebuilt there: `locate(content_id, reference)` → `chapter_key` / `segment_id` (a scrolled tab's `scroll_anchor` when it has one, as for a restore). The verses of `highlight_range` flash once (accent at 30 %, fading over 1.2 s) and keep the quiet `.target` underline until the tab's `highlight_range` is null (the next navigation, Back or Forward). A range across chapters is marked on every rendered block | `followPositions`, `buildStream`, `ReadingStream.open` / `flash` / `setMark` |
| **The column is not rebuilt by scrolling**: `columnSignature` leaves a stream tab's reference out and carries its navigation serial instead. A reference that changes without a position (an undo, for instance) is caught by comparing it with the one the stream was put at or reported (`host.known`) | `columnSignature`, `noteNavigation` |
| **Position reported live**: when the verse on the reading line changes (at most once a frame), `tab.record_scroll {workspace_id, tab_id, segment_id, scalar}`, and once more with the final scalar 150 ms after scrolling stops. It is quiet: `sync()`, then the positions: the driver's and each follower's tab label (`BSB Jn 4`, the core's `chapter_label`) and identity row (`John 4:7`, `formatHuman` as in B072) are changed in place | `ReadingStream.tick` / `settle`, `reportTop`, `refreshLabels` |
| **Linked scroll**: for each follower position with `scrolled: true`, its stream scrolls to the mapped `segment_id` without animation, loading that chapter if it is not rendered. The stream remembers the `scrollTop` it set, and a scroll that leaves it there is not the reader's, so it is never reported (no feedback). Scrolling the follower drives the group the other way | `followPositions`, `ReadingStream.follow`, `expected` |
| **Versification**: the core maps each follower's verse (`setResourceSchemes`). BSB Malachi 4:5 puts a linked WLC tab at Malachi 3:23 and "Mal 4" at 3:19. A bridge or a verse an edition numbers differently lands on the rendered verse whose `number…last_number` holds it | `holding` |
| **Not in this edition**: a `locate` error shows specs/21's state with the core's words: "Berean Standard Bible · John 22 / John has no chapter 22.", "SBL Greek New Testament · Ps 3 / Psalms is not in SBLGNT." It shows no text from another edition, and it has the buttons "Choose another…" and "Close tab" | `notInEdition` |
| **Placeholders are visible and never quoted**: placeholder verses are drawn in a muted chrome colour, and each block holding them has the line "Placeholder text — this edition is not installed". When a selection or right-clicked word touches one, Highlight, Collect, Note on it, Study (tool well), and Highlight (its submenu removed), Collect, Note on it, Copy with text, Word study and Search lemma (context menu) are disabled with "Placeholder text can’t be quoted or collected". The Highlight tool's drag, a Word study click and the commands refuse with the same toast. A block holding fixture verses ends with their licence line (the SBLGNT one is CC BY 4.0) | `QUOTE_COMMANDS`, `withPlaceholderRows`, `refusePlaceholder`, `attributeBlocks` |
| **Fixture verses keep every B072–B077 behaviour**: highlights are anchors repainted for whichever blocks are rendered, so a released and reloaded block paints them again. A selection across blocks is cut per block and its anchors joined. Word study, the context menu, Find in this chapter (in the named chapter's block), Copy with text (from `chapterContent`) and Note on it all work | `streamAnchors`, `paintHighlights`, `findInChapter`, `verseText` |
| **Ink on a band**: the margin canvas is three viewports tall around the visible text. It moves (and repaints) only when the reader leaves it or blocks change, and it has no bitmap until there is ink or an ink tool. Points are page coordinates; each stroke remembers the verse it began at and that verse's top, and is drawn shifted by however far that verse has moved since (blocks loaded or released above it). A stroke whose verse is not rendered is not drawn | `sizeCanvas`, `placeBand`, `strokeShift`, `verseAnchorAt` |
| **Reflow keeps the place**: a change of text size, spacing, measure, column widths, focus mode, the well's collapse, the dock or the window size puts each stream back on the verse it had at the reading line | `restoreStreams` |
| **Room at the end**: 80 % of the viewport below the stream, so an edition's last verse (WLC Malachi 3:24, Revelation 22:21) can reach the reading line | `.measure.stream` |
| **Speed changes found while measuring**: a rebuilt column now takes its old one's place, where before every column was re-inserted and the other streams restored. Tool CSS reads one root class per effect instead of `[data-tool]`, so a tool change no longer restyles every verse. The Word study cursor is set on the text root | `renderColumns`, `TOOL_CLASSES` |

## The eight "must work" items

1. **Typing `John 3:16`**: the column (and its linked SBLGNT column) opens at John 3 with 3:16
   on the reading line (within 2 px, tested), 3:16 flashes once and stays underlined, and the
   window is John 1–5. Screenshot `web-read-jump-john-3-16.png`.
2. **Scrolling** down John 21 into Acts 1–2 and up John 1 into Luke 24–23 is seamless. The
   tested `maxJump` of the verse under the eye is ≤ 2 px over hundreds of 60 px steps, the
   blocks are released and loaded on the way, and the header, tab label ("BSB Acts 2", "BSB
   Lk 23") and identity row follow. A scripted 90 px-a-frame scroll through 90 chapters (Ps 1
   → Ps 90) had no frame over 25 ms (table below). Genesis 2 → 1 upward has no jump either.
3. **Headings and layout**: "Acts" heads Acts 1 only, "Psalms" heads Psalm 1 only. Psalm 3
   opens at its title (`Ps.3.0`, no verse number, italic), then has eight poetry lines and
   prose paragraphs every five verses (the placeholder layout).
4. **Linked columns**: BSB and SBLGNT (group 1) put the same verse on their reading lines while
   scrolling, verse for verse (the follower's verse lands on its reading line within 2 px).
   Only the driver reports; scrolling the SBLGNT drives the BSB back. A WLC tab linked into
   group 1 follows by its own scheme: "Mal 4" → WLC Mal 3:19, "Mal 4:5" → 3:23. Scrolling the
   BSB on into Matthew leaves the WLC at its edition's end.
5. **Back and Forward** return to the typed passages (`John 3:16–18` → `Ps 23` → `Jude 3`,
   ⌘[ ⌘[ ⌘]), and the mark is cleared on Back as the core does. `John 3:16-18` marks the
   three verses. `Jude 3` opens at Jude 1:3. `Ps 23` opens at 23:1 (the BSB has no title
   verse there). Typing `Genesis` (the core's book row, merged from b078-core-reading
   2e53c87) opens Gen 1:1 on the reading line, with SBLGNT showing "Genesis is not in
   SBLGNT."; `Jo` lists Joshua, Job, Joel, Jonah, John. `John 22` shows the not-in-edition
   state in both columns.
6. **Fixture verses** (BSB Jn 3:16–17, SBLGNT Jn 3:16, WLC Gen 1:1–2) show real text, and
   all 42 B072–B077 cases pass on the streams: highlight drag, Hebrew highlight, pen and
   eraser, word study, context menus, find in chapter, copy with text, selection and notes.
   Placeholder verses are muted and noted, and their quote and collect actions are disabled
   with words (tested on the well, the menu, the tool drag and the toast).
7. **Highlights and ink survive unloading**: a highlight on "eternal life" and a pen stroke at
   3:17 are still there after John 3 has been released (scrolled ~16,000 px on) and loaded
   again. The highlight is repainted as the same range, and the stroke's pixels sit at 3:17
   again (within 25 % of the count before).
8. **Window cap**: never more than 9 blocks. 780 DOM nodes at most in a column through 90
   chapters, and the JS heap grows by 1–2 MB over the run (table below).

## Cases (`just test-shell-web`, 54 in all, 12 new)

| Case | Checks |
|---|---|
| `reading-jump-flash-mark` | the palette "John 3:16" → 3:16 on the reading line ±2 px in BSB and SBLGNT, flash once and mark (BSB and the follower), headers "BSB · Jn 3" / "SBLGNT · Jn 3", window John 1–5, after 1.4 s no flash and the mark stays |
| `reading-range-and-back` | `John 3:16-18` marks three verses and the identity row reads "John 3:16–18". `Ps 23` → Ps.23.1, unmarked. `Jude 3` → Jude.1.3. ⌘[ ⌘[ ⌘] walk the visits and clear the mark. Typing `Genesis` → Gen.1.1, SBLGNT "Genesis is not in SBLGNT" |
| `reading-palette-books` | "Jo" lists John among the Passage rows; "Genesis" + Enter → `Gen.1`, Gen 1:1 on the reading line, header "BSB · Gen 1", SBLGNT not-in-edition |
| `reading-invalid-reference` | `John 22` → "Berean Standard Bible · John 22 … John has no chapter 22." in both columns, no text; ⌘[ returns to the text |
| `reading-scroll-down-across-books` | John 21:20 → Acts 2 at 60 px steps: no jump > 2 px, header "BSB · Acts 1" on the way, tab "BSB Acts 2", identity "Acts 2:n", "Acts" heads Acts 1, SBLGNT on the same verse, ≤ 9 blocks |
| `reading-scroll-up-across-books` | John 1:3 → Luke 23 upward with no jump, header "BSB · Lk 24", tab "BSB Lk 23"; Genesis 2:3 → Genesis 1 upward with no jump |
| `reading-linked-follow` | after 120 steps the SBLGNT has the BSB's verse on its reading line ±2 px, only t1 reports, t3's passage is that verse; scrolling column 2 drives column 1 |
| `reading-wlc-follows-its-scheme` | WLC linked into group 1 in column 3: "Mal 4" → WLC Mal 3:19 ("WLC · Mal 3"), SBLGNT not-in-edition, the BSB scrolls on into Matthew 1 with no jump |
| `reading-psalm-poetry` | Ps 3 opens at its title `Ps.3.0`, heading "Psalm 3", no book title, title without a number, 8 poetry lines, "Psalms" on Psalm 1 only |
| `reading-placeholder-actions` | a placeholder selection: the four selection tools disabled with the reason, pressing Collect toasts it, the menu's Collect / Note on it / Highlight disabled (no submenu), the Highlight tool refuses, a fixture verse still highlights |
| `reading-highlight-and-ink-survive-unload` | highlight and stroke in John 3, John 3 released, reloaded: the highlight is repainted as "eternal life" and the stroke is painted at 3:17 again |
| `reading-dom-window-cap` | Ps 100 at 120 px steps through ≥ 30 chapters: ≤ 9 blocks, < 6,000 nodes, no jump |

B072–B077 cases adapted to the stream (the checks are the same; what changed is how the
tests find things): `placeholder-and-navigation` (Romans 8 is now a placeholder chapter in the
stream with its header and notice, not the "not installed" box), `scroll-anchor-recorded`
(scrolls on from the typed verse; restoring returns to the same verse), `pen-and-eraser`
(draws in the visible part of the column), `menu-keyboard-open` (the caret goes in John
3:16's text), and `menu-column-background-theme` / `menu-paste-passage` (the background is the
sticky header, since the text fills the column).

## Screenshots (docs/evidence/B078, 8 web files)

`just test-shell-web --case seed-session --read-shots docs/evidence/B078`:

| File | State |
|---|---|
| `web-read-jump-john-3-16.png` | "John 3:16" typed after Ps 23: 3:16 on the reading line in BSB and SBLGNT, mid-flash, sticky headers, the fixture licence lines, Chapter 4 with its notice below |
| `web-read-jump-dark.png` | the same in the dark theme |
| `web-read-john-acts-boundary.png` | mid-scroll across John 21 → Acts 1 in both columns: the "Acts" book title, "Chapter 1", the notice, the header still "Jn 21", the 21:20 mark |
| `web-read-linked-bsb-sblgnt.png` | the two linked columns after scrolling into John 4: both at 4:44 on the reading line, labels "BSB Jn 4" / "SBLGNT Jn 4" |
| `web-read-psalm-3-poetry.png` | Psalm 3: the title line, poetry lines, Psalm 4 and 5 headings; SBLGNT "Psalms is not in SBLGNT." |
| `web-read-wlc-malachi-scheme.png` | "Mal 4:5" in the BSB with the WLC linked in column 3 at Mal 3:23 (right to left, marked), SBLGNT not-in-edition, the BSB into Matthew |
| `web-read-invalid-john-22.png` | "John 22": the not-in-edition state in both columns |
| `web-read-placeholder-notice.png` | close-up of column 1 at John 4: header, "CHAPTER 4", the notice, muted placeholder verses |

## Speed

`just test-shell-web --case seed-session --bench --samples 200`, headless Chrome, 200 samples
per row (more where stated). **Work** and **frame** are as in B072-web.md: work is the
synchronous change plus a forced layout; frame runs to the next `requestAnimationFrame`, so it
includes the wait for that frame. Two runs, A/B, interleaved with the B077 tree (main fc1c439,
its own JS, on this branch's wasm, since the API change is additive). The Mac agent was building
on the same machine (load 2.5–4.4). Final runs on 9a56a93.

**B078 rows** (p50 / p95 ms, run 1 · run 2):

| Interaction (budget) | session | work p50 / p95 | frame p95 |
|---|---|---:|---:|
| Typed jump → stream painted: `nav.navigate` round 12 references across the canon (Ps 119:105, Gen 1:1, Rev 22:21, Mt 5:3, Isa 53:5, Jn 3:16–18, Acts 1:8, Ps 23, Rom 8:28, Jude 3, Obad 1, Mal 4:5), the driver's column and its linked follower rebuilt and opened (≤ 100 ms) | seed | 4.4 / 7.7 · 4.2 / 6.4 | 18.8 · 18.5 |
| | bench | 8.6 / 13.2 · 8.5 / 13.1 | 20.9 · 20.6 |
| Linked follow: `tab.record_scroll` through every follower scrolled and laid out, sampled during the scripted scroll (≤ 16 ms) | seed (1 follower), n = 574 | 1.2 / 1.5 · 1.0 / 1.2 | — |
| | bench (2 followers), n = 575 | 1.7 / 2.2 · 2.6 / 3.5 | — |

**Scripted continuous scroll**: 90 px every frame for 600 frames from Psalm 1, so through 90
chapters (Ps 1 → Ps 90; 92 appends, 89 releases), in column 1 with its linked follower:

| | seed run 1 · run 2 | bench run 1 · run 2 |
|---|---|---|
| frame interval p50 / p95 / max | 16.7 / 16.7 / 16.8 · 16.7 / 16.8 / 16.8 | 16.7 / 16.7 / 16.8 · 16.7 / 16.7 / 16.8 |
| frames over 25 ms (a dropped frame beyond one) | 0 · 0 | 0 · 0 |
| one scroll frame's work (window, header, reading line) p50 / p95 / max | 0.1 / 2.0 / 4.8 · 0.1 / 1.5 / 4.1 | 0.1 / 0.6 / 1.8 · 0.1 / 0.9 / 2.6 (both columns' frames, n = 1,177) |
| blocks at most · DOM nodes in the column at most | 9 · 780 | 9 · 780 |
| JS heap after (growth over the run) | 6.9 MB (+1.1) · 9.1 MB (+2.0) | 10.8 MB (+1.9) · 11.1 MB (+1.0) |

No frame was dropped. A block's load, insertion and the `scrollTop` correction fit in the
frame they happen in (the per-frame work maximum is 4.8 ms). The window holds 9 blocks of up to
780 elements, whatever the distance scrolled.

**B072/B076/B077 rows, B077 tree → B078 tree** (work p95 ms, run 1, run 2; budget 16 ms unless
stated):

| Interaction | seed | bench |
|---|---:|---:|
| Palette keystroke | 3.0, 3.7 → 1.0, 1.0 | 3.5, 3.1 → 2.9, 2.6 |
| Keystroke with the Text section | 2.1, 2.5 → 0.7, 0.7 | 2.0, 2.1 → 1.4, 1.2 |
| Trusted keystroke → palette rendered (handler) | 2.2, 2.1 → 2.1, 2.0 | 2.8, 2.8 → 2.3, 2.4 |
| Tab switch (`tab.next`) | 5.2, 5.1 → 3.8, 4.7 | 4.4, 5.6 → 6.6, 7.0 |
| Column focus | 4.5, 4.4 → 3.2, 2.5 | 3.7, 3.8 → 4.3, 2.9 |
| Workspace switch (≤ 100 ms) | 4.3, 4.1 → 8.6, 8.7 | 6.1, 4.8 → 14.8, 13.8 |
| Tool change | 3.1, 2.8 → 7.8, 7.7 | 4.0, 4.8 → 8.7, 8.3 |
| B076 translation ↔ original | 3.5, 2.5 → 2.6, 2.2 | — |
| B076 translation → notes | 4.4, 4.2 → 2.5, 2.3 | — |
| B076 selection group | 2.6, 2.9 → 3.2, 2.9 | 2.7, 2.9 → 3.1, 3.1 |
| B077 menu opens (word / tab / background) | 3.8–4.9 / 2.9–3.6 / 1.8–2.3 → 3.4–4.5 / 2.1–3.2 / 1.8–2.6 | 2.9–4.1 / 2.6–2.8 / 2.4–2.5 → 4.3–4.6 / 2.8–3.2 / 2.3 |
| B077 choose → Compare / Search | 1.6–1.7 / 2.9–3.0 → 1.6–2.1 / 3.0–3.1 | 3.4 / 3.4–3.6 → 3.9–4.2 / 3.1–3.2 |
| B077 trusted right-click → menu drawn (handler) | 3.5, 3.5 → 5.2, 5.2 | 3.5, 3.2 → 5.7, 5.7 |
| Open a chapter, warm (≤ 100 ms) | 5.4, 5.2 → 7.0, 7.0 | 6.9, 6.5 → 10.1, 10.3 |

Every row stays inside its budget. What grew, and why:
- **Opening a chapter** and the **workspace switch** now build a stream: the chapter with one
  neighbour each side (3 chapters instead of 2 fixture verses), plus the linked follower's. The
  ten-workspace switch rebuilds three streams. The rest of each window loads on the next frame,
  so the frame p95 of a tab switch or an open is 18–21 ms against 16–17 ms before (that frame
  also carries the fill, 1–3 ms).
- **Tool change** p95 7.7–8.7 ms (p50 1.0–2.8): toggling into or out of Highlight restyles the
  verse spans for the tool's selection colour (`.tool-highlight .t::selection`). Before the fix
  in 9a56a93, every tool change restyled every verse (p95 12–16 ms). Pen ↔ eraser costs 0.7 ms.
- **The right-click** handler is about 2 ms slower: hit-testing and selecting a word in a column
  of 780 elements instead of 20. It is still drawn on the next frame (frame p95 16.8–17.8,
  the same as the B077 tree's 17.4–17.9).

## Lanes

On the committed tree bb34626 (b078-core-reading 2e53c87 merged, wasm rebuilt; the evidence
update after it is documentation only):

| Lane | Summary line | Exit |
|---|---|---:|
| `nix develop .#core -c just ci` | `Summary [7.988s] 330 tests run: 330 passed, 0 skipped`; validate `PASS: 36 schema/example validations; … 21 conformance fixtures; …`; fmt, clippy `-D warnings`, doc tests, wasm32 check | 0 |
| `nix develop .#web -c just test-web` | `conformance cases: 1733 passed, 0 failed (api 0.1.3)`; `tests 10, pass 10, fail 0` | 0 |
| `nix develop .#web -c just test-wasm` | `test result: ok. 9 passed; 0 failed` | 0 |
| `nix develop .#web -c just test-dom-reader` | `DOM reader: 13/13 cases passed` | 0 |
| `nix develop .#web -c just test-shell-web` | `Summary: Web shell: 54/54 cases passed` | 0 |

The speed tables were measured on 9a56a93, before the merge; the merge changes the core's
palette book rows only.

## Commits

| Commit | Content |
|---|---|
| `cd3bd6d` | reading tabs as a continuous stream of chapter blocks (`reading.js`, the shell wiring, styles; B072–B077 cases adapted) |
| `5685431` | the 11 reading cases and `--read-shots`; room after an edition's last verse |
| `4666a7a` | speed: the window fills on the next frame, a rebuilt column replaces only itself, no ink bitmap without ink; the verse beginning on the reading line's row is the position; the B078 bench rows |
| `9137c0f` | the single-passage reading path removed |
| `9a56a93` | tool change restyles only what the tool changes |
| `5674857` | the 8 screenshots |
| `ca1fa82` | this file |
| merge | b078-core-reading 2e53c87 (the core's book rows in the palette) |
| `bb34626` | `reading-palette-books` ("Jo", "Genesis"), the whole-book check typed; specs/23 §7 live follow and the reading line |
| this commit | this file updated |

## Lockstep

Sent to the Mac agent (b078mac) at the start and again with the final choices. **No reply
had arrived when this was written**, so these are the web's choices as offered, not an
agreement:

- The reading line is 24 px below the sticky header. The jump lands there, followers are put
  there and the position is read there. The position is the verse that begins on the reading
  line's row, else the one running through it (with a scalar).
- Sticky header "BSB · Jn 3": a 28 px bar, chrome font 12 px semibold, for the block at the
  reading line.
- Book title on a book's first chapter (reading font, 1.6×, semibold, centred). The chapter
  heading is chrome font 13 px, uppercase, letter-spaced, chrome colour. A Psalm title is
  italic in the chrome colour with no number. Poetry is one line per verse with a 1.5 em
  hanging indent.
- Placeholder: the verses are muted, with one notice per block, "Placeholder text — this
  edition is not installed" (italic 12 px). The disabled reason is "Placeholder text can’t be
  quoted or collected" on Highlight, Collect, Note on it, Study, Copy with text, Word study and
  Search lemma.
- Flash: accent at 30 %, fading out over 1.2 s, once. Then the B072 underline as the quiet mark.
- Not in edition: "<resource name> · <passage>", the core's message with a full stop, and
  "Choose another…" / "Close tab".
- Window: 2 each side, more up to 1.5 viewports of reach, at most 9. Live follow on each
  change of verse, plus a settle report.
- The identity row stays `formatHuman` ("John 4:7"), as in B072. The tab label is the core's
  "BSB Jn 4".

## Deviations from the brief

- **Live following** rather than only on settle (approved by team-lead; specs/23 §7 now says so): specs/23 §7 says to report when scrolling
  settles. The brief asks for linked columns that "scroll together verse-for-verse", so the
  stream reports each time the verse on the reading line changes (at most once a frame) and
  once more on settle. The cost is in the table (linked follow p95 ≤ 4 ms in the ten-workspace
  session).
- **The reading line** (24 px under the header; approved, in specs/23 §7) is the position, not the very top edge. That
  keeps the typed verse, with its sliver of context above, the reported verse, so the
  identity row does not drift to the verse above.
- **The SBLGNT column with Genesis/Psalms/Malachi** shows the not-in-edition state, as specs/23
  §7 says (it is linked to the BSB). It does not stay where it was.
- **Text selection across blocks** joins each block's anchors. A highlight across a chapter
  boundary is two anchor runs; this is not tested beyond the single-block cases.

## Decisions for the owner

- The reading line, 24 px below a 28 px sticky header: is the sliver of the previous verse
  above the typed one right, or should the typed verse sit at the very top?
- The window: two chapters each side (up to 9 short ones), 1.5 viewports of reach. Raise it
  if a fast scrollbar drag should reach further before waiting a frame.
- The placeholder look: muted chrome-coloured verses plus one italic notice per chapter. The
  alternative is a single banner per column.
- Live follow, as above.
- A follower whose edition lacks the book (SBLGNT for Genesis) shows the not-in-edition state
  instead of staying on its last passage. That is spec'd, and it is visible in
  `web-read-psalm-3-poetry.png`.

## Skipped

- B071 packs (`addPackManifest` + fetched chunks): the stream calls `chapterContent` only, so
  a file pack plugs in where `ReadingHandle.placeholder()` is. Fetching chunks before
  `chapterContent` (`chunk_not_loaded`) is not built yet; it needs the packs to test against.
- A "nearest verse" hint for `mapping: nearest` (optional in the core's step list).
- Headed Chrome numbers; the runs are headless.
- Deuterocanonical books and the Tanakh order (the core has neither in a shipped edition).
