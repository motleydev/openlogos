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
just lint-wasm    # clippy for wasm32, including the worker tests
```

`core-host/` is a Node package with no dependencies beyond the generated `pkg/`; its tests read
the module bytes with `fs` and pass them to `init()`, because Node's `fetch` does not load
`file:` URLs. The one `--target web` build serves both the Node host and the browser.

`just test-wasm` runs `scripts/wasm-test.sh`, which reads the installed Chrome's version, fetches
the matching chromedriver from Chrome for Testing once into `~/.cache/openlogos/chromedriver/`
and passes it to `wasm-pack test --headless --chrome`. wasm-pack's own chromedriver download did
not match the installed Chrome on the development Mac (docs/evidence/B061-wasm.md). Set
`CHROMEDRIVER` to use a driver you already have; `just test-wasm -- --nocapture` shows the
conformance case count.
