# apps/web

Thin TypeScript interface over semantic DOM text; the Rust core runs as WebAssembly in a worker
with OPFS persistence (docs/adr/0001-rust-core-native-ui.md). The reader itself does not exist
yet; what is here is the core's WebAssembly binding and the hosts that prove it (B061).

`nix develop .#web` provides Node.js 24 (LTS), `wasm-bindgen-cli`, `wasm-pack`, `binaryen`
(`wasm-opt`) and Rust with the `wasm32-unknown-unknown` target. Google Chrome is expected on the
machine for the browser test and is the one thing not provided by Nix.

## The binding

`crates/ffi/src/wasm.rs` is the wasm-bindgen surface, compiled only for `wasm32`. It exposes
`apiVersion()`, `parseReference()`, `formatOsis()`, `formatHuman()`, `mapVerse()`,
`parseTokenId()`, `formatTokenId()`, `selectionToAnchor()`, `anchorToUtf16()`, `reanchor()`,
`runConformance()` and `conformanceFixturesJson()`. Values cross as JS objects with the core's
serde field names (`segment_id`, `content_revision`), the same shapes as the fixture JSON, and a
failing call throws an `Error` with `kind` (the variant tag, e.g. `unknown_book`) and `details`.
Offsets a page gives or receives are UTF-16 code units, that is JS string indexes; anchors keep
Unicode scalar values inside. The generated `openlogos_ffi.d.ts` carries the TypeScript types.

## The DOM reader (B005)

`reader/` is the semantic-DOM reading surface spike: `index.html` and `reader.js`, no
framework and no build, served next to `core-host/pkg`. Three panes and a notes dock of fixture
text; the browser's own selection becomes core anchors through `selectionToAnchor` (verse
markers are not core text and are mapped out) and the ranges `anchorToUtf16` hands back are
painted with the CSS Custom Highlight API, or overlays where it is missing. `?case=<name>`
drives one of its cases, `?autorun=1` all of them. `reader/test/run-chrome.mjs` runs them in a
real browser through WebDriver (`just test-dom-reader`, headless Chrome by default;
`GECKODRIVER`/`FIREFOX` or `WEBDRIVER_URL` with `--browser firefox|safari` for the others) and
takes the evidence screenshots. Results and the OPFS/IndexedDB fallback notes: docs/evidence/B005.md.

## Run

Inside `nix develop .#web`:

```sh
just wasm-build   # wasm-pack --target web, then wasm-opt -Oz, into core-host/pkg (gitignored); prints sizes
just test-web     # builds, then runs the Node 24 test host in core-host with node:test
just test-wasm    # the same checks as wasm-bindgen tests in a dedicated worker in headless Chrome
just test-dom-reader  # the B005 DOM reader's selection cases in headless Chrome through WebDriver
just test-ink-web     # the B039 ink prototype's palm cases, latency runs and export in headless Chrome
just test-ink-web --actions --headed   # plus WebDriver Actions pen and mouse drags (trusted input) in a visible Chrome
just ink-excalidraw-build && just test-ink-web --page excalidraw   # the Excalidraw comparison page, bundled offline
just lint-wasm    # clippy for wasm32, including the worker tests
just preview-web  # serve apps/web on 127.0.0.1:8780 for hand review (index, reader, ink); --tailnet shares it
just feedback "note"  # drag a region of the screen; shot and note land in ~/OpenLogosFeedback/<utc>/
```

## The ink prototype (B039)

`ink/` is the margin-ink spike: `index.html` and `ink.js`, no framework, a canvas beside a
reading pane. Pointer events become strokes outlined by the core's `inkOutline` (the
perfect-freehand 1.2.3 port in crates/ink, through the wasm module) and filled once each with
the non-zero rule at the stroke's opacity (the highlighter rule); the document is the
ink-document record and `inkDocumentJson` validates it on export. Pen, highlighter, stroke
eraser with tombstones, undo/redo, stylus and finger modes, and the palm arbiter of
`crates/ink/src/touch.rs` mirrored in JS (same case names). `?autorun=1` runs the palm cases,
a live-page palm case, synthesized pen and touch latency runs and the export; `?record=1`
records real pointer events for a hand run (`openlogosInk.handReport()`). `ink/excalidraw/`
mounts the pinned `@excalidraw/excalidraw` 0.18.1 offline (esbuild from nixpkgs, `dist/`
gitignored) and drives its freedraw tool with the same loop. Results: docs/evidence/B039.md.

`core-host/` is a Node package with no dependencies beyond the generated `pkg/`; its tests read
the module bytes with `fs` and pass them to `init()`, because Node's `fetch` does not load
`file:` URLs. The one `--target web` build serves both the Node host and the browser.

`just test-wasm` runs `scripts/wasm-test.sh`, which reads the installed Chrome's version, fetches
the matching chromedriver from Chrome for Testing once into `~/.cache/openlogos/chromedriver/`
and passes it to `wasm-pack test --headless --chrome`. wasm-pack's own chromedriver download did
not match the installed Chrome on the development Mac (docs/evidence/B061-wasm.md). Set
`CHROMEDRIVER` to use a driver you already have; `just test-wasm -- --nocapture` shows the
conformance case count.
