# B076 evidence (web): the contextual tool well in the browser

Recorded 30 October 2026 on the Mac mini (Apple M1, Chrome 156) in the worktree
`openlogos-b076web`, branch `b076-web-well`. The branch was cut from `b076-core-well` (a8f7433)
and merged `b076-core-well` again at 8c01647 to get `tab.open_lexicon`, then at 3702fc5, where focus
stays on the clicked column. The work was done by a
Claude Code subagent. Nothing was pushed or merged, no simulator was touched, and this branch
does not change `crates/`. Only fixture text appears on screen. The Mac half of B076 is
`b076-mac-well` and has its own evidence.

Owner feedback: "The tool bar needs to be contextually useful for what it's looking at." The
core decides the well (specs/22 §4.1, docs/evidence/B076-core.md), and the web shell draws it.

## What works

| Item (the B076 brief) | Where |
|---|---|
| The well is drawn from `toolWell(context)` in core order: tools, then a divider and ink (two overlapping swatches and the width dot) when present, then a divider and the selection group when present. It is fetched again on every render (focus, tab, apply) and on `selectionchange` when the selection appears or goes away. `focus` (`tab` / `notes_dock`) and `has_selection` go to `apply`, `commands`, `paletteQuery` and `toolWell`. `setResourceLanguages` is called beside `setResourceTitles` | `apps/web/shell/shell.js` (`context`, `fetchWell`, `renderWell`, `selectionChanged`), `text-source.js` `languages()` |
| Groups are rebuilt only when their command ids change. A context change swaps buttons within one frame, with no transition. The well's top stays fixed across contexts and selections, and the selection group grows downward. In a window too short for the tallest well, the well moves up just enough to stay on screen | `renderWell`, `placeWell`, `shell.css` (`.well .group`) |
| Tooltip `Label (key)` (`Bold (⌘B)`, `Heading`), `Label — reason` when disabled. Disabled items are dimmed (`aria-disabled`); a click shows the reason as a toast. The active tool is a filled accent button with `aria-pressed` and `aria-label` "Tools, current: …" | `renderWell`, `pressWellItem` |
| Effective tool: `effective_tool` is marked active and drives the text (`data-tool`: ink canvas, highlight and collect drags, the Word study cursor). The stored tool stays stored: Pen → notes shows Type → back on Bible text shows Pen again. A bare key outside its context shows the core's words as a toast ("Eraser works on Bible text"), not a dialog | `render`, `run` |
| **Word study** (W, Greek and Hebrew): a click on a word sends the word as clicked (the `Intl.Segmenter` word under the caret) and its verse to `tab.open_lexicon`. That reuses the Lexicon tab in column N+1 or adds one there, or opens column N+1 when there is none. Focus stays on the clicked column, so the next word
can be clicked straight away. The tab shows the word, "Greek · John 3:16" (or "Hebrew · Gen 1:1") and "Lexicon entries arrive with the lexicon import." No glosses | `studyWordAt`, `lexiconPage`; `text-source.js` resource `lexicon` |
| **Notes**: the notes tab and the dock are editable rich text, a `contenteditable` over HTML kept in memory per note id (until B074). The tab and the dock showing the same note stay in step. Bold and italic (⌘B / ⌘I, only while the editor has focus), heading (toggles an `h3`), bullet list, and Link passage. Link passage opens the palette prefilled with "Link passage "; the core's row inserts a passage link at the caret the palette was opened from. Clicking the link runs `nav.navigate` in the last-focused reading column | `noteEditor`, `noteFormat`, `linkPassage`, `passageLink` |
| **Selection actions**: Highlight now and Collect behave as in B072. Note on it appends `“quote” — John 3:16` as a quote paragraph to the open note (the shown notes tab, else the dock, which it opens if closed); the reference is a passage link. Search this (⌘E) puts the selected words in the palette field and opens it. Copy reference copies `formatHuman` of the selection's verses and shows the toast "Copied John 3:16". Study keeps its B072 toast | `noteSelection`, `searchSelection`, `copyReference` |
| Tests: 7 new `just test-shell-web` cases (28 in all) | `apps/web/shell/test/run-chrome.mjs` |
| Speed rows for the well; screenshots with `--well-shots <dir>` | `bench()` in `shell.js`; `wellScreenshots()` in the runner |

