# B061 evidence (wave 2, item 3) — WebAssembly build, browser worker and Node test

Recorded 28 October 2026 on the owner's Mac (aarch64-darwin). Work was done in the git worktree
`openlogos-b061-wasm` (branch `b061-wasm`) cut from `b061-core-bindings` at f4e4843, which holds
the core API of docs/evidence/B061-core.md. This note covers acceptance item 3 of B061:
"WebAssembly build runs the same conformance fixtures in a browser worker and Node test". Item 2
(UniFFI Swift and Kotlin bindings) is a sibling branch and is not claimed here.

## Versions

| Tool | Version | From |
|---|---|---|
| Rust | 1.99.1 (rustc 58cf8f9c3 2026-10-13), target `wasm32-unknown-unknown` | flake.nix `rustVersion` |
| `wasm-bindgen` crate | =0.2.121 (js-sys =0.3.98, serde-wasm-bindgen =0.6.5, wasm-bindgen-test =0.3.71) | crates/ffi/Cargo.toml |
| `wasm-bindgen-cli` | 0.2.121 | `nix develop .#web -c wasm-bindgen --version` |
| wasm-pack | 0.15.0 | `.#web` shell |
| wasm-opt | binaryen version 129 | `.#web` shell |
| Node.js / npm | v24.21.0 / 11.19.0 | `.#web` shell (`nodejs_24`) |
| Google Chrome | 156.0.8078.48 (installed on the Mac, outside Nix) | `/Applications/Google Chrome.app` |
| chromedriver | 156.0.8078.48 from Chrome for Testing, fetched by `scripts/wasm-test.sh` | `~/.cache/openlogos/chromedriver/` |

`scripts/wasm-bindings.sh` refuses to build when `wasm-bindgen --version` and `cargo pkgid
wasm-bindgen` disagree, so the crate and CLI cannot drift apart silently.

## What was built

| Commit | Content |
|---|---|
| `00b26b1` | `crates/ffi/src/wasm.rs` (`#[cfg(target_arch = "wasm32")]`): `apiVersion`, `parseReference`, `formatOsis`, `formatHuman`, `mapVerse` over the Psalm 9/10 fixture table, `parseTokenId`, `formatTokenId`, `selectionToAnchor`, `anchorToUtf16`, `reanchor`, `runConformance`, `conformanceFixturesJson`, plus a TypeScript section for the object shapes. Dependencies pinned exactly under `[target.'cfg(target_arch = "wasm32")']`; `crate-type = ["cdylib", "staticlib", "rlib"]`. Additive `VersificationFixture::verse_map()` in the conformance crate. `-p openlogos-ffi` added to the wasm check in `justfile` and `flake.nix`; the validator's Python added to the `.#web` shell so `just ci` runs there. |
| `ce86e45` | `scripts/wasm-bindings.sh` (wasm-pack `--target web` into gitignored `apps/web/core-host/pkg/`, then `wasm-opt -Oz`, sizes printed), `scripts/wasm-test.sh` (headless Chrome with a matching chromedriver), `apps/web/core-host` Node test host (`node:test`, no dependencies, committed `package-lock.json`), `crates/ffi/tests/worker.rs` (`wasm_bindgen_test_configure!(run_in_dedicated_worker)`), just group `6 · web` (`wasm-build`, `lint-wasm`, `test-wasm`, `test-web`), `apps/web/README.md`. |

### How values cross the boundary

`serde-wasm-bindgen` with `Serializer::json_compatible()` turns the core's serde derives into JS
objects directly: field names are the core's (`segment_id`, `content_revision`), `None` is
`null`, and nothing is serialized to a JSON string and parsed again. The shapes are therefore
identical to the fixture JSON and to what the UniFFI side sees, and a fixture segment can be
handed back in after `parseTokenId()` on its token ids (token ids are objects in the core, strings
in fixtures and the pipeline). A failing call throws a JS `Error` whose `message` is the
`thiserror` text, `kind` the serde tag of the variant (`unknown_book`, `splits_surrogate_pair`,
`invalid_argument`, ...) and `details` the whole error object. Offsets given to or returned by
`selectionToAnchor` and `anchorToUtf16` are UTF-16 code units, that is JS string indexes;
anchors keep scalar values inside.

