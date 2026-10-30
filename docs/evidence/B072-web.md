# B072 evidence (web): the workspace shell in the browser

Recorded 30 October 2026 on the Mac mini (Apple M1, macOS 26.4.1, Chrome 156.0.8078.48) in the
worktree `openlogos-b072web`, branch `b072-web-shell` cut from `b070-core-shell` (70ce91e) with
`b070-core-shell` merged in at dec4426 (reading, ink, width and scroll commands) and a7d41e5
(reason texts, resource titles, `formatShort`, token-aligned sizes and swatches, argument
expansion rows) and 45fe931 (`ShellHandle.view()` with chapter labels, concrete keys on
expanded rows), by
a Claude Code subagent working under the lead's audit. Nothing was pushed or merged. No
simulator was touched. This branch does not change `crates/`; the merged core commits do. Only
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
| `7a5d55e`, `95f73d8` | palette dropdown width token (770), width dot on the core's widths, ten evidence shots; collapsed well as a 22 × 22 grip |
| `52cfedb` | merge of `b070-core-shell` at a7d41e5 |
| `ef9afc4` | the core's `disabled_text` / `reason_text`, `setResourceTitles` rows, `formatShort` labels, expansion rows; seed sets only `reading.theme`; four palette shots retaken (21 cases) |
| `8d875fc` | evidence after a7d41e5 (audited; PR #24) |
| `a62bcf5` | merge of `b070-core-shell` at 45fe931 |
| `4df5c49` | tab strip labels from `view()` (`resource_title` + `chapter_label`); no `{n}` placeholder in key glyphs; two palette shots retaken |
| this commit | this file, updated |

## Cases (`just test-shell-web`, WebDriver Actions)

Key and pointer events are trusted events sent through chromedriver, so the registry's
bindings are what gets tested. The page is reloaded before each case.

`seed-session` (shape, labels, badges, link words, switcher, placeholder text, chrome metrics against the tokens) ·
`tokens-match-design` (three themes, swatches and their `::highlight` rules, radii, spacing, fonts, reading size and leading, measure) ·
`registry-bindings` (one binding per web shortcut, 49 commands, tool keys read from the registry) ·
`palette-passage-follows-group` (⌘K, `jn 3:16`, Enter: group 1 follows, the pinned note stays) ·
`palette-sections-and-expand` (shortcut glyphs, Show all N, Tab expands, disabled rows with the core's reasons, `@` and `#` sections, the core's tab rows `BSB · Jn 3:16`, argument expansion rows with concrete keys (`⌥⇧2`), no `{n}` placeholder, the dropdown width, Esc returns focus) ·
`reason-text-from-core` (⌘Z with nothing to undo toasts the core's `reason_text`) ·
`slash-focuses-palette` · `column-tab-workspace-keys` (⌥2, ⌥], ⌥[, ⌥⇧], ⌥⇧[) ·
`tab-commands-and-undo` (⌥T, ⌥⇧3, ⌘Z twice, ⌘⇧Z, ⌥W, the × of a hidden tab, `+`) ·
`tool-keys-and-well` (V H P M E C, X, click, grip double-click both ways, grip drag, no bare keys in the field) ·
`highlight-drag` (anchor and painted range, repainted after a re-render) · `collect-drag` · `hebrew-highlight` (an RTL drag) ·
`pen-and-eraser` · `placeholder-and-navigation` (Rom 8, then ⌘[) · `menus` (switcher, text menu as core commands and its `Aa 21` label, theme through the palette, pane menu) ·
`column-resize` (`column.resize` on drop, ⌘Z restores, double-click equalizes) · `scroll-anchor-recorded` (`tab.record_scroll`, restored after a rebuild) ·
`swatches-and-width` (`tool.swatch`, swap from the alternate well, width dot) · `well-clears-column-one` (1440 and 1024; collapsing gives the inset back) · `only-registered-commands`.

Headless and headed Chrome: **21/21** on the final tree.

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
| Palette keystroke: passage, commands, tabs (16 ms) | seed | 1.2 / 2.3 | 16.1 | 0.7 / 2.0 | 16.6 |
| | bench | 0.9 / 3.3 | 16.7 | 0.9 / 2.1 | 16.6 |
| Trusted keystroke → palette rendered (16 ms) | seed | 0.8 / 2.2 | 15.9 | 0.9 / 1.3 | 14.7 |
| | bench | 1.0 / 2.2 | 15.9 | 1.0 / 2.5 | 15.7 |
| Keystroke with the Text section, fixture provider (50 ms) | seed | 1.3 / 2.1 | 16.4 | 0.6 / 0.7 | 16.7 |
| | bench | 0.6 / 1.3 | 16.6 | 0.5 / 1.0 | 16.6 |
| Tab switch within a column (next frame) | seed | 3.4 / 4.5 | 16.1 | 1.6 / 1.9 | 16.6 |
| | bench | 3.5 / 5.1 | 16.5 | 2.0 / 2.3 | 16.7 |
| Column focus (next frame) | seed | 1.1 / 1.4 | 16.6 | 0.5 / 0.8 | 16.6 |
| | bench | 3.0 / 5.1 | 16.3 | 1.1 / 3.3 | 16.9 |
| Workspace switch to painted text (100 ms) | seed (2) | 3.8 / 5.1 | 16.5 | 1.7 / 1.9 | 16.6 |
| | bench (10 × 3 × 4) | 4.3 / 5.3 | 16.1 | 5.5 / 6.7 | 15.5 |
| Tool change (next frame) | seed | 1.6 / 2.1 | 16.5 | 0.6 / 0.8 | 16.6 |
| | bench | 4.0 / 5.6 | 16.3 | 3.1 / 4.3 | 16.7 |
| Open a chapter in a tab, warm (100 ms) | seed | 3.6 / 5.0 | 16.4 | 1.8 / 2.3 | 16.6 |
| | bench | 4.5 / 6.2 | 16.4 | 6.0 / 7.1 | 16.6 |

Every row is within its budget. The worst work p95 is 7.1 ms (open a chapter, headed, bench), against a 100 ms budget. The
worst next-frame rows are tool change at 5.6 ms and tab switch and column focus at 5.1 ms
(headless, bench), about a third of one frame. Each change now reads both `session()` and
`view()` (about 0.05 ms each for the fixture session in Node). The machine was shared with the
Mac agent's builds, so the spread between runs (1–3 ms) is load as much as code. Raw numbers,
with max and n: `docs/evidence/B072/web-bench-headless.json` and `web-bench-headed.json`, both
on 4df5c49. Timer resolution is 0.1 ms because the page is not cross-origin isolated.
Cold launch is not measured here, and neither is the text section over the real
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
again. After a7d41e5 only the four palette shots changed (the core's row labels and reasons),
and after 45fe931 only `web-palette-text` and `web-palette-commands-expanded-dark` did. Only
those were replaced; the others are byte-identical. The second flag also writes the other five (`web-dark-1440`, `web-light-1024`,
`web-palette-commands`, `web-workspace-menu`, `web-text-menu`) outside the repository, for the
NAS archive.

## Lanes on the committed tree

| Command | Result |
|---|---|
| `nix develop .#core -c just ci` | exit 0. `Summary [   6.819s] 302 tests run: 302 passed, 0 skipped` |
| `nix develop .#web -c just test-web` | exit 0. `tests 7`, `fail 0`; `conformance cases: 1220 passed, 0 failed (api 0.1.2)` |
| `nix develop .#web -c just test-wasm` | exit 0. `test result: ok. 6 passed; 0 failed` |
| `nix develop .#web -c just test-dom-reader` | exit 0. `DOM reader: 13/13 cases passed` |
| `nix develop .#web -c just test-ink-web` | exit 0. `Web ink: 6/6 palm cases passed, live palm case passed, export valid` |
| `nix develop .#web -c just test-shell-web --bench --samples 200` | exit 0. `Summary: Web shell: 21/21 cases passed; bench: headless Chrome, 200 samples per interaction` |
| `nix develop .#web -c just test-shell-web --headed --bench --samples 200` | exit 0. `Summary: Web shell: 21/21 cases passed; bench: headed Chrome, 200 samples per interaction` |

## Deviations from the brief and decisions to review

1. **Host view state.** Only the tool well's position and collapse, and a column-rule drag
   until it ends in `column.resize`, stay outside the session. Highlights and ink strokes
   live in host memory (B074 persists them) and ⌘Z does not undo them. Everything else that
   was host state in the first version is now a core command (dec4426).
2. **Colours and seed.** The pen draws in the theme's accent, as the Mac app does. Highlight
   and the highlighter pen use the current swatch. Swatches outside the four token colours
   still paint, at 30 %. The seed session sets only `reading.theme: "light"`, which is the
   tokens' default, where the core's is `"system"`. Everything else is the core's default,
   which since a7d41e5 matches the tokens (sizes −2…4, swatches #F5D90A / #4CC38A, width 4).
3. **Palette rows and reasons come from the core.** Tab rows (`BSB · Jn 3:16`,
   `Study · column 1`) use the titles the host registers with `setResourceTitles`. Disabled
   rows and menu items show `disabled_text`, and a refused command's toast shows
   `reason_text`. Commands with arguments expand into rows such as "Move tab to column 2" and
   "Move tab to new column". The host relabel and the copied reason sentences are gone.
4. **Scroll timing differs between the hosts.** Web records `tab.record_scroll` 150 ms after
   scrolling stops; the Mac records just before each command. In both, the session holds the
   anchor before anything reads it.
5. **The measure cap** comes from `layoutConstants()` (640, and 900 from 28 pt up). The
   tokens file keeps the numbers, labelled as derived, and the test fails if they differ.
6. **Passages.** By the lead's ruling, the identity row shows the full human form from
   `formatHuman` (`John 3:16`). The tab strip shows the core's `view()` label, `resource_title`
   plus `chapter_label` (`BSB Jn 3`, or `Note` with no passage). The collect toast uses
   `formatShort` (`Collected: Jn 3:16`). The Mac app does the same.
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
12. **Disk.** Free space on `/` fell from 28 GiB to 11 GiB over the session (Nix shells and
    `target/`), still above the 4 GiB stop line.

## Skipped or open

- No persistence (B074). The text section is the fixture provider (B071).
- Ink on touch: a finger always scrolls (B039's arbiter in stylus mode). Pen input was only
  tested with a mouse; no tablet was attached.
- Firefox and Safari were not run (the runner takes `WEBDRIVER_URL`, as the reader's does).
- Cold launch to text is not measured.