### Icons (the core's names → web SVG; the same meaning as the Mac's SF Symbols)

Agreed with the Mac agent by message. Both sides proposed the same symbols, except for
`heading` and `study`; for those two, web asked the Mac to use the ones in this table. Check
B076-mac.md for what it ships.

| Core icon | Web inline SVG (20 × 20, stroked) | SF Symbol (Mac) |
|---|---|---|
| `select` | arrow pointer (filled) | `cursorarrow` |
| `word-study` | closed book with α on the cover | `character.book.closed` |
| `highlight` | marker with a baseline | `highlighter` |
| `pen` | pencil tip | `pencil.tip` |
| `highlighter-pen` | brush with a translucent blot | `paintbrush.pointed` |
| `eraser` | eraser | `eraser` |
| `collect` | arrow into a tray | `tray.and.arrow.down` |
| `type` | A with a text I-beam | `character.cursor.ibeam` |
| `bold` | **B** | `bold` |
| `italic` | *I* (serif) | `italic` |
| `heading` | large A + small A | `textformat.size` |
| `list` | three bullets and lines | `list.bullet` |
| `link-passage` | two chain links | `link` |
| `swatch`, `swatch-alternate` | the two overlapping colour wells (B072) | the same |
| `brush-width` | the width dot (B072) | the same |
| `highlight-now` | marker (as `highlight`, in the selection group) | `highlighter` |
| `note` | page with a plus | `note.text.badge.plus` |
| `search` | magnifying glass | `magnifyingglass` |
| `copy-reference` | two overlapping pages | `doc.on.doc` |
| `study` | book with text lines | `text.book.closed` |

### Commits

| Commit | Content |
|---|---|
| `4035f8a` | the well from `toolWell`, focus context, notes editing, selection actions, 6 cases |
| `a359a21` | merge of `b076-core-well` at 8c01647 (`tab.open_lexicon`) |
| `5da203d` | Word study → `tab.open_lexicon`, the Lexicon tab page, 1 case |
| `bf75de2` | speed rows for the well |
| `81364ee` | the well stays on screen in short windows (1 case), screenshot states, links don't wrap |
| `fc5be3a` | screenshots |
| `623e2d2` | this file, first version |
| `55b1faf` | merge of `b076-core-well` at 3702fc5 (focus stays on the clicked column) |
| `82a21fe` | `word-study-lexicon` follows it |
| `33972f7` | the lexicon screenshot is taken straight after the click, with no ⌥2; it is byte-identical, column 2 keeps the badge |
| this commit | this file, updated |

## Cases (`just test-shell-web`, WebDriver Actions: trusted keys and pointer)

The 21 B072 cases still pass; `registry-bindings` now expects 70 commands. New cases:

