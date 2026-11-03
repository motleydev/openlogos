# B077 evidence (web): the OpenLogos context menu in the browser

Recorded 3 November 2026 on the Mac mini (Apple M1, Chrome 156) in the worktree
`openlogos-b077web`, branch `b077-web-menu`, which has B076 web and `b077-core-menu` (806627c)
merged (e7b5eaa). The work was done by a Claude Code subagent. Nothing was pushed or merged,
no simulator was touched, and this branch does not change `crates/`. Only fixture text appears
on screen. The Mac half is `b077-mac-menu` and has its own evidence; the visible details below
were proposed to the Mac agent for lockstep (see "Lockstep").

Owner direction (2026-11-03): "override right-click context menu on web and native app; power
features like word searches." The core decides the menu for what was clicked (specs/22 §4.2,
docs/evidence/B077-core.md); the web shell draws it and performs the host-side effects.

## What works

| Item (the B077 brief) | Where |
|---|---|
| **Replaces the browser menu** in reading text, notes (tab and dock), tab strips, identity rows and badges, and the empty area of a column. The palette field, the tool well, the shell's own menus and the toast keep the browser's menu. **⇧-right-click** falls through to the browser's menu everywhere (the event is left alone, `defaultPrevented` stays false) | `apps/web/shell/shell.js` (`targetAt`, the `contextmenu` listener) |
| **Targets**: a word (the `Intl.Segmenter` word under the pointer, selected as the menu opens, as macOS does) with `text`, `reference`, `segment_id`, `scalar_start`/`scalar_end` (code points in the verse), `resource_id` and `tab_id`; a right-click inside a selection gives the selection (`text` as the core quotes it, start and end verse, edition, tab); a passage link in a note gives `note_link`; other note text `note_text` (with the selected text, `tab_id` of a Notes tab, none for the dock); a tab; a column (identity row or badge); the background of a column | `targetAt`, `wordTarget`, `selectionTarget`, `wordAtCaret` |
| **Opening changes nothing**: a right-click does not focus the column (the column's pointerdown ignores the right button); the menu's `focus` is applied only when a row is chosen, then the row, both under the menu's `context` (`run(invocation, ctx)`). Before dispatch the selection or note caret the menu was opened on is put back, since keyboard navigation moves focus into the menu | `showContextMenu`, `chooseContextItem` |
| **Drawing**: a custom element styled from the shell tokens in light, sepia and dark: the title as a dimmed header, sections between separators (labels become the ARIA group names), an icon per row (the core's names as inline SVG, 32 new ones), a check for `active`, a swatch dot for `value`, keys as glyphs on the right (`⌘⇧C`, `⌥T`, `⌥⇧2`), `›` for a submenu, disabled rows dimmed with `disabled_text` as a second line (and as the tooltip). One level of submenus, opened to the right or flipped to the left at the edge. The menu and submenus stay inside the viewport (8 px gap), moving left or up from the pointer when needed | `apps/web/shell/context-menu.js`, `shell.css` (`.ctx-*`), `ICONS` in `shell.js` |
| **Keyboard and ARIA**: `role="menu"`, `menuitem` / `menuitemradio` with `aria-checked`, `group`, `separator`, `aria-haspopup` / `aria-expanded`, `aria-disabled`, `aria-keyshortcuts`. ↑ ↓ Home End move (disabled rows too, so their reasons are read), → opens a submenu, ← and Esc close it, Return or Space runs, Esc closes the menu and returns focus (and the selection) to where it was, Tab closes, typed letters jump (type-ahead, 500 ms buffer). While the menu is open no registry key fires (type-ahead "c" is not the Collect tool). A submenu parent only opens its submenu (the Mac cannot run one either) | `context-menu.js` `onKey`; the shell's keydown returns while it is open |
| **Opening from the keyboard**: ⇧F10 or the ContextMenu key. The target is the selection, else the word at a collapsed caret in reading text, else the caret in a note, else the focused tab, else the active column's background; the menu opens at it with the first row focused | `contextMenuKey` |
| **Closes** on a press outside, a scroll or wheel outside the menu, a window resize and the window losing focus | `context-menu.js` |
| **Host effects** (`handled: false`) | `perform` |
| · `search.text` scope `edition` / `all` / `workspace` | the palette opens with `#word` (its Text section) and a chip in the field: "This edition · BSB", "All editions", "Workspace". The Text section is expanded and its rows are filtered to the scope's editions (from the existing fixture text source; edition: that resource, workspace: every resource open in the workspace). × on the chip drops the scope; closing the palette clears it | `searchText`, `queryPalette`, `renderScopeChip` |
| · `search.text` scope `chapter` (Find in this chapter) | every case-insensitive match in the column's text is painted (CSS Custom Highlight `ol-find`, the current one darker), the current one scrolled into view; toast "4 matches in John 3" (or "No matches in John 3"). ⌘G / ⇧⌘G step and wrap ("2 of 4 in John 3"), Esc clears. A rebuild of the column clears it | `findInChapter`, `findKey` |
| · `copy.text` / `copy.reference` / `copy.passage` | clipboard `loved` / `John 3:17` (`formatHuman`) / `“eternal life” — John 3:16 (BSB)`, the shape of B076's Note on it quote; without `text` the verse (or range) comes from the text source (`“For God did not send … through Him.” — John 3:17 (BSB)`). Toasts "Copied “loved”", "Copied John 3:17", "Copied John 3:16 with its text" | `copyText`, `copyPassage`, `verseText` |
| · `host.cut` / `host.copy` / `host.paste` | the browser's cut and copy on the restored selection (`execCommand`, then `navigator.clipboard.writeText` if refused); paste reads the clipboard and inserts it at the note caret the menu was opened on; toasts "Copied" / "Cut" | `clipboardAction` |
| · `nav.paste_passage` | reads the clipboard, parses it (all of it, else the reference after the dash that `copy.passage` writes, else the first run that reads as one) and runs `nav.navigate` with the toast "Opened John 3:17"; "No passage on the clipboard" otherwise. When the browser does not grant reading the clipboard, the shell's own last copy is used | `pastePassage`, `passageIn`, `readClipboard` |
| · `selection.highlight_in` | the selection (the right-clicked word) highlighted in the chosen swatch | `highlightSelection(swatch)` |
| · `note.remove_link` | the passage link becomes its words, and both views of the note follow; toast "Removed link to John 3:16" | `removeLink` |
| · `tab.open_edition`, `tab.close_others` | core commands; applied and re-rendered (undo with ⌘Z) | `run` |
| Tests: 14 new `just test-shell-web` cases (42 in all); the registry count case now expects 84 commands | `apps/web/shell/test/run-chrome.mjs` |
| Speed rows; screenshots with `--menu-shots <dir>` | `bench()` in `shell.js`, the runner's bench block, `menuScreenshots()` |

### Icons (the core's menu names → web SVG)

The well's 21 names keep their B076 drawings. New: `search-all` (glass over a globe), `lemma`
(glass with α), `find` (page with a glass), `compare` (two pages side by side), `open` (arrow
out of a box), `column`, `new-column` (column with a plus), `cross-references` (two lines with
arrows both ways), `copy`, `copy-passage` (copy with lines), `cut` (scissors), `paste`
(clipboard), `remove-link` (struck link), `close` (×), `close-others`, `duplicate` (copy with a
plus), `move`, `pin`, `follow`, `link`, `focus` (corners round a dot), `move-left`,
`move-right`, `new-tab` (folder tab with a plus), `equalize`, `close-column`, `paste-passage`
(clipboard with an arrow), `text-size` (A A), `text-larger` (A +), `text-smaller` (A −),
`text-reset` (A), `theme` (half-filled circle). A swatch row shows a dot in its colour instead.

### Commits

| Commit | Content |
|---|---|
| `bb9ff5c` | the menu element (`context-menu.js`), its styles, the scope chip's place |
| `c1e8d48` | targets, focus-first dispatch, keyboard opening, host commands, find in chapter, scoped search, icons |
| `82de890` | 14 cases, registry count 84 |
| `81e0f53` | speed rows (open per target, choose → effect, trusted right-clicks with a pointerdown baseline) |
| `9d52846` | screenshots, the header aligned with the labels |
| `e8749a9` | toasts and the Workspace chip label as agreed with the Mac host |
| `b307a70` | this file |
| this commit | the commit table |

## Cases (`just test-shell-web`, WebDriver Actions: trusted keys and pointer, right-clicks with button 2)

- `menu-word-translation`: right-click on "loved" in BSB → header "loved", 4 sections, the 12 rows in core order; Word study disabled "Word study works on Greek and Hebrew text", Search lemma "Needs the lexicon import", Cross references "Needs cross-reference data"; Highlight, Open John 3:16 in and Copy are submenus; Collect shows ⌘⇧C; the word is selected; the target is `{word, loved, John.3.16, scalars 11–16, bsb, t1}`; nothing dispatched on open; the menu is inside the viewport; Esc closes and the word stays selected.
- `menu-word-greek-focus-on-choose`: on ἠγάπησεν in column 2 Word study is enabled and Compare in BSB is offered; opening leaves column 1 active; choosing Word study dispatches `tab.focus` then `tab.open_lexicon` (Lexicon Jn 3 in column 3, column 2 active).
- `menu-compare-and-undo`: with SBLGNT's column closed, Compare in SBLGNT on "world" in BSB opens SBLGNT John 3:16 in column 2 (`tab.open_edition`, the core's); ⌘Z takes it back.
- `menu-highlight-submenu-keyboard`: ↓ ×9 reaches Highlight, → opens Yellow ✓ / Green / Blue / Pink (radio rows with swatches, to the right), ← closes it, → ↓ Return highlights "eternal" green; type-ahead "c" → Compare in SBLGNT, "cop" → Copy, and the tool stays Select.
- `menu-selection-copy-passage`: a right-click inside the "eternal life" selection gives the selection target and ends Copy, Copy with reference (Study present); Copy with reference → `“eternal life” — John 3:16 (BSB)`, toast "Copied John 3:16 with its text"; Copy ▸ Copy verse with reference on "condemn" copies the whole of John 3:17; Copy ▸ Copy reference → "John 3:17".
- `menu-search-scope`: Search “world” in this edition → palette `#world`, chip "This edition · BSB", only BSB rows; on κόσμον in all editions → "All editions" with SBLGNT rows; × drops the scope; Esc clears it.
- `menu-find-in-chapter`: Find in this chapter on "world" → 4 matches painted, toast "4 matches in John 3"; ⌘G → "2 of 4 in John 3"; ⇧⌘G twice wraps to the last; Esc clears the paint.
- `menu-tab-close-others-undo`: the tab menu of BSB Jn 3 (header "BSB · Jn 3:16", 7 rows); Close other tabs leaves one tab, ⌘Z brings WLC back; Move to ▸ on SBLGNT shows "Column 2 · SBLGNT" disabled "The tab is already in column 2" (its key ⌥⇧2 still shown, dimmed); Column 3 · Note moves it.
- `menu-note-link-remove`: after Note on it, a right-click on the passage link → `note_link` John.3.16 in tab t4, header "John 3:16", rows Open John 3:16 in, Compare in, Copy reference, Remove link; Remove link leaves "“eternal life” — John 3:16" as text; on the note's text the menu starts Cut, Copy, Paste, Bold, Italic.
- `menu-column-background-theme`: the badge of column 2 → "Column 2", first row Focus column 2, column 1 still active; Move left moves SBLGNT to column 1; the background below the text → New tab, Paste passage, Text size, Theme; Theme ▸ System / Light ✓ / Dark / Sepia; Dark applies.
- `menu-paste-passage`: Copy reference on John 3:17, then in the Hebrew workspace background ▸ Paste passage navigates WLC to John.3.17.
- `menu-escape-hatch`: ⇧-right-click on a word → no OpenLogos menu, `defaultPrevented` false; a plain right-click → `true` and the menu; the palette field keeps the browser's menu.
- `menu-keyboard-open`: ⇧F10 on the focused tab → the tab menu with Close tab focused, ↓ Close other tabs, Esc returns focus to the tab; ⇧F10 with a selection → the selection menu; the ContextMenu key with a collapsed caret in "loved" → the word menu with the word selected.
- `menu-closes`: a press outside closes it, a scroll closes it; a right-click 6 px from the bottom-right corner at 1024 × 600 keeps the whole menu inside the viewport.

`registry-bindings` now expects 84 commands (70 + B077's 14).

## Screenshots (docs/evidence/B077, 8 web files, 1.0 MB)

`just test-shell-web --case seed-session --menu-shots docs/evidence/B077`, headless Chrome,
1440 × 900:

| File | State |
|---|---|
| `web-menu-word-greek.png` | right-click on ἠγάπησεν (SBLGNT): Word study enabled, hovered; Compare in BSB |
| `web-menu-word-bsb-disabled.png` | right-click on "loved" (BSB): Search lemma, Word study (hovered) and Cross references disabled with their words |
| `web-menu-highlight-submenu.png` | the same menu with Highlight ▸ open: Yellow ✓, Green hovered, Blue, Pink |
| `web-menu-selection.png` | the "eternal life" selection's menu |
| `web-menu-tab.png` | the SBLGNT tab's menu with Move to ▸ open (its own column disabled with words) |
| `web-menu-note-link.png` | a passage link in the note: Compare in ▸ BSB / SBLGNT, flipped to the left at the window edge |
| `web-menu-find-in-chapter.png` | Find in this chapter for "world": four matches, the second current after ⌘G, toast "2 of 4 in John 3" |
| `web-menu-dark.png` | dark theme: the word menu on "Son" with Copy ▸ open |

## Speed (specs/22 §7; budget: drawn on the next frame, 16 ms)

`just test-shell-web --case seed-session --bench --samples 200`, headless Chrome, 200 samples
per row. **Work** and **frame** are defined as in B072-web.md: work is the synchronous change
plus a forced layout, and frame runs to the next `requestAnimationFrame`, so it includes the
wait for that frame. The Mac app's agent was building and running on the same machine
(load 2.6–4.6), so every row was run twice, A/B, interleaved with the B076 tree (e7b5eaa: B076
web with the B077 core, its own JS, the same wasm). Numbers are work p95 ms for run 1 / run 2.

| Interaction (budget) | session | B077 work p50 / p95 | frame p95 |
|---|---|---:|---:|
| Context menu opens, word (core menu + word selected + drawn) | seed / bench | 1.2 / 4.1–5.3 · 3.5 / 4.3–5.0 | 16.0 / 16.5 |
| Context menu opens, tab | seed / bench | 0.9 / 1.1–3.8 · 2.3 / 3.0–3.3 | 16.4 / 16.4 |
| Context menu opens, background | seed / bench | 0.8 / 1.8–3.5 · 1.9 / 2.5–3.0 | 16.7 / 16.5 |
| Choose → effect: Compare in SBLGNT (`tab.open_edition` and the re-render) | seed / bench | 0.5 / 1.3–1.9 · 2.9 / 3.3–3.6 | 16.6 / 16.5 |
| Choose → effect: Search in all editions (palette open with scoped rows) | seed / bench | 2.1 / 3.0–3.5 · 2.5 / 2.9–3.1 | 15.9 / 15.7 |

**Trusted right-clicks** (WebDriver button 2 on "loved", the selection cleared between, 200
per session per run), from the `contextmenu` event's timestamp:

| | session | handler done (menu laid out) p50 / p95 | menu's next frame p50 / p95 | baseline: an empty right-button pointerdown's next frame p50 / p95 |
|---|---|---:|---:|---:|
| run 1 | seed / bench | 2.4 / 3.4 · 3.4 / 3.6 | 16.2 / 18.1 · 16.2 / 17.8 | 16.2 / 18.0 · 16.2 / 17.8 |
| run 2 | seed / bench | 3.3 / 3.6 · 3.0 / 3.7 | 16.2 / 17.8 · 16.2 / 17.8 | 16.2 / 17.8 · 16.2 / 17.8 |

The menu is laid out 3–4 ms after the event and painted in the first frame after it: the same
frame as a pointerdown with no work at all (the baseline column is identical). The ~16 ms is
where Chrome delivers WebDriver's right button within its frame, not the menu.

B072/B076 rows, B076 tree → B077 tree (work p95 ms, run 1 / run 2):

| Interaction | seed | bench |
|---|---:|---:|
| Palette keystroke | 2.7, 2.4 → 3.0, 3.3 | 3.5, 3.0 → 3.5, 3.9 |
| Trusted keystroke → palette rendered (handler) | 2.0, 1.3 → 1.4, 2.3 | 2.7, 2.5 → 2.2, 2.4 |
| Keystroke with the Text section | 2.1, 0.7 → 2.0, 2.1 | 1.8, 1.7 → 2.1, 1.1 |
| Tab switch | 3.6, 2.5 → 4.7, 5.1 | 5.6, 6.2 → 6.3, 2.7 |
| Column focus | 1.6, 1.2 → 1.4, 3.9 | 3.5, 4.5 → 3.9, 1.2 |
| Workspace switch | 4.5, 3.8 → 2.3, 4.6 | 5.0, 5.2 → 5.4, 3.0 |
| Tool change | 2.7, 3.1 → 1.1, 3.5 | 4.5, 5.2 → 4.9, 5.1 |
| B076 translation ↔ original | 3.4, 2.7 → 1.0, 3.7 | — |
| B076 translation → notes | 4.2, 3.3 → 1.4, 4.6 | — |
| B076 selection group | 2.9, 2.5 → 2.9, 3.4 | 2.8, 2.8 → 2.8, 3.0 |
| Open a chapter | 4.3, 5.0 → 4.8, 5.1 | 5.3, 7.3 → 6.1, 6.1 |

The differences between the two trees are as large as the differences between two runs of the
same tree; every row stays far under its budget. The only per-render addition is a check that
find matches are still attached; the palette adds a scope filter that is skipped when no scope
is set.

## Lanes (committed tree e8749a9; this file is documentation only)

| Command | Result |
|---|---|
| `nix develop .#core -c just ci` | exit 0. `Summary [   6.714s] 312 tests run: 312 passed, 0 skipped` |
| `nix develop .#web -c just test-web` | exit 0. `tests 9, pass 9, fail 0`; `conformance cases: 1594 passed, 0 failed (api 0.1.2)` |
| `nix develop .#web -c just test-wasm` | exit 0. `test result: ok. 8 passed; 0 failed` |
| `nix develop .#web -c just test-dom-reader` | exit 0. `DOM reader: 13/13 cases passed` |
| `nix develop .#web -c just test-shell-web` | exit 0. `Summary: Web shell: 42/42 cases passed` |

## Lockstep

Agreed with the Mac agent (b077mac) over messages: the title as a disabled header, sections
between separators with no labels drawn, disabled rows with the reason as a subtitle and
tooltip, keys shown (dimmed on disabled rows), submenu parents that only open, the word
selected on right-click, editions registered only for bsb / sblgnt / wlc (so BSB shows
"Compare in SBLGNT" and SBLGNT "Compare in BSB"), the chip labels "This edition · BSB",
"All editions", "Workspace", and the toasts: "Copied “loved”", "Copied John 3:16",
"Copied John 3:16 with its text", "Copied" / "Cut", "Opened John 3:17" / "No passage on the
clipboard", "Removed link to John 3:16", "4 matches in John 3" / "No matches in John 3",
"2 of 4 in John 3". Highlight in colour has no toast.

Open when this file was written (web's proposal, sent to b077mac; if they disagree it goes to
team-lead): (a) `copy.passage` keeps curly quotes, `“text” — John 3:16 (BSB)`, like Note on
it; the Mac proposed `text — John 3:16 (BSB)` without them. (b) Find in this chapter only
highlights in the column (no palette and no "This chapter" chip), because with the palette
open Esc and focus belong to the field; the Mac proposed palette rows for the chapter as well.
Chip placement (inside the field on web, left of it on the Mac) is platform layout.

## Deviations from the brief

- **⌘G / ⇧⌘G and Esc for Find in this chapter are host view keys**, bound only while matches
  are shown, like Esc closing the palette; they are not registry commands (the registry has no
  find-next). If the owner wants them in the palette, the core would add `find.next` /
  `find.previous`.
- **The scope chip filters the core's Text rows on the host** by the edition short name in the
  row title (`BSB · Jn 3:16`); the core's palette query has no scope argument. With the
  fixture text source this is exact; B071's pack source keeps the same titles.
- **Clipboard reading**: headless Chrome under WebDriver may not grant `clipboard-read`; paste
  and Paste passage then use the shell's own last copy. The cases check the shell's copy
  (`host.copied`) and read the system clipboard only as a cross-check (equal or unreadable).
- **Word targets are per `.t` text node**; a word split by markup (none in the fixture text)
  would be cut at the node edge.
- **The trusted right-click timing** is measured from the `contextmenu` event's timestamp; the
  browser delivers WebDriver's right button late in a frame, so a no-work pointerdown has the
  same next-frame time (the baseline column). The menu adds no frame.

## Decisions for the owner

1. Visible details proposed to the Mac host for lockstep (header row, separators only, icons
   on every row, disabled reason as a second line, submenu parents never run, toasts and
   clipboard formats). Confirm or change; both hosts follow.
2. The clipboard format of "Copy … with reference": `“text” — John 3:16 (BSB)`, the same as
   Note on it's quote. Another style (reference first, no edition, verse numbers) is a host
   formatting change.
3. Find in this chapter is case-insensitive substring matching, not whole words, and does not
   fold accents (κόσμον does not match κόσμος). Lemma search is the core's "Needs the lexicon
   import".
4. The menu's opening on a right-click selects the word and leaves it selected after Esc, as
   macOS does.

## Skipped

- No headed (`--headed`) speed run this time; headless only.
- `selection.study` keeps its B072 toast ("Word study is not built yet").
- Link passage from the note menu opens the palette as the well's button does (no new dialog).