## Commands and results

All commands ran inside `nix develop .#web` from the worktree with `set -o pipefail` and the
exit code checked. The three runs in the table were made from a clean `pkg/` at `ce86e45`
(the tree had the same content as the commit; the evidence commit followed). Durations include
rebuilds caused by the sibling agent competing for the machine; warm figures are in brackets.

| Command | Result | Duration |
|---|---|---|
| `just ci` | exit 0. rustfmt, clippy (host, `--all-targets`, `-D warnings`), nixfmt, `just --fmt`, shellcheck (now four scripts), actionlint clean; validator `PASS: 12 schema/example validations; 14 agent records; 69 acyclic tasks; source references, budgets and fixture hashes; 9 negative tests, including secret leakage, ink coordinates and research evidence scope.`; nextest `Summary [ 34.707s] 62 tests run: 62 passed, 0 skipped`; doc tests 0; `check-wasm` for core, text, conformance and ffi | 1 m 42 s (32 s warm) |
| `just lint-wasm` (`cargo clippy --target wasm32-unknown-unknown -p openlogos-ffi --all-targets -- -D warnings`) | `Finished`, no warnings | 0.5 s warm, 29 s cold |
| `just test-web` (= `wasm-build` then `npm test` in `apps/web/core-host`) | exit 0. `wasm-pack 0.15.0, wasm-bindgen 0.2.121, wasm-opt version 129`; `openlogos_ffi_bg.wasm: 465944 bytes before wasm-opt -Oz (149809 gzipped), 427774 after (158788 gzipped)`; Node: 6 tests, 6 pass, 0 fail, `conformance cases: 283 passed, 0 failed (api 0.1.0)` | 27.8 s (first release build of the wasm target 55 s; Node suite itself 0.2–2.2 s) |
| `just test-wasm -- --nocapture` (`wasm-pack test --headless --chrome --chromedriver ... crates/ffi`) | exit 0. `Running headless tests in Chrome`, `running 5 tests`, all `ok`, `conformance cases: 283 passed, 0 failed (api 0.1.0)`, `test result: ok. 5 passed; 0 failed` | 33 s (15 s warm; the tests themselves 0.14–0.20 s) |

Verbatim last lines of `just ci`:

```text
cargo check --target wasm32-unknown-unknown -p openlogos-core -p openlogos-text -p openlogos-conformance -p openlogos-ffi
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.22s
```

Verbatim Node host output (`npm test`, after the build lines):

```text
conformance cases: 283 passed, 0 failed (api 0.1.0)
✔ apiVersion() is the core's API_VERSION (1082.658833ms)
✔ parseReference() parses a compound citation (32.323083ms)
✔ a core error is a JS Error with kind and details (4.634625ms)
✔ mapVerse() answers the Psalm 9/10 fixture cases (37.481917ms)
✔ a UTF-16 selection round-trips through an anchor on the Hebrew fixture (19.080708ms)
✔ runConformance() passes every fixture case (280.214833ms)
ℹ tests 6
ℹ suites 0
ℹ pass 6
ℹ fail 0
ℹ cancelled 0
ℹ skipped 0
ℹ todo 0
ℹ duration_ms 2239.332041
```

Verbatim browser worker output (`just test-wasm -- --nocapture`, after the cargo lines):

