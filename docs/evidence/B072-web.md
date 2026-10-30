# B072 evidence (web): the workspace shell in the browser

Recorded 30 October 2026 on the Mac mini (Apple M1, macOS 26.4.1, Chrome 156.0.8078.48) in the
worktree `openlogos-b072web`, branch `b072-web-shell` cut from `b070-core-shell` (70ce91e), by
a Claude Code subagent working under the lead's audit. Nothing was pushed or merged. No
simulator was touched, and `crates/` is unchanged. Only fixture text appears on screen. The
Mac half of B072 is a separate branch and has its own evidence.

## What was built

| Item (the B072 brief) | Where |
|---|---|
| Palette field: the only top bar, with the core's sectioned results, ↑/↓, Enter, ⌥Enter or Tab to show a whole section, Esc to return focus, a query on every keystroke | `apps/web/shell/shell.js` (MARK: The palette), `paletteQuery` with `platform: "web"` |
| Workspace switcher (`Study · 1/2`) and text menu (A− A A+, line spacing, theme) | `shell.js` (MARK: Menus) |
| Numbered columns with 1 px rules that can be dragged; a tab strip (28) with the shown tab marked by weight and a 2 px accent underline, close × on hover or focus, `+`, horizontal overflow; an identity row (32) with a badge, the resource, the passage, the link word and a `…` menu | `shell.js` (MARK: Rendering), `shell.css` |
| Text: the B005 surface, capped at the measure, scroll position per tab, specs/21's placeholder when a passage is not installed | `apps/web/reader/surface.js` (moved out of reader.js), `shell.js` `buildColumn`, `placeholder` |
| Floating tool palette: grip (drag along the edge, double-click to collapse), six tools as inline SVG with name and key in the tooltip, `aria-pressed` plus a filled button, two overlapping swatches (X swaps them), a width dot | `shell.js` (MARK: The floating tool palette) |
| Highlight: dragging over text paints it at once through the core's anchors. Collect: a toast such as `Collected: Jn 3:16`. Pen, highlighter pen, eraser: a margin canvas over the text that draws with B039's brushes | `shell.js` (MARK: Highlights, Ink), `apps/web/ink/layer.js` (moved out of ink.js) |
| Every control dispatches a registered command through `ShellHandle.apply`; `handled: false` is carried out by the host | `run()` and `perform()` in `shell.js` |
| Keys come only from the registry (`shortcuts.web`, `bare_key`) | `apps/web/shell/keys.js` |
| `TextSource` (resource id and reference in; display metadata and core segments out) | `apps/web/shell/text-source.js` |
| Seed session (Study: BSB Jn 3 + WLC Gen 1 \| SBLGNT Jn 3 (group 1) \| the note pinned as a tab; Hebrew: WLC Gen 1) and the bench session (10 × 3 × 4) | `apps/web/shell/seed.js` |
| Tokens | `design/tokens/shell.json`, as CSS custom properties in `shell.css`; the runner fails if the two disagree |
| `just test-shell-web` | `scripts/shell-web-test.sh`, `apps/web/shell/test/run-chrome.mjs` |
| `just preview-web` links the shell | `apps/web/index.html` lists **Workspace shell** first, at `/shell/` |

No framework and no bundler. The shell is plain ES modules over the wasm-pack output, like
`reader` and `ink`. The DOM is updated per column: a column is rebuilt only when its tabs,
shown tab or passage change. Focus, tool and theme changes only set classes. Hidden tabs and
other workspaces keep no DOM.

### Commits

| Commit | Content |
|---|---|
| `b5c876c` | `reader/surface.js` and `ink/layer.js` as shared modules (both spikes unchanged in behaviour) |
| `31e8bb3` | the shell, the tokens, `just test-shell-web` (17 cases) |
| `3fd3dca` | speed runs (synthetic and trusted keystrokes), tab rows in the chrome's words, scroll anchors, screenshots |
| `fd8ca7c` | Hebrew at the token reading size, bench JSON |
| this commit | this file |

## Cases (`just test-shell-web`, WebDriver Actions)

Key and pointer events are trusted events sent through chromedriver, so the registry's
bindings are what gets tested. The page is reloaded before each case.

