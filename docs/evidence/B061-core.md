# B061 evidence (wave 1) — core domain API and conformance fixtures

Recorded 28 October 2026 on the owner's Mac (aarch64-darwin, the toolchain pinned by B002:
Rust 1.99.1, cargo-nextest 0.9.136, Nix 2.34.4). Work was done in the git worktree
`openlogos-b061` (branch `b061-core-bindings`) cut from `main` at 0e50e13. This wave covers
acceptance items 1 and 4 of B061 (reference parsing and anchor round trip through a versioned
API; conformance fixtures). Items 2 and 3 (UniFFI Swift/Kotlin bindings, WebAssembly build
with a browser worker and Node test) are the second wave and are not claimed here.

## What was built

| Commit | Content |
|---|---|
| `62bead6` | Workspace dependencies pinned exactly: serde 1.0.229 (derive), serde_json 1.0.154, thiserror 2.0.21, unicode-segmentation 1.13.3. `openlogos-conformance` added to the wasm check lists in `justfile` and `flake.nix`. |
| `3cbe6df` | `core::reference`: `BookId` (66 books, OSIS ids, USFM numbers), `Reference`, `ReferenceRange`, `ReferenceSet`, `ReviewFlag`, `AliasTable::english()`, `parse()`, canonical and human formatting. |
| `a42571d` | `core::versification`: `SchemeId`, `VerseMapKind`, `VerseMapEntry`, `VerseMap::map()` with pivot through `org`, `MappingStatus`, `MappingResult`. |
| `fa0cc84` | `core::anchor`: `TokenId`, `ScalarRange`, `TextQuoteSelector`, `Anchor`. `text::offsets`: `Utf16Range`, `GraphemeRange`, lossless conversions and `snap_to_graphemes`. |
| `61dd409` | `core::anchor::reanchor()`: stable token ids, then exact quote with context; `Exact`, `Moved`, `Ambiguous`, `Orphaned`. |
| `e642e40` | `core::anchor::split_selection()` for cross-verse selections; reanchoring keeps offsets inside and beyond tokens. |
| `ae25258` | `crates/conformance`: five JSON fixtures, `run_all()`, `fixtures()`, `fixtures_json()`, `ConformanceReport`. |
| `f77d629` | `openlogos_core::API_VERSION = "0.1.0"` with the bump rule; `api_version()` returns it. |

The module-by-module public surface is in `crates/README.md` ("Core API (B061)").

## Commands and results

All commands ran inside `nix develop .#core` from the worktree, with `set -o pipefail` and the
exit code checked; `just ci` was green at every commit listed above.

| Command | Result | Duration |
|---|---|---|
| `nix develop .#core -c just ci` at `f77d629` | exit 0. `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; nixfmt, `just --fmt --check`, shellcheck, actionlint clean; validator `PASS: 12 schema/example validations; 14 agent records; 69 acyclic tasks; source references, budgets and fixture hashes; 9 negative tests, including secret leakage, ink coordinates and research evidence scope.`; nextest `Summary [4.749s] 62 tests run: 62 passed, 0 skipped`; doc tests 0; wasm check `Finished dev profile` for core, text and conformance | 19.4 s warm |
| `cargo nextest run -p openlogos-conformance the_suite_passes --no-capture` | `conformance cases: 283 passed, 0 failed` | 1 s |
| `cargo check --target wasm32-unknown-unknown -p openlogos-core -p openlogos-text -p openlogos-conformance` (the `check-wasm` step of `just ci`) | `Checking openlogos-core`, `Checking openlogos-text`, `Checking openlogos-conformance`, `Finished dev profile [unoptimized + debuginfo] target(s) in 1.14s` | 1.1 s |

Verbatim last lines of `just ci`:

```text
cargo check --target wasm32-unknown-unknown -p openlogos-core -p openlogos-text -p openlogos-conformance
    Checking openlogos-core v0.1.0 (/Users/jessemartin/Development/Personal/ios-apps/openlogos-b061/crates/core)
    Checking openlogos-text v0.1.0 (/Users/jessemartin/Development/Personal/ios-apps/openlogos-b061/crates/text)
    Checking openlogos-conformance v0.1.0 (/Users/jessemartin/Development/Personal/ios-apps/openlogos-b061/crates/conformance)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.14s