```text
Chrome 156.0.8078.48 driver: /Users/jessemartin/.cache/openlogos/chromedriver/156.0.8078.48/chromedriver-mac-arm64/chromedriver
     Running tests/worker.rs (.../target/wasm32-unknown-unknown/debug/deps/worker-a1e9bbc79b686485.wasm)
Running headless tests in Chrome on `http://127.0.0.1:52149/`
Loading Wasm module...
using deprecated parameters for the initialization function; pass a single object instead
running 5 tests
Invoking test: utf16_selection_round_trips_through_an_anchor
test utf16_selection_round_trips_through_an_anchor ... ok
Invoking test: runs_in_a_dedicated_worker
test runs_in_a_dedicated_worker ... ok
Invoking test: parses_a_compound_citation
test parses_a_compound_citation ... ok
Invoking test: conformance_suite_passes
conformance cases: 283 passed, 0 failed (api 0.1.0)
test conformance_suite_passes ... ok
Invoking test: api_version_is_the_core_version
test api_version_is_the_core_version ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 filtered out; finished in 0.17s
```

The deprecation line comes from wasm-bindgen-test-runner's own harness calling the module's
`init()`, not from code in this repository. `runs_in_a_dedicated_worker` asserts that
`DedicatedWorkerGlobalScope` exists in the global scope, so the worker configuration is checked
rather than assumed.

### What the two hosts check

Both hosts run against the one `--target web` build (Node reads the module bytes with `fs` and
passes them to `init()`, since Node's `fetch` has no `file:` support):

1. `apiVersion()` equals the core's `API_VERSION` (Node reads the constant out of
   `crates/core/src/lib.rs`; the worker compares with `openlogos_core::API_VERSION`) and equals
   `runConformance().api_version`.
2. `parseReference("Jn 3:16, 18; Rom 8:28-30")` gives three ranges, `John.3.16;John.3.18;Rom.8.28-Rom.8.30`
   in OSIS form and `John 3:16, 18; Rom 8:28–30` in human form; `Hezekiah 3:16` throws
   `kind: "unknown_book"` with message ``unknown book `Hezekiah` ``.
3. For every selection of the `hebrew-cantillation-genesis-1` fixture (seven, including the maqqef
   alone, the sof pasuq and a range that starts inside a cluster): the fixture's UTF-16 span goes
   through `selectionToAnchor` to one anchor whose scalar range, quoted text and token ids equal
   the fixture's, `text.slice(start, end)` in JS equals the quote, `anchorToUtf16` returns the
   original span, and (Node) `reanchor` onto the same revision is `exact`. Node also checks that an
   index inside the `👍🏽` surrogate pair of the bidi fixture throws `splits_surrogate_pair`.
4. `runConformance().failed === 0` with every failing case listed if any, and the case count
   printed: 283.

Node additionally runs every case of the Psalm 9/10 fixture through `mapVerse` (status, targets
and path equal the fixture) and checks that a range (`Ps 9:1-2`) throws `not_one_reference`.

## Module size

`wasm-pack build --target web --release` with the workspace release profile (`opt-level = "z"`,
fat LTO, one codegen unit, stripped), then `wasm-opt -Oz`:

| Stage | Raw | gzip -9 |
|---|---|---|
| wasm-pack output | 465,944 bytes | 149,809 bytes |
| after `wasm-opt -Oz` | 427,774 bytes (−8.2 %) | 158,788 bytes (+6.0 %) |

The optimized module is smaller on disk but compresses worse, so over a gzip-serving host it
is 9 KB larger. specs/01-product.md sets time budgets (cold open ≤2 s) rather than a byte
budget; both figures are recorded so the choice can be revisited when the reader exists. The
generated JS glue is 30 KB, the `.d.ts` 7 KB. The module carries the five conformance fixtures
(`include_str!`), serde_json, serde-wasm-bindgen and unicode-segmentation's tables.

`wasm-opt` had to be told the features Rust enables by default on `wasm32-unknown-unknown`
(`--enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext
--enable-mutable-globals --enable-reference-types --enable-multivalue`): wasm-bindgen drops
rustc's `target_features` section, and without the flags the validator rejected `memory.copy`
and `i64.trunc_sat_f64_s`.

## Headless Chrome and chromedriver

`wasm-pack test --headless --chrome crates/ffi` on its own failed here: wasm-pack 0.15.0 printed
`chromedriver version: 156.0.8078.73`, but the binary it installed into
`~/Library/Caches/.wasm-pack/chromedriver-54d2ddac2d082548/` reports
`ChromeDriver 148.0.7778.167`, which cannot drive Chrome 156; every session ended with
`Error: http status: 404`. `scripts/wasm-test.sh` reads the installed Chrome's version, fetches
the same version of chromedriver from Chrome for Testing
(`https://storage.googleapis.com/chrome-for-testing-public/<version>/mac-arm64/chromedriver-mac-arm64.zip`,
also `mac-x64` and `linux64`) once into `~/.cache/openlogos/chromedriver/<version>/` and passes
it with `--chromedriver`. With it the worker tests ran and passed as shown above. `CHROMEDRIVER`
overrides the lookup. The browser run was not reported as passed until it actually ran.