- `well-contexts`: the translation well (6 tools + ink), Greek (Word study second, Select and the well's top unmoved), notes (Type, Bold, Italic, Heading, List, Link passage, Collect; Type pressed; tooltips; the label), the notes dock's focus → notes, back in a column → translation; the well's top stays the same throughout.
- `effective-tool-restore`: Pen stays stored in notes while Type is in effect (`data-tool="type"`); E in notes → toast "Eraser works on Bible text"; Pen comes back on Greek; Word study falls back to Select on BSB and comes back on SBLGNT.
- `selection-group`: dragging "eternal life" adds the six actions in core order without moving the ink items above; Copy reference → "Copied John 3:16"; the group goes when the selection does.
- `selection-note-and-search`: Note on it appends `“eternal life” — John 3:16` (a passage link) to the notes tab; with that tab closed, the dock opens and gets the next quote; ⌘E puts "For God so loved" in the open palette.
- `notes-bold-heading-link`: ⌘B/⌘I in the note (`note.bold` dispatched, `handled: false`), Heading → `<h3>`, List → `<ul><li>`, Link passage → palette "Link passage " → `jn 3:17` → a link "John 3:17"; clicking it moves column 1 to John 3:17.
- `word-study-lexicon`: W on SBLGNT, a click on ἠγάπησεν → column 3 gets "Lexicon Jn 3" showing `ἠγάπησεν / Greek · John 3:16 / Lexicon entries arrive with the lexicon import.`, with focus and Word study still on column 2; κόσμον reuses that tab; in the Hebrew workspace אֱלֹהִ֑ים opens a new column 2, with focus staying on column 1 ("Hebrew · Gen 1:1"); a click on BSB with Word study stored does nothing.
- `well-fits-short-window`: at 1024 × 600 the tallest well (Greek + selection) stays inside the window.

## Screenshots (docs/evidence/B076, 6 files, 660 KB)

`just test-shell-web --case seed-session --well-shots docs/evidence/B076`, headless Chrome,
1440 × 900:

| File | State |
|---|---|
| `web-well-translation.png` | BSB focused: six tools, ink |
| `web-well-original-lexicon.png` | SBLGNT focused with Word study active; the Lexicon tab for ἠγάπησεν open in column 3 |
| `web-well-notes-editing.png` | the note being edited: "Love that gives: **grace**, see John 3:16" (a passage link); the notes well with Type active |
| `web-well-selection.png` | "eternal life" selected in BSB: tools, ink and the selection group |
| `web-well-selection-dark.png` | the same in the dark theme |
| `web-well-fallback-pen-in-notes.png` | Pen chosen on BSB, then the note focused: Type in effect, Pen still stored |

## Speed (specs/22 §7; budget 16 ms, next frame)

`just test-shell-web --case seed-session --bench --samples 200`, headless and `--headed`; 200
samples per row. Work and frame are defined as in B072-web.md: **work** is the synchronous
change plus a forced layout, **frame** runs to the next `requestAnimationFrame`, which
includes waiting for the frame. The B076 rows run on the seed session, which has a translation,
an original and a notes column. The bench session shows only translations, so it gets only the
selection row.

| Interaction | session | headless work p50 / p95 ms | headless frame p95 | headed work p50 / p95 ms | headed frame p95 |
|---|---|---:|---:|---:|---:|
| Column focus translation ↔ original (well changes) | seed | 1.7 / 3.3 | 16.3 | 2.3 / 3.3 | 17.0 |
| Translation → notes (well changes) | seed | 2.3 / 4.0 | 16.6 | 2.6 / 4.1 | 17.2 |
| Selection appears → selection group drawn | seed | 0.7 / 0.9 | 16.5 | 0.6 / 0.8 | 16.7 |
| | bench | 1.0 / 2.7 | 16.5 | 1.8 / 2.7 | 17.0 |

Each B076 row's work p95 is at most 4.1 ms against 16 ms. Headed frame p95 runs slightly over
16.7 ms (17.0–17.3 ms) on every row, B072's rows included. That is the visible window's vsync
wait, not work.

B072's rows again, B072 → B076 (work p95 ms):

| Interaction (budget) | session | headless B072 → B076 | headed B072 → B076 |
|---|---|---:|---:|
| Palette keystroke (16) | seed / bench | 2.3 → 1.3 / 3.3 → 1.3 | 2.0 → 1.2 / 2.1 → 1.2 |
| Trusted keystroke → palette rendered (16), handler p95 | seed / bench | 2.2 → 1.9 / 2.2 → 1.5 | 1.3 → 1.3 / 2.5 → 2.6 |
| Keystroke with the Text section (50) | seed / bench | 2.1 → 0.8 / 1.3 → 0.7 | 0.7 → 0.8 / 1.0 → 0.6 |
| Tab switch (next frame) | seed / bench | 4.5 → 3.6 / 5.1 → 2.9 | 1.9 → 5.0 / 2.3 → 2.8 |
| Column focus (next frame) | seed / bench | 1.4 → 2.7 / 5.1 → 1.9 | 0.8 → 1.3 / 3.3 → 3.6 |
| Workspace switch (100) | seed / bench | 5.1 → 2.3 / 5.3 → 4.8 | 1.9 → 1.7 / 6.7 → 6.5 |
| Tool change (next frame) | seed / bench | 2.1 → 2.5 / 5.6 → 3.0 | 0.8 → 2.7 / 4.3 → 4.4 |
| Open a chapter (100) | seed / bench | 5.0 → 3.5 / 6.2 → 5.7 | 2.3 → 2.0 / 7.1 → 6.8 |

There is no regression past noise. Seed column focus (1.4 → 2.7 ms headless) and tool change
(0.8 → 2.7 ms headed) now include building the well from the core and swapping its buttons.
The core's `tool_well` call takes 8 µs (B076-core), so the rest is the DOM. Every row is far
inside its budget. The raw JSON is in the scratchpad, not the repo.

## Lanes (committed tree 33972f7; this file is documentation only)

| Command | Result |
|---|---|
| `nix develop .#core -c just ci` | exit 0. `Summary [   7.686s] 306 tests run: 306 passed, 0 skipped` |
| `nix develop .#web -c just test-web` | exit 0. `tests 8, pass 8, fail 0`; `conformance cases: 1371 passed, 0 failed (api 0.1.2)` |
| `nix develop .#web -c just test-wasm` | exit 0. `test result: ok. 7 passed; 0 failed` |
| `nix develop .#web -c just test-dom-reader` | exit 0. `DOM reader: 13/13 cases passed` |
| `nix develop .#web -c just test-shell-web` | exit 0. `Summary: Web shell: 28/28 cases passed` |

## Deviations from the brief

1. **The well can move up in a short window.** The top stays fixed through context and
   selection changes. When the tallest well (Greek, ink, selection) would run past the bottom
   of the window, it moves up just enough to stay on screen; otherwise the actions would be cut
   off. At 1440 × 900 and 1024 × 768 it never moves.
2. **A selection counts only in reading text.** `has_selection` is true for a selection inside
   a Bible text, which has anchors and a reference. A selection inside a note does not add the
   selection group, because Note on it, Collect and Copy reference need a passage.
3. **The width dot** still steps up through the core's widths and wraps (B072 behaviour, the
   `swatches-and-width` case). The core's `tool.width` item, decision 6, is not turned into a
   widths menu.