```

Unit tests by crate (nextest): core 40, text 8, conformance 5, one placeholder test in each of
the nine other crates. The conformance crate's `the_suite_passes` test runs the 283 fixture
cases and fails on any mismatch, printing every failing case with expected and actual JSON.

Free space on the Mac went from 8.1 GiB before this task to 7.8 GiB after it (it dipped to
6.9 GiB during builds); nothing under 3 GiB was seen. `cargo clean` was not needed.

## Fixtures and their sources

| Fixture | Source and license | Expectations derived how |
|---|---|---|
| `hebrew-cantillation-genesis-1` | Genesis 1:1–2 from `openscriptures/morphhb` `wlc/Gen.xml` (Open Scriptures Hebrew Bible, WLC 4.20; text public domain, markup CC BY 4.0), fetched 28 October 2026 | Text reconstructed from the `<w>` and `<seg>` elements with the OSHB `/` morpheme divider removed; verse 1:2 carries two maqqefs. Python `unicodedata` confirmed NFC would change both verses, so the bytes are stored as they are. Scalar and UTF-16 offsets computed in Python; grapheme offsets by a small UAX #29 approximation (combining marks, emoji modifiers, ZWJ extend) and confirmed by unicode-segmentation at test time. |
| `cross-verse-selection-john-3` | John 3:16–17, Berean Standard Bible (public domain), from `bible.helloao.org/api/BSB/JHN/3.json`, fetched 28 October 2026; the footnote marker after "only" is dropped | Tokens are whitespace-separated words with punctuation attached; token ids `bsb:44003016NNN`. The paragraph selection starts at "but have eternal life." and ends after "His Son" in verse 17. |
| `bidi-isolation-note` | Written for this fixture (CC0); the Hebrew words are WLC Gen 1:1 words | U+2067 RLI, U+2068 FSI and U+2069 PDI around the Hebrew, and "👍🏽" (base plus skin-tone modifier) at the end; the fixture pins that isolates are one scalar, one UTF-16 unit and one cluster each and that the emoji is 2 scalars, 4 units, 1 cluster. |
| `psalm-9-10-numbering` | Numbering facts, CC0; each row carries its own source note | See the next section. |
| `reference-parsing-english` | Written for this fixture (CC0) | The syntaxes named in the task plus the ambiguous `Ph` and chapter-only `Ps 23` review flags, and eight inputs that must fail with a named error kind. |

## Psalm 9/10 table: what is asserted and what is not

Rows are stored from each scheme to `org`, as specs/19 prescribes, and expanded verse by verse
by the conformance crate. Scheme names follow Paratext usage (`org`, `eng`, `lxx`).

- `eng` Ps 9:0 = `org` Ps 9:1 and `eng` Ps 9:1–20 = `org` Ps 9:2–21: the Masoretic text numbers
  the superscription as verse 1, English editions do not number it, and Paratext's `eng` scheme
  numbers an unnumbered title as verse 0. Verified by comparing the WLC and BSB/KJV verse
  counts (21 versus 20). The exact line of Paratext's `eng.vrs` was not consulted.
- `eng` Ps 10:1–18 = `org` Ps 10:1–18: no superscription in the MT; counts agree.
- `lxx` Ps 9:1–21 = `org` Ps 9:1–21 and `lxx` Ps 9:22–39 = `org` Ps 10:1–18: Rahlfs numbers the
  superscription as 9:1 and joins Hebrew 9 and 10 into Greek 9 of 39 verses (21 + 18). The
  verse-by-verse alignment of Greek 9:22–39 with Hebrew 10:1–18 was not checked against a
  printed Rahlfs; the row says so. Nothing is asserted about Greek Psalm 10 (Hebrew 11) and
  mapping it is reported unmapped.
- Psalm 13:5–6: added as the nearest real `merge` case because Psalms 9 and 10 contain no
  partial or merged verse between these three schemes. Hebrew 13:6 covers English 13:5 and
  13:6 (compared in KJV and BSB against the WLC). `eng` 13:5 → `org` is reported `partial`,
  `org` 13:6 → `eng` is `exact` with two targets.
- `partial` and `absent` row kinds, and `ambiguous` results from contradicting rows, are
  exercised by synthetic rows in `core::versification`'s unit tests, not by this fixture.
- Everything outside the table is `unmapped`; the map never assumes identity.

## Decisions to review

- `Reference.verse` is `Option<u32>`: `None` is a chapter-only reference (`Ps 23`), flagged
  `ChapterOnly`. The task sketched `verse` as a plain number; a plain number cannot express
  "whole chapter" without a sentinel.
- `BookId::number()` is the USFM/Paratext book number (Matthew 41, Revelation 67) and is the
  `BB` field of token ids; `canonical_index()` is the 0-based position. Deuterocanonical books
  can be added without renumbering. Specs/19 does not say which numbering `BB` uses.
- `Jude 3` is verse 3 of the single chapter; `Jude.3` (dotted OSIS form) is chapter 3. `Jude`
  alone parses as the whole chapter with a `ChapterOnly` flag. Other book-only inputs are
  `MissingChapter` errors.
- Verse 0 is accepted by the parser (Psalm titles in `eng`); chapter 0 is not.
- The canonical set separator is `;` (`Rom.8.28;Rom.8.31;Rom.12.1`); the human form uses an
  en dash for ranges and elides repeated books and chapters.
- A split row is `exact` with several targets in the splitting direction and `partial` in the
  merging direction; the conformance fixture relies on this reading of specs/03's statuses.
- `reanchor()` matches the exact quote alone when the context match fails and reports a single
  hit as `Moved`; several hits are `Ambiguous`. The `id_lineage` step is left to the package
  crate, as the task's "minimal" scope allowed.
- `reanchor()` carries an anchor's offsets relative to its token span, so sub-token
  (morpheme-level) selections and selections running into untokenized punctuation survive a
  revision; the conformance suite needed this.
- `split_selection()` takes the joiner between segments as a string; verse-number runs and other
  paragraph decoration are the text crate's attributed-run work, not done here.

## Skipped or uncertain

- No Linux run, no UniFFI, no wasm-bindgen, no browser or Node execution (second wave).
- Grapheme expectations in the fixtures were generated by a simplified cluster rule and then
  agreed with unicode-segmentation 1.13.3; a future Unicode version could move a boundary, which
  the suite would report as a failure rather than hide.
- The English alias table is one list; it has not been reviewed against a published
  abbreviation standard, and only `Ph`, `Jud` and `Ez` are marked ambiguous.
- `BookId` has 66 variants; adding the deuterocanon is a MINOR bump of `API_VERSION`.
