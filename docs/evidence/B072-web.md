# B072 evidence (web): the workspace shell in the browser

Recorded 30 October 2026 on the Mac mini (Apple M1, macOS 26.4.1, Chrome 156.0.8078.48) in the
worktree `openlogos-b072web`, branch `b072-web-shell` cut from `b070-core-shell` (70ce91e) with
`b070-core-shell` at dec4426 merged in (the core's reading, ink, width and scroll commands), by
a Claude Code subagent working under the lead's audit. Nothing was pushed or merged. No
simulator was touched. This branch does not change `crates/`; the merged dec4426 does. Only
fixture text appears on screen. The Mac half of B072 is a separate branch and has its own evidence.

## What was built

| Item (the B072 brief) | Where |
|---|---|
| Palette field: the only top bar, with the core's sectioned results, ↑/↓, Enter, ⌥Enter or Tab to show a whole section, Esc to return focus, a query on every keystroke | `apps/web/shell/shell.js` (MARK: The palette), `paletteQuery` with `platform: "web"` |
| Workspace switcher (`Study · 1/2`) and text menu (`Aa 19`: A− A A+, line spacing, theme, all core commands) | `shell.js` (MARK: Menus) |
| Numbered columns with 1 px rules (a drag ends in `column.resize`, a double-click is `column.equalize`); a tab strip (28) with the shown tab marked by weight and a 2 px accent underline, close × on hover or focus, `+`, horizontal overflow; an identity row (32) with a badge, the resource, the passage, the link word and a `…` menu | `shell.js` (MARK: Rendering), `shell.css` |
| Text: the B005 surface, capped at the core's measure (`layoutConstants()`), scroll position recorded with `tab.record_scroll` and restored from `scroll_anchor`, specs/21's placeholder when a passage is not installed | `apps/web/reader/surface.js` (moved out of reader.js), `shell.js` `buildColumn`, `placeholder` |
| Floating tool palette: grip (drag along the edge, double-click to collapse), six tools as inline SVG with name and key in the tooltip, `aria-pressed` plus a filled button, two overlapping swatches (the current one chooses with `tool.swatch`, the alternate one or X swaps), a width dot that steps 2/4/6 with `tool.width` | `shell.js` (MARK: The floating tool palette) |
| Highlight: dragging over text paints it at once through the core's anchors. Collect: a toast such as `Collected: Jn 3:16`. Pen, highlighter pen, eraser: a margin canvas over the text that draws with B039's brushes | `shell.js` (MARK: Highlights, Ink), `apps/web/ink/layer.js` (moved out of ink.js) |
| Every control dispatches a registered command through `ShellHandle.apply`; `handled: false` is carried out by the host. Text size, spacing and theme render from `session.reading`; width and swatches from `session.ink` | `run()` and `perform()` in `shell.js` |
| Keys come only from the registry (`shortcuts.web`, `bare_key`) | `apps/web/shell/keys.js` |
| `TextSource` (resource id and reference in; display metadata and core segments out) | `apps/web/shell/text-source.js` |
| Seed session, the same JSON as the Mac app (Study: BSB John 3:16 + WLC Gen 1:1 \| SBLGNT John 3:16 (group 1) \| `fixture:note:1` pinned as a tab, widths 0.4/0.35/0.25; Hebrew: WLC Gen 1:1), and the bench session (10 × 3 × 4) | `apps/web/shell/seed.js` |
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
| `cc789e7` | the first version of this file |
| `372e6fd` | merge of `b070-core-shell` at dec4426 |
| `853ddc0` | the shell on the core's reading, ink, width and scroll commands; seed and labels in step with the Mac app; tab rows `BSB · John 3:16`; screenshots retaken without stray ink (20 cases) |
| this commit | this file, updated |

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
`pen-and-eraser` · `placeholder-and-navigation` (Rom 8, then ⌘[) · `menus` (switcher, text menu as core commands and its `Aa 21` label, theme through the palette, pane menu) ·
`column-resize` (`column.resize` on drop, ⌘Z restores, double-click equalizes) · `scroll-anchor-recorded` (`tab.record_scroll`, restored after a rebuild) ·
`swatches-and-width` (`tool.swatch`, swap from the alternate well, width dot) · `well-clears-column-one` (1440 and 1024; collapsing gives the inset back) · `only-registered-commands`.

Headless and headed Chrome: **20/20** on the final tree.

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
| Palette keystroke: passage, commands, tabs (16 ms) | seed | 0.7 / 1.6 | 16.5 | 0.7 / 1.1 | 16.7 |
| | bench | 0.9 / 1.6 | 16.5 | 0.9 / 1.5 | 17.1 |
| Trusted keystroke → palette rendered (16 ms) | seed | 0.5 / 1.3 | 16.6 | 0.9 / 1.8 | 15.7 |
| | bench | 1.1 / 1.9 | 15.8 | 1.0 / 1.8 | 15.6 |
| Keystroke with the Text section, fixture provider (50 ms) | seed | 0.5 / 0.6 | 16.6 | 0.6 / 0.8 | 16.4 |
| | bench | 1.1 / 1.8 | 16.4 | 0.6 / 0.8 | 16.7 |
| Tab switch within a column (next frame) | seed | 1.6 / 1.9 | 16.6 | 1.6 / 1.9 | 16.2 |
| | bench | 3.2 / 5.0 | 15.6 | 1.5 / 1.8 | 16.8 |
| Column focus (next frame) | seed | 0.4 / 0.5 | 15.5 | 0.5 / 0.7 | 16.7 |
| | bench | 1.3 / 1.8 | 16.6 | 0.6 / 0.7 | 16.9 |
| Workspace switch to painted text (100 ms) | seed (2) | 3.0 / 4.6 | 16.6 | 1.7 / 2.1 | 17.3 |
| | bench (10 × 3 × 4) | 2.4 / 2.9 | 16.6 | 1.9 / 2.2 | 16.7 |
| Tool change (next frame) | seed | 1.5 / 2.0 | 16.5 | 0.7 / 0.8 | 17.1 |
| | bench | 0.9 / 1.0 | 16.6 | 0.8 / 0.9 | 16.6 |
| Open a chapter in a tab, warm (100 ms) | seed | 3.4 / 4.9 | 16.0 | 1.7 / 2.0 | 16.7 |
| | bench | 2.3 / 3.1 | 15.9 | 1.9 / 2.1 | 16.6 |

Every row is within its budget. The worst work p95 is 5.0 ms (tab switch, headless, bench),
about 30 % of one frame. Raw numbers, with max and n: `docs/evidence/B072/web-bench-headless.json`
and `web-bench-headed.json`, both on 853ddc0. Timer resolution is 0.1 ms because the page is not cross-origin
isolated. Cold launch is not measured here, and neither is the text section over the real
editions (B071).

## Screenshots (`docs/evidence/B072/`, 10 PNGs, 1.1 MB)

The evidence keeps ten web shots (the task's cap is 20 across both hosts):
- `web-light-1440`, `web-sepia-1440` and `web-dark-1024`: the seed session with one highlight
  on "eternal life" and no ink.
- `web-ink`: a pen stroke in the accent and a highlighter-pen stroke over "For God did not send".
- `web-palette-passage`, `web-palette-tabs` and `web-palette-text`: the palette with each section.
- `web-palette-commands-expanded-dark`: `>` with the Commands section expanded.
- `web-tool-palette`: the highlighter pen is current.
- `web-placeholder`: Rom 8 in group 1.

`just test-shell-web --shots docs/evidence/B072 --extra-shots artifacts/b072/shots` takes them
again. The second flag also writes the other five (`web-dark-1440`, `web-light-1024`,
`web-palette-commands`, `web-workspace-menu`, `web-text-menu`) outside the repository, for the
NAS archive.

## Lanes on the committed tree

| Command | Result |
|---|---|
| `nix develop .#core -c just ci` | exit 0. `Summary [   6.340s] 299 tests run: 299 passed, 0 skipped` |
| `nix develop .#web -c just test-web` | exit 0. `tests 7`, `pass 7`, `fail 0`; `conformance cases: 1174 passed, 0 failed (api 0.1.2)` |
| `nix develop .#web -c just test-wasm` | exit 0. `test result: ok. 6 passed; 0 failed` |
| `nix develop .#web -c just test-dom-reader` | exit 0. `DOM reader: 13/13 cases passed` |
| `nix develop .#web -c just test-ink-web` | exit 0. `Web ink: 6/6 palm cases passed, live palm case passed, export valid` |
| `nix develop .#web -c just test-shell-web --bench --samples 200` | exit 0. `Summary: Web shell: 20/20 cases passed; bench: headless Chrome, 200 samples per interaction` |
| `nix develop .#web -c just test-shell-web --headed --bench --samples 200` | exit 0. `Summary: Web shell: 20/20 cases passed; bench: headed Chrome, 200 samples per interaction` |

## Deviations from the brief and decisions to review

1. **Host view state.** Only the tool well's position and collapse, and a column-rule drag
   until it ends in `column.resize`, stay outside the session. Highlights and ink strokes
   live in host memory (B074 persists them) and ⌘Z does not undo them. Everything else that
   was host state in the first version is now a core command (dec4426).
2. **Colours.** The pen draws in the theme's accent, as the Mac app does. Highlight and the
   highlighter pen use the current swatch. The seed session (theme light, width 4) starts the wells on the token
   yellow and green, as the Mac app's does, because the core's `InkSettings` default is `#FFE066` / `#7FD1FF`,
   which are not token swatches. A colour that is not a token swatch still paints, at 30 %.
   The core's text-size steps run −3…5 but the tokens hold seven sizes (−2…4), so the shell
   clamps at both ends. One of the two should change.
3. **Tab rows in the palette** read `BSB · John 3:16` with `Study › column 1`. The core titles
   them `bsb · John 3:16`, so the host relabels them until the core's tab labels land; it also
   copies the reason texts (item 4) until then. The workspace row still says
   `1 columns, 1 tabs`.
4. **Disabled reasons.** The bindings return `{ kind, … }` but not the sentence, so
   `format.js` repeats the core's `thiserror` texts. A `reason_text` in the outcome would
   remove the copy.
5. **The measure cap** comes from `layoutConstants()` (640, and 900 from 28 pt up). The
   tokens file keeps the numbers, labelled as derived, and the test fails if they differ.
6. **Passages are written the way `formatHuman` writes them** (`John 3:16`, `Rom 8`, not
   `Romans 8`). Tab labels are the short name plus the chapter (`BSB Jn 3`), with SBL
   abbreviations from a table in `format.js`; the Mac app uses the same labels.
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
11. **Notes tab.** Column 3's note is a tab of kind `notes` with `content_id: "fixture:note:1"`
    (short name `Note`) and no reference. The notes dock (`view.notes_dock`) is a separate
    trailing panel showing the same note.
12. **Disk.** Free space on `/` fell from 28 GiB to 12 GiB over the session (Nix shells and
    `target/`), still above the 4 GiB stop line.

## Skipped or open

- No persistence (B074). The text section is the fixture provider (B071).
- Ink on touch: a finger always scrolls (B039's arbiter in stylus mode). Pen input was only
  tested with a mouse; no tablet was attached.
- Firefox and Safari were not run (the runner takes `WEBDRIVER_URL`, as the reader's does).
- Cold launch to text is not measured.