4. **Search this** puts the words in the field unchanged (no `#` prefix), so the palette shows
   every section, the Text section included.

## Decisions for the owner

1. **Notes are rich text, not visible markdown** (on both hosts, by agreement): bold, italic,
   a heading line at 1.25× size, bullets, passage links in the accent colour, underlined, and
   quotes with a 2 px rule on the leading edge. The in-memory form is HTML until B074 picks a
   storage format.
2. **A passage link navigates the last-focused reading column** (column 1 by default) and
   focuses it. A plain click follows the link; it is not editable as text.
3. **Note on it uses the notes tab** when one is shown in the workspace, else the dock, which
   it opens if needed.
4. **Lexicon tab language** comes from the column where the word was clicked (`grc` → Greek,
   `he`/`hbo` → Hebrew). The core stores only the word and the passage.
5. **The selection group's Highlight now** reuses the Highlight icon. The group and the divider
   tell them apart, and so does the tooltip ("Highlight now (⌘⇧H)").
6. **⌘B / ⌘I** are taken only while a note editor has focus. Elsewhere the browser keeps them.

## Skipped

- Selection actions inside notes (see deviation 2).
- Persisting notes or the lexicon language (B074).
- Hover screenshots of tooltips: headless Chrome draws no native tooltips. The tooltip texts
  are asserted in `well-contexts`.
