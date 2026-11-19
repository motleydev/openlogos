# B071 evidence (web): the preview pack and text search in the browser

Recorded 6–8 November 2026 on the Mac mini (Apple M1, Chrome 156, headless) in the worktree
`openlogos-b071web`, branch `b071-web` (the core branch `b071-preview-pack` plus `web-end-room`,
PR #37). A Claude Code subagent did the work; nothing was pushed or merged. The Mac half is
`b071-mac`. The core part is docs/evidence/B071.md, and its "What the host phase must do" is the
list followed here.

**No corpus byte is in the repository.** The screenshots in `B071-web/` show only the made-up
pack the tests write (`apps/web/shell/test/synthetic-pack.mjs`) and the conformance fixture
verses. The shots of the real editions, for the owner to look at, are outside the repository
(see "Screenshots").

## What was built

| Item | Where |
|---|---|
| **The real editions.** At launch each `packs/<id>/manifest.json` goes into the `ReadingHandle` over the placeholder of the same id (`addPackManifest`). A stream asks for a chapter's chunk when it needs it: it fetches the chunk, the core checks it with `supplyChunk` (SHA-256), and then the stream opens. Neighbours and three chapters past the window either side are fetched ahead of the reader. A bundle is read by `Range`, and a server that ignores Range is handled by cutting the bytes here. The host has no format of its own | `apps/web/shell/pack.js`, `reading.js` `open` / `want` / `prefetch` |
| **Opening waits for its chunks.** What is on screen stays until they arrive, and a newer jump supersedes an older one still waiting. An empty stream says "Loading…" after 200 ms. A chunk that fails (HTTP error, hash mismatch) shows "<Book C> could not be loaded. Try again", and is not asked for again for 5 s | `reading.js` `showWaiting`, `showFailure` |
| **First paint has text.** Before the first render, the shown tabs' chapters and their neighbours are fetched | `shell.js` `preloadShown` |
| **`just preview-web` serves the pack** from outside the repository at `/preview-pack/` (`OPENLOGOS_PREVIEW_PACK`, default `~/.cache/openlogos/preview-pack`). The pack is sent with `no-cache` (revalidate) and the app with `no-store`. The script says whether a pack was found, and rebuilds a module older than text search | `scripts/preview.sh` |
| **No pack.** Every edition stays the core's placeholder, unchanged: each block says "Placeholder text — this edition is not installed" (the Mac's words), and the palette's Text section is the fixture provider. `?pack=off` forces this, `?pack=<url>` names another pack | `pack.js` `packBase` |
| **Text search in a worker.** `search-worker.js` holds a second instance of the module, fetches `search/<id>.sqlite` of each installed edition once (streamed, with progress) and calls `useEditionSearch`. The page's core answers passages, commands and tabs within the keystroke. The worker's Text group is put into that result when it answers, and an answer for an overtaken query is dropped. The worker runs only the newest query it holds. Search starts after the first paint | `search-worker.js`, `search.js`, `shell.js` `queryPalette` / `textAnswered` |
| **Late text rows, as on the Mac** (lead's lockstep ruling): other rows paint first; the previous keystroke's text rows stay until the new answer, and clear at once for `>` and `@`; Enter waits for the current answer (at most 1 s, so a dead worker cannot swallow the key). A row chosen above the Text section stays chosen | `textAnswered`, `runWaitingEnter` |
| **Search not ready** shows under a Text heading: "Text search is loading… 42%", and "Text search could not load." when it failed (lead's ruling; the Mac shows only the failure line, in the same words). While an answer is on its way, "Nothing matches" is not shown | `search.js` `statusLine`, `renderPalette` |
| **A row's disabled state comes from the real session.** The worker's session has one column, so the page asks its own core (`ShellHandle.disabledText`, wasm only) for each text row. `#love @9` reads "No column 9" as on the Mac | `textAnswered` |
| **A hit opens what was hit** (core, lead's ruling, to be flagged to the owner). A text row's `nav.navigate` carries `resource_id`, with the reference in the hit's own numbering. If the focused tab already shows that edition, it navigates. Otherwise a new pinned, unlinked tab of that edition opens in the focused column (or `@N` / `@new`) and the shown tab is left alone | `crates/workspaces/src/commands.rs`, `palette.rs` |
| **About this edition** (core command `view.about_edition`, lead's ruling): a row in the column's … menu, and the palette lists it. A small panel under the … button shows the edition's name, `content_revision` and `Edition.attribution` as the pack has it, with its addresses as links. On a placeholder the panel shows the placeholder line. Nothing is added under chapters. It is disabled off a reading tab ("About this edition works on a reading tab") | `shell.js` `openAbout`, `fillAbout`; `shell.css` `.menu.about` |
| **Editions name their resources.** Short name, name, direction and language come from the manifest (WLC is `hbo`, `rtl`). The fixture licence line ("Fixture text. …") is no longer put under pack text | `text-source.js` `useEditions` |
| **Followers beside a verse the edition does not print.** A follower's verse is shown at the verse that holds it (bridge), else the last verse of its chapter numbered at or below it, else the chapter's first. A follower whose reference lies past the edition's last verse (SBLGNT Rom 16:25) is rebuilt there rather than in the not-in-edition state; only a typed verse (the tab's mark equals its reference) gets that state, in the core's words. A follower into a chapter its edition lacks stays where it is (b071mac does the same) | `reading.js` `holding`, `follow`; `shell.js` `buildStream` |
| **Re-anchoring on new revisions.** A stored highlight whose `content_revision` differs from the block's is placed with the core's `reanchor` against the fixture text of that revision (`bsb-2024`, `sblgnt-2010`, `oshb-wlc-4.20`). `exact` and `moved` are painted where they land. `ambiguous`, `orphaned`, and a revision this host has no text for are not painted, never guessed. Stored highlights are not rewritten, and the counts are in the snapshot. Ink is pinned to segments and has no revision | `shell.js` `reanchored` |
| **Lazy token conversion.** A block's core paragraph (thousands of `parseTokenId` calls in a real chapter) is built when a selection first needs it, not when the block is loaded | `reading.js` `load` |

## Core commits (for both hosts)

| Commit | What | Mac |
|---|---|---|
| `74dfcc0` | `view.about_edition` registered (`handled_by_core` false), disabled off a reading tab; `shell-commands.json` lists the id | sent to b071mac to cherry-pick |
| `8b6209c` | Text rows' `nav.navigate` carries `resource_id`; the core opens the hit's edition as above. The two text rows in `shell-commands.json` gain the field; no other expected result changed | sent |
| `78fc3e0` | `ShellHandle.disabledText(invocation, context)`, **wasm only** (`crates/ffi/src/wasm.rs`); no UniFFI change | not needed |

`crates/ffi` changed (`78fc3e0`), so every host lane must pass before merge.

## Cases (`just test-shell-web`)

The run from the committed tree gave `Web shell: 73/73 cases passed` with the real pack present: 61 from before, plus 12 new. Without a pack, the 3 `pack-real-*` cases print `{"skipped": true, ...}` and the Summary names them as SKIPPED.

| Case | Checks |
|---|---|
| `pack-none-is-the-placeholder` | no pack: no editions, no search worker, the placeholder notice, fixture text results |
| `pack-fixture-chunks-through-the-core` | the committed fixture packs: three editions installed, no placeholder notice, fixture verse from the pack, no fixture licence line; WLC's bundle by `Range` and by a server that ignores Range |
| `pack-search-states` | "Text search is loading…" (not "Nothing matches"), nothing for `>`, "Text search could not load." for a pack with no search packs |
| `pack-follower-beside-an-unprinted-verse` | made-up pack: SBLGNT at 16:24 puts the BSB at 16:23; the BSB at 16:25 puts the SBLGNT at 16:24 and it is rebuilt there after a workspace switch; a typed `Rom 16:25` gets the core's state; a typed absent `Matt 17:21` lands on 17:20, unmarked (the core's rule: the next printed verse of the typed range, else the nearest earlier), the follower too |
| `pack-wlc-order-and-direction` | `rtl`, `hbo`, header "WLC · Mal 3", Malachi 3 runs into Psalms (`tanakh_bhs`) |
| `pack-highlight-across-a-new-revision` | `bsb-2024` highlights on the new revision: 3:16 exact, 3:17 moved with 7 new scalars before it, an unknown revision not painted; stored as they were |
| `pack-about-this-edition` | the … menu row, the panel's text and link, Escape, the palette lists it, the command opens it, the placeholder's panel |
| `pack-chunk-failures-say-so` | a missing chunk and a wrong hash each show the failure line with Try again; the page lives on and reads elsewhere |
| `pack-slow-chunks-no-jump` | chunks held back 120 ms each: 160 scroll steps down and 120 up with the text under the eye moving ≤ 2 px, ≤ 9 blocks, and the newer of two waiting jumps wins |
| `pack-real-editions-and-search` | the real pack: three editions, search ready (17.8 MB), rows for English, Greek and Hebrew, a hit opened on the reading line ±2 px |
| `pack-real-a-hit-opens-what-was-hit` | a Hebrew hit on a BSB tab opens a WLC tab at Gen 1:1 and the BSB stays; `@9` disabled in the core's words, `@2` enabled; Enter pressed straight after typing runs the current answer's first row, which differs from the previous one's |
| `pack-real-awkward-places` | Rom 16 both ways, WLC Mal 3 → Ps 1 and no chapter after 2 Chr 36, WLC Ps 3:1 a title, each edition's attribution has its licence words |

`registry-bindings` now counts 86 commands, one more for `view.about_edition`.

## Measurements (specs/22 §7, with a repaint, p95)

`just test-shell-web --pack-bench --samples 200`, real pack served from localhost by the test
server, headless Chrome 156, 1440×900. **The machine was shared**: the Mac agent and two import
agents were building. Load averages: 2.3 at the start and 8.4 at the end of the main run (8
November 16:37–16:39 for its last part); 4.3–6.6 for the launch run (18:58). An earlier main run
at load 2.4–2.7 gave the figures in brackets.

A sample ends at the frame after the text is in the DOM, plus the task after that frame. The
keystroke rows end when the *current* keystroke's text rows are painted, merge and disabled check
included.

| Interaction | Samples | p50 ms | p95 ms | max ms | Budget | |
|---|---|---|---|---|---|---|
| Keystroke → text results painted, trusted keys at 70 ms a key (from the keydown's timestamp) | 221 | 15.6 | **25.3** (40.0) | 56.5 (82.9) | ≤ 50 | pass |
| Keystroke → text results painted, script-driven, every prefix of the 62 bench queries | 473 | 14.2 | **24.6** (27.0) | 59.9 | ≤ 50 | pass |
| of which the page's own handler per keystroke (main thread) | 474 | 1.2 | 2.1 | 18.6 | | |
| of which keystroke → worker's answer rendered | 473 | 3.2 | 18.4 | 56.9 | | |
| Long tasks (> 50 ms) on the main thread while typing | | | | | | 0 in all three runs |
| Open a chapter, warm (chunks in the core), across the canon, with followers | 200 | 22.2 | **34.1** (31.8) | 178.4 (41.4) | ≤ 100 | pass |
| Open a chapter on first visit (chunks fetched from localhost) | 200 | 30.0 | 51.5 (33.1) | 98.8 | (warm budget) | |
| Workspace switch to first painted text, 10 workspaces of 3 × 4 | 200 | 28.9 | **40.4** (38.6) | 76.7 | ≤ 100 | pass |
| Cold launch → restored workspace's text painted (navigation start; module, manifests, store, chunks) | 200 | 54.8 | **71.4** (134 in the loaded run) | 845.4 | ≤ 1000 | pass |
| Restore: store read → restored text painted, chunk fetches included | 200 | 15.2 | **17.9** | 24.9 | ≤ 100 | pass |
| Continuous scroll, 90 px a frame, Isa 1 → 28 (chunks fetched while scrolling) | 600 frames | 16.7 | 16.8 | 16.8 | frame | 0 frames over 25 ms, ≤ 5 blocks, ≤ 491 DOM nodes |
| Linked follow during that scroll | 472 | 1.5 | 3.1 | 6.9 | | |

One of the 474 script-driven keystrokes never got its answer painted within 3 s, in both runs.
I did not find why; the run counts it as `unanswered` rather than as a sample. The warm-open
maximum of 178 ms is one sample in the loaded run.

## First load and sizes

A browser with its cache cleared, the page loaded, until text is painted and search is ready.
The servers here send no compression.

| | Bytes on the wire | Note |
|---|---|---|
| Module `openlogos_ffi_bg.wasm` | 4,154,861 | 1,324,650 gzip -9 (B071.md); fetched by the page, compiled again in the worker |
| Three manifests | 630,462 | |
| Chunks for the restored workspace (22 chapters: 3 shown tabs, neighbours, read-ahead) | 1,326,180 | |
| Search packs (`search/*.sqlite`) | 17,838,080 | 7,143,375 gzip -9 (B071.md) |

| | Text painted | Search ready (from its start) |
|---|---|---|
| Localhost | 87 ms | 115 ms (module 32, fetch 25, attach 7) |
| Chrome's network emulation, 50 Mbit/s, 20 ms | **1,092 ms** | 237 ms, **not throttled**: the emulation did not reach the worker's fetches |

On that emulated link the first-ever load paints at **1.09 s**, over the 1 s that the budget sets
for a cold launch. That budget is for a launch, which is 71 ms above; but a first visit over a
network misses it, almost all of it the 4.15 MB uncompressed module and 0.63 MB of manifests.
Search over the same link would need about 2.9 s for 17.8 MB uncompressed, or about 1.1 s
gzipped (arithmetic, not measured), after the text is already on screen. Serving with
compression is the obvious step and is not done in `just preview-web`.

**The worker's cost.** The module is compiled twice (page and worker; about 14–32 ms in the
worker). Linear memory is 4.6 MB on the page and 33.2 MB in the worker, which holds the three
SQLite databases (17.8 MB) in memory. The core keeps every supplied chunk parsed for the session
(no eviction API); reading the whole BSB would hold about 42 MB of chunk JSON's content in the
page's core.

## Screenshots

In `B071-web/` (7 files, 1.3 MB; made-up pack and fixture verses only):

`web-pack-search-loading.png`, `web-pack-search-could-not-load.png`,
`web-pack-about-this-edition.png`, `web-pack-follower-beside-absent-verse.png`,
`web-pack-typed-past-the-end.png`, `web-pack-chunk-could-not-load.png`,
`web-pack-wlc-malachi-into-psalms.png`.

The made-up verses begin with their own number, so a marker and the text both show it ("16 16
made-up …"). That is test data, not the reader.

The real editions, outside the repository for the owner to look at (session scratchpad,
`realshots/`): John 3 BSB with linked SBLGNT; text search in English, Greek and Hebrew; About on
the SBLGNT; WLC Psalm 3 and 4; WLC Malachi into Psalms; Romans 16:24 followed; Psalm 119.

## Seen on screen (no text decision changed)

- WLC Psalm headings are English ("PSALM 3") over Hebrew text: the core's `chapter_label` rule
  from B078, already listed as open in B071.md.
- WLC Psalm titles (`superscription`) are italic with their `org` verse number, as decision 18
  says; the BSB titles at verse 0 have no number.
- BSB poetry reads as one run per verse (decision 4), as expected.
- Nothing else looked wrong in the places checked: John 3, Romans 16, Psalms 3, 4 and 119,
  Malachi 3–4, Gen 1. Qere, sigla and the Romans doxology were not judged by eye.

## Lanes

| Lane | Result |
|---|---|
| `just test-shell-web`, committed tree, real pack present | `Web shell: 73/73 cases passed`, exit 0 |
| `apps/web/core-host` `npm test` (Node host) | exit 0, 11 of 11; conformance 1,844 passed, 0 failed |
| `just lint-wasm` | exit 0 |
| core and conformance tests (`cargo nextest -p openlogos-workspaces -p openlogos-conformance`) | 84 passed, exit 0 |
| `just ci` | **not run**: see below |
| `just test-wasm`, `just test-swift`, `just test-kotlin` | not run here (test-swift is b071mac's) |

`just ci` builds and lints the whole workspace natively. This worktree had no native build tree
(its `target` is 960 MB, wasm and two crates), free disk was 6.6–7.1 GiB with two import agents
building, and the brief says to stop below 4 GiB and not to start new build trees. So it was left
for the lead's run.

## Not verified

- Firefox and Safari: only Chrome was run.
- Search over a throttled network (the emulation did not reach the worker), and any network
  with compression.
- Search has no case without the real pack, because the made-up pack has no search packs. The
  `pack-real-*` cases cover it and skip visibly in CI.
- The one unanswered keystroke above.
- iPad.