`seed-session` (shape, labels, badges, link words, switcher, placeholder text, chrome metrics against the tokens) ·
`tokens-match-design` (three themes, swatches and their `::highlight` rules, radii, spacing, fonts, reading size and leading, measure) ·
`registry-bindings` (one binding per web shortcut, 49 commands, tool keys read from the registry) ·
`palette-passage-follows-group` (⌘K, `jn 3:16`, Enter: group 1 follows, the pinned note stays) ·
`palette-sections-and-expand` (shortcut glyphs, Show all N, Tab expands, disabled rows with reasons, `@` and `#` sections, tab rows, Esc returns focus) ·
`slash-focuses-palette` · `column-tab-workspace-keys` (⌥2, ⌥], ⌥[, ⌥⇧], ⌥⇧[) ·
`tab-commands-and-undo` (⌥T, ⌥⇧3, ⌘Z twice, ⌘⇧Z, ⌥W, the × of a hidden tab, `+`) ·
`tool-keys-and-well` (V H P M E C, X, click, grip double-click both ways, grip drag, no bare keys in the field) ·
`highlight-drag` (anchor and painted range, repainted after a re-render) · `collect-drag` · `hebrew-highlight` (an RTL drag) ·
`pen-and-eraser` · `placeholder-and-navigation` (Rom 8, then ⌘[) · `menus` (switcher, text menu, theme through the palette, pane menu) ·
`column-resize` · `only-registered-commands`.

Headless and headed Chrome: **17/17** on the final tree.

## Speed (specs/22 §7)

Measured with `just test-shell-web --bench --samples 200` (headless) and `--headed --bench`
(a visible Chrome window), 200 samples per row. Each sample starts as a task at a random point
in a frame, the way an input event arrives.

- **work** is the synchronous part, measured with `performance.now()` around the action plus a
  forced layout: the core call, `handle.session()`, the DOM update, style and layout.
- **frame** runs from the start of the action to the `requestAnimationFrame` callback of the
  frame that paints it. It includes waiting for that frame, so at 60 Hz its p95 sits near
  16.7 ms however small the work is. The budget means "lands in the next frame", and work far
  below 16 ms is what guarantees that.

"Trusted keystroke" rows are 210 real key presses typed through WebDriver into the field. They
run from the keydown's `timeStamp` to the end of the handler (results rendered and laid out),
and to the next frame.

`seed` is the launch session (2 workspaces, 5 tabs). `bench` is 10 workspaces × 3 columns × 4
tabs (120 tabs).

| Interaction (budget) | session | headless work p50 / p95 ms | headless frame p95 | headed work p50 / p95 ms | headed frame p95 |
|---|---|---:|---:|---:|---:|
| Palette keystroke: passage, commands, tabs (16 ms) | seed | 0.7 / 1.0 | 16.5 | 0.6 / 0.9 | 16.6 |
| | bench | 0.8 / 1.1 | 15.8 | 0.8 / 1.1 | 16.6 |
| Trusted keystroke → palette rendered (16 ms) | seed | 0.4 / 1.1 | 15.6 | 0.5 / 1.1 | 14.7 |
| | bench | 0.8 / 1.4 | 16.1 | 0.8 / 1.4 | 15.4 |
| Keystroke with the Text section, fixture provider (50 ms) | seed | 0.6 / 0.7 | 15.4 | 0.5 / 0.6 | 16.6 |
| | bench | 0.6 / 0.8 | 16.5 | 0.6 / 0.8 | 16.7 |
| Tab switch within a column (next frame) | seed | 1.7 / 2.2 | 16.5 | 1.5 / 2.0 | 16.6 |
| | bench | 1.8 / 2.2 | 16.0 | 1.6 / 2.0 | 16.8 |
| Column focus (next frame) | seed | 0.4 / 0.6 | 16.5 | 0.4 / 0.4 | 16.6 |
| | bench | 0.7 / 0.8 | 16.5 | 0.6 / 0.9 | 16.6 |
| Workspace switch to painted text (100 ms) | seed (2) | 2.3 / 2.9 | 16.5 | 1.6 / 2.0 | 16.6 |
| | bench (10 × 3 × 4) | 2.6 / 3.0 | 16.6 | 1.9 / 2.2 | 16.6 |
| Tool change (next frame) | seed | 0.6 / 0.8 | 16.6 | 0.6 / 0.7 | 16.7 |
| | bench | 0.9 / 1.8 | 16.5 | 0.8 / 1.0 | 16.6 |
| Open a chapter in a tab, warm (100 ms) | seed | 2.6 / 4.8 | 16.0 | 1.5 / 2.0 | 16.6 |
| | bench | 2.4 / 3.4 | 16.5 | 1.9 / 2.1 | 16.6 |

Every row is within its budget. The worst work p95 is 4.8 ms (open a chapter, headless, seed),
30 % of one frame. Raw numbers, with max and n: `docs/evidence/B072/web-bench-headless.json`
(tree 3fd3dca) and `web-bench-headed.json` (fd8ca7c). Between those two trees only the CSS of
the Hebrew size changed. Timer resolution is 0.1 ms because the page is not cross-origin
isolated. Cold launch is not measured here, and neither is the text section over the real
editions (B071).

## Screenshots (`docs/evidence/B072/`, 14 PNGs, 1.5 MB)

`web-light-1440`, `web-sepia-1440`, `web-dark-1440`, `web-light-1024` and `web-dark-1024` show
the seed session with a highlight on "eternal life" and one pen stroke.
`web-palette-passage`, `-commands`, `-tabs` and `-text` show the palette with each section, and
`web-palette-commands-expanded-dark` shows `>` with the Commands section expanded.
`web-tool-palette` (highlighter pen current), `web-workspace-menu`, `web-text-menu` and
`web-placeholder` (Rom 8 in group 1) complete the set. They are regenerated by
`just test-shell-web --shots docs/evidence/B072`.

## Lanes on the committed tree

| Command | Result |
|---|---|
| `nix develop .#core -c just ci` | exit 0. `PASS: 33 schema/example validations; …; 122 negative tests …`; `Summary [  13.322s] 299 tests run: 299 passed, 0 skipped` |
| `nix develop .#web -c just test-web` | exit 0. `tests 7`, `pass 7`, `fail 0`; `conformance cases: 1118 passed, 0 failed (api 0.1.2)` |
| `nix develop .#web -c just test-wasm` | exit 0. `test result: ok. 6 passed; 0 failed` |
| `nix develop .#web -c just test-dom-reader` | exit 0. `DOM reader: 13/13 cases passed` |
| `nix develop .#web -c just test-ink-web` | exit 0. `Web ink: 6/6 palm cases passed, live palm case passed, export valid` |
| `nix develop .#web -c just test-shell-web --bench --samples 200` | exit 0. `Summary: Web shell: 17/17 cases passed; bench: headless Chrome, 200 samples per interaction` |
| `nix develop .#web -c just test-shell-web --headed --bench --samples 200` | exit 0. `Summary: Web shell: 17/17 cases passed; bench: headed Chrome, 200 samples per interaction` |

## Deviations from the brief and decisions to review

1. **Host state with no registered command.** The registry has no command for the following,
   so the host keeps them as view state, outside the session and undo: column widths after a
   drag, scroll position (a tab's stored `scroll_anchor` is read on first show but not written
   back), the tool well's position, line spacing, pen width (the width dot only displays it)
   and choosing a swatch colour (only X swaps). Highlights and ink strokes live in host memory
   only (no annotation store yet) and ⌘Z does not undo them. All but highlights and ink want
   core commands (`column.resize`, `view.line_spacing`, `tool.width`, `tool.swatch`, a scroll
   anchor setter) so the superset rule holds exactly. The line-spacing segmented control is the
   one visible control without a command.
2. **Pen colour.** The pen draws in the theme's text colour. Highlight and the highlighter pen
   use the current swatch. The swatches are highlight colours, and a yellow pen line would not
   read. The owner should confirm.
3. **Tab rows in the palette** show the strip's label (`BSB Jn 3`) and `Study › column 1`. The
   core titles them `bsb · John 3` (the raw resource id) with `Study · column 1`. Two core
   nits for whoever next touches `palette.rs`: resource display names, and the workspace row's
   `1 columns, 1 tabs`.
4. **Disabled reasons.** The bindings return `{ kind, … }` but not the sentence, so
   `format.js` repeats the core's `thiserror` texts. A `reason_text` in the outcome would
   remove the copy.
5. **The measure cap** comes from the tokens file, which copies the core's
   `MEASURE_CAP_STANDARD_PT` and `MEASURE_CAP_ACCESSIBILITY_PT` (640, and 900 from 28 pt up).
   The bindings do not expose it.
6. **Passages are written the way `formatHuman` writes them** (`John 3`, `Rom 8`, not
   `Romans 8`). Tab labels use SBL abbreviations (`Jn 3`) from a table in `format.js`.
7. **Rename workspace** from the switcher opens the palette prefilled with `Rename workspace `,
   so the name is typed as the command's argument. Enter on any command row that is disabled
   only for a missing argument does the same.
8. **Focus mode** hides the other columns and the tool well. The palette field stays (specs/22
   §3: always visible).
9. **Keys are matched by `KeyboardEvent.code`**, so ⌥2 works even though macOS types `™`. This
   assumes a US layout, as the registry's names do.
10. **Shared modules.** B005's paragraph rendering and anchor round trip moved to
    `reader/surface.js`, and B039's brushes, compositing and palm arbiter moved to
    `ink/layer.js`. `reader.js` and `ink.js` import them, and their lanes pass as before.
11. **Notes tab.** Column 3's note is a tab of kind `notes` with `content_id: "notes"` and no
    reference. The notes dock (`view.notes_dock`) is a separate trailing panel showing the same
    note.
12. **Disk.** Free space on `/` fell from 28 GiB to 14 GiB over the session (Nix shells and
    `target/`), still above the 4 GiB stop line.

## Skipped or open

- No persistence (B074). The text section is the fixture provider (B071).
- Ink on touch: a finger always scrolls (B039's arbiter in stylus mode). Pen input was only
  tested with a mouse; no tablet was attached.
- Firefox and Safari were not run (the runner takes `WEBDRIVER_URL`, as the reader's does).
- Cold launch to text is not measured.