## Disk

Free space on the Mac was 7.4 GiB at the start of this task and 3.6 GiB at the end, with the
UniFFI sibling building on the same disk (`openlogos-b061-uniffi/target` 1.7 GiB,
`openlogos-b061-wasm/target` 877 MiB of which 397 MiB is the wasm32 debug build for the worker
tests). It did not go under 3 GiB; `just check` (Nix flake check, which vendors and rebuilds every
dependency in the sandbox) was not run for that reason and is listed under skipped.

## Decisions to review

- `serde-wasm-bindgen` over JSON strings: one serialization, same serde derives as UniFFI and the
  fixtures, snake_case keys preserved rather than camel-cased so fixtures round-trip unchanged.
- Errors are `Error` instances with `kind` and `details` rather than plain objects, so
  `instanceof Error`, stack traces and `assert.throws` work; `details` keeps the variant's
  fields (a field named `name` must not overwrite `Error.name`, hence not `Object.assign`).
- `selectionToAnchor(paragraph, utf16Start, utf16End)` takes one `Paragraph` object
  (`edition_id`, `content_revision`, `joiner`, `segments`) instead of bare segments, because
  `split_selection` needs the edition, revision and joiner too.
- `mapVerse` accepts exactly one reference; a chapter-only reference is allowed and maps as
  `unmapped`, as the core and the fixture's "chapter-only reference is unmapped" case define;
  ranges and lists throw `not_one_reference`.
- Token ids cross as objects (the core's serde form); `parseTokenId`/`formatTokenId` convert the
  `edition:BBCCCVVVWWW[s]` string form. Alternative: strings everywhere at the boundary.
- `VersificationFixture::verse_map()` is the one additive change outside `crates/ffi`, so the
  binding does not duplicate the row expansion; `run_all()` is unchanged.
- `openlogos-conformance` is a `path` dependency in `crates/ffi/Cargo.toml` because the root
  `Cargo.toml` has no workspace entry for it; adding one would touch a shared file.
- The TypeScript interfaces in `wasm.rs` are hand-written and must follow the serde shapes; the
  tests exercise the values but nothing checks the `.d.ts` against them yet.
- `crate-type = ["cdylib", "staticlib", "rlib"]` as agreed with the UniFFI branch.
- Python in the `.#web` shell: without it `just ci` fails at `just validate` there.

## Skipped or uncertain

- `just check` (Nix flake check) not run: disk. The `wasm` check in `flake.nix` now includes the
  ffi crate; `cargo check --locked` for it passed through `just ci`.
- No Linux run; `scripts/wasm-test.sh` has a `linux64` branch and looks for `google-chrome`,
  `google-chrome-stable`, `chromium` or `chromium-browser`, untested.
- No `--target nodejs` build; the web build is consumed by both hosts by design.
- The browser worker test does not run `reanchor` or `mapVerse`; both are covered in Node and
  inside `runConformance()` which the worker runs.
- `wasm-opt`'s effect on gzipped size is negative here; not investigated further.
