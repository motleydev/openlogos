# B078 evidence (core part) — continuous reading

Recorded 3 November 2026 on the Mac mini (`office-hub`, Apple M1). Work was done in the git
worktree `openlogos-b078` (branch `b078-core-reading`, from main fc1c439, B076 and B077
merged) by a Claude Code subagent while the owner slept. Nothing was pushed or merged; no
simulator, device, keychain or macOS user was touched; no Bible text beyond the conformance
fixtures already in the repository was read, fetched or committed. This is the core part: the
web and Mac hosts build the reading surface on this branch in their own B078 parts.

Owner direction (2026-11-03): "When I type a Bible passage, I want to be taken to the whole
context, infinite scroll style, of scripture in context." Full text is blocked on the B071
permission, so the machinery runs tonight on **placeholder editions** with the real canon
structure, and B071's packs plug in later without host changes. Spec:
specs/23-continuous-reading.md.

## What was built

| Item | Where |
|---|---|
| Canon and versification table | `crates/core/src/canon.rs` and the generated `canon/data.rs` (`tools/gen_canon.py`): chapters per book and verses per chapter in `eng` and `org` for the 66 books, the 144 `eng` → `org` rows, `map`, `first_verse`/`last_verse`/`chapter_count`/`contains`, `segment_reference`, `BookOrder` (`protestant`, `tanakh_bhs`) |
| Reading stream and TextPack | `crates/text/src/reading.rs`: `ReadingLibrary` (`locate`, `chapter_neighbors`, `chapter_content`, `chunk_for`, `supply_chunk`, `editions`, `schemes`), `TextPack`, `ChapterBlock`, `Location`, `ReadingError`, `reference_for_segment` |
| Placeholder pack | `crates/text/src/reading/placeholder.rs`: `PlaceholderPack` over the canon table, `OverlayVerse`, `placeholder_text` |
| File pack format | `crates/text/src/reading/pack.rs`: `PackManifest`, `ChapterChunk`, `FilePack` (disk through `DirSource`, memory, or chunks a browser supplies), `PackWriter`; `schemes/text-pack.schema.json` |
| Fixture packs | `crates/conformance/fixtures/packs/{bsb,sblgnt,wlc}`: one chapter each from the text fixtures (WLC as a bundle), written by `fixture_packs()`; a test keeps the files equal |
| Shell | `crates/workspaces/src/reading.rs` (`TabPosition`, `FollowMapping`, `map_reference`, `highlight_of`, `holds`); `commands.rs`: `tab.record_scroll` moves the passage and carries linked tabs, `nav.navigate` maps followers and marks `highlight_range`, `nav.back`/`forward` clear it, `tab.open_edition` maps into the edition's scheme, `Effects.positions`, `Shell::set_resource_schemes`; `Tab.highlight_range` (schema 2.0, optional) and `TabView.highlight_range` |
| Bindings | UniFFI `crates/ffi/src/native/reading.rs` (`ReadingHandle`, records, `reference_for_segment`), `canon_map_verse`, `ShellHandle.set_resource_schemes`, `ShellOutcome.positions`, `ShellTab.highlight_range`; wasm `ReadingHandle`, `referenceForSegment`, `canonMapVerse`, `ShellHandle.setResourceSchemes`, TypeScript types |
| Conformance | `crates/conformance/fixtures/continuous-reading.json` (kind `reading`) with `crates/conformance/src/reading.rs` |
| Host tests | Swift `testContinuousReadingStreamsChaptersAndTheShellFollows`, Kotlin `continuousReadingStreamsChaptersAndTheShellFollows`, Node `B078: continuous reading ...`, wasm worker `reading_handle_streams_chapters_and_the_shell_follows_scrolling` |
| Spec, backlog, validator | specs/23 (new), specs/22 points to it; B078 in specs/backlog.json; B071 gains "writes the B078 pack format" (no new dependency); `tools/validate.py`: kind `reading`, the text-pack schema over the fixture manifests, `check_text_pack` (every chunk by length and SHA-256), two negative cases |
| Speed | `openlogos-shell-bench` rows for locate, chapter content (placeholder and file pack), neighbours and scroll-follow |
| API | `API_VERSION` 0.1.2 → 0.1.3 (additive text and conformance API); the examples and host tests name it |

### Commits

| Commit | Subject |
|---|---|
| `9d0cd56` | B078: canon and versification table in core (eng and org, from S51) |
| `21e1040` | B078: reading stream, placeholder pack and text pack format in crates/text |
| `17c6108` | B078: shell follows the reading position (record_scroll, linked tabs, highlight) |
| `e5b6a3a` | B078: continuous-reading conformance fixture and the three fixture packs |
| `2875f3b` | B078: text-pack schema, highlight_range in workspace 2.0, validate.py checks packs |
| `ac678cf` | B078: UniFFI surface for continuous reading |
| `1a4475b` | B078: wasm surface and host tests for continuous reading; API 0.1.3 |
| `8791532` | B078: bench rows for locate, chapter content (placeholder and file pack), neighbours, scroll-follow |
| `1f4d81f` | B078: specs/23 continuous reading, backlog B078, B071 writes the pack format |
| `5f11a13` | B078: the reading fixture's segment cases are segment_references |
| `2418735` | B078: evidence (core part), lanes to follow |

plus the evidence commit that records the lanes below.

## Canon data source

S51 (specs/10-sources.md): the Copenhagen Alliance versification mappings,
`versification-mappings/standard-mappings/eng.json` and `org.json`, derived from the Paratext
`.vrs` files, at commit `519af776ab65d89a51e37f750779250f940459ef` of
https://github.com/Copenhagen-Alliance/versification-specification. Data licence CC BY-SA 4.0
(code Apache 2.0). The two JSON files (27,681 and 21,077 bytes; sha256 `5162023f…1338` and
`3b8e065f…cac7`, full hashes in the generated file) were fetched once into the session
scratchpad, not committed; `tools/gen_canon.py --eng … --org … --commit …` turns them into
`crates/core/src/canon/data.rs`, which carries the attribution, the commit and the hashes.
Structure only: verse counts and numbering rows, no text.

Checks on the table (unit tests in `canon.rs`): 31,104 `eng` verses (the KJV's 31,102 plus
Acts 19:41 and 2 Cor 13:14, which the original numbering folds into a neighbour); Psalm 119
has 176; Joel has 3 chapters in `eng` and 4 in `org`, Malachi 4 and 3; every `eng` verse maps
to `org` and back to itself except Acts 19:41 and 2 Cor 13:14 (unmapped). The conformance
suite also checks the table against the hand-written `joel-malachi-chapter-numbering` and
`psalm-9-10-numbering` fixtures on every verse their rows cover.

## API

Rust (`openlogos_text::reading`, `openlogos_core::canon`, `openlogos_workspaces::reading`),
JSON (what wasm returns, field for field what the UniFFI records hold), Swift/Kotlin names
(UniFFI camelCases them), wasm names.

| Rust | UniFFI (Swift / Kotlin) | wasm |
|---|---|---|
| `conformance::reading_library()` | `ReadingHandle.placeholder()` | `ReadingHandle.placeholder()` |
| `ReadingLibrary::new()` + `insert(FilePack)` | `ReadingHandle()` + `openPack(directory:)` / `addPackManifest(manifestJson:)` | `new ReadingHandle()` + `addPackManifest(json)` |
| `editions()`, `schemes()` | `editions()`, `schemes()` | `editions()`, `schemes()` |
| `locate(resource_id, input)` | `locate(resourceId:input:)` → `ReadingLocation` | `locate(resourceId, input)` |
| `chapter_neighbors(resource_id, key)` | `chapterNeighbors(resourceId:chapterKey:)` → `ReadingNeighbors` | `chapterNeighbors(resourceId, key)` |
| `chapter_content(resource_id, key)` | `chapterContent(resourceId:chapterKey:)` → `ReadingChapter` | `chapterContent(resourceId, key)` |
| `chunk_for(resource_id, key)` | `chunkFor(resourceId:chapterKey:)` → `ReadingChunk?` | `chunkFor(resourceId, key)` |
| `supply_chunk(resource_id, key, bytes)` | `supplyChunk(resourceId:chapterKey:bytes:)` | `supplyChunk(resourceId, key, Uint8Array)` |
| `reference_for_segment(segment_id)` | `referenceForSegment(segmentId:)` | `referenceForSegment(segmentId)` |
| `canon::map(&r, &from, &to)` | `canonMapVerse(reference:fromScheme:toScheme:)` | `canonMapVerse("Joel 2:28", "eng", "org")` |
| `Shell::set_resource_schemes(map)` | `ShellHandle.setResourceSchemes(schemes:)` | `ShellHandle.setResourceSchemes(obj)` |
| `Effects.positions: Vec<TabPosition>` | `ShellOutcome.positions: [ShellTabPosition]` | `outcome.effects.positions` |
| `Tab.highlight_range`, `TabView.highlight_range` | `ShellTab.highlightRange` | `view …tabs[i].highlight_range` |

Errors: Rust `ReadingError` (serde tag `kind`); UniFFI `ReadingCallError.Refused(kind, reason)`
(Kotlin `ReadingCallException.Refused`, fields `kind`, `reason`; no `message`/`cause`); wasm
throws an `Error` with `kind`, `message` (the reason) and `details`. Kinds: `unknown_resource`,
`invalid_reference`, `book_not_in_edition`, `chapter_out_of_range`, `verse_out_of_range`,
`unknown_chapter`, `chunk_not_loaded`, `hash_mismatch`, `invalid_pack`.

## JSON examples

Produced by the wasm build (`apps/web/core-host/pkg`) against the placeholder library and the
`shell-commands` fixture session with the schemes registered; UniFFI records hold the same
fields. Long arrays shortened where marked.

### reading.locate("bsb", "Jn 3:16-18")

```json
{
 "resource_id": "bsb",
 "chapter_key": "John.3",
 "segment_id": "John.3.16",
 "reference": "John.3.16-John.3.18",
 "verse_start": "John.3.16",
 "verse_end": "John.3.18",
 "highlight_range": "John.3.16-John.3.18",
 "granularity": "verse"
}
```

### reading.locate("bsb", "Psalm 3")

```json
{
 "resource_id": "bsb",
 "chapter_key": "Ps.3",
 "segment_id": "Ps.3.0",
 "reference": "Ps.3",
 "verse_start": "Ps.3.0",
 "verse_end": "Ps.3.8",
 "highlight_range": null,
 "granularity": "chapter"
}
```

### reading.locate("wlc", "Mal 4:1") throws

```json
{
 "kind": "chapter_out_of_range",
 "message": "Malachi has no chapter 4"
}
```

### reading.chapterNeighbors("bsb", "Mal.4")

```json
{
 "chapter_key": "Mal.4",
 "prev": "Mal.3",
 "next": "Matt.1"
}
```

### reading.chapterContent("bsb", "John.3") (verses shortened)

```json
{
 "resource_id": "bsb",
 "content_revision": "bsb-2024",
 "chapter_key": "John.3",
 "book": "John",
 "chapter": 3,
 "heading": {
  "book_title": null,
  "chapter_label": "Chapter 3",
  "short_label": "Jn 3"
 },
 "verses": [
  {
   "segment_id": "John.3.1",
   "number": 1,
   "last_number": 1,
   "label": "1",
   "text": "Placeholder verse — John 3:1",
   "paragraph_start": true,
   "poetry": false,
   "superscription": false,
   "placeholder": true,
   "tokens": []
  },
  {
   "segment_id": "John.3.16",
   "number": 16,
   "last_number": 16,
   "label": "16",
   "text": "For God so loved the world that He gave His one and only Son, that everyone who believes in Him shall not perish but have eternal life.",
   "paragraph_start": true,
   "poetry": false,
   "superscription": false,
   "placeholder": false,
   "tokens": [
    {
     "id": "bsb:44003016001",
     "start": 0,
     "end": 3
    },
    "… 26 more"
   ]
  },
  "… 34 more"
 ],
 "prev": "John.2",
 "next": "John.4",
 "placeholder": true
}
```

### reading.chapterContent("wlc", "Ps.3") (verses shortened)

```json
{
 "resource_id": "wlc",
 "content_revision": "oshb-wlc-4.20",
 "chapter_key": "Ps.3",
 "book": "Ps",
 "chapter": 3,
 "heading": {
  "book_title": null,
  "chapter_label": "Psalm 3",
  "short_label": "Ps 3"
 },
 "verses": [
  {
   "segment_id": "Ps.3.1",
   "number": 1,
   "last_number": 1,
   "label": "1",
   "text": "טקסט ממלא מקום — 3:1",
   "paragraph_start": true,
   "poetry": false,
   "superscription": true,
   "placeholder": true,
   "tokens": []
  },
  {
   "segment_id": "Ps.3.2",
   "number": 2,
   "last_number": 2,
   "label": "2",
   "text": "טקסט ממלא מקום — 3:2",
   "paragraph_start": true,
   "poetry": true,
   "superscription": false,
   "placeholder": true,
   "tokens": []
  },
  "… 7 more"
 ],
 "prev": "Ps.2",
 "next": "Ps.4",
 "placeholder": true
}
```

### shell.apply({command_id: "tab.record_scroll", args: {tab_id: "t1", segment_id: "John.3.17", scalar: 0}})

```json
{
 "command_id": "tab.record_scroll",
 "handled": true,
 "recorded": false,
 "effects": {
  "followed_tab_ids": [],
  "positions": [
   {
    "workspace_id": "w1",
    "tab_id": "t1",
    "driver": true,
    "scrolled": true,
    "reference": "John.3.17",
    "segment_id": "John.3.17",
    "reference_label": "Jn 3:17",
    "chapter_label": "Jn 3",
    "highlight_range": null,
    "mapping": "same"
   },
   {
    "workspace_id": "w1",
    "tab_id": "t3",
    "driver": false,
    "scrolled": true,
    "reference": "John.3.17",
    "segment_id": "John.3.17",
    "reference_label": "Jn 3:17",
    "chapter_label": "Jn 3",
    "highlight_range": null,
    "mapping": "exact"
   },
   {
    "workspace_id": "w1",
    "tab_id": "t4",
    "driver": false,
    "scrolled": true,
    "reference": "John.3.17",
    "segment_id": "John.3.17",
    "reference_label": "Jn 3:17",
    "chapter_label": "Jn 3",
    "highlight_range": null,
    "mapping": "unregistered"
   }
  ]
 },
 "revision": 2
}
```

### browser.chunkFor("wlc", "Gen.1")

```json
{
 "resource_id": "wlc",
 "chapter_key": "Gen.1",
 "path": "chapters.bin",
 "offset": 0,
 "length": 1728,
 "sha256": "3c96df62864b932fa4d1826d23a6babccbe4bcc556a047c9a4ae560ddf6468ca"
}
```

A navigation with linked tabs (the `continuous-reading` fixture session: BSB t1 and WLC t2 in
group 1, SBLGNT t3 following, BSB t4 pinned), `nav.navigate {reference: "Mal.4.5-Mal.4.6"}`:

| tab | reference | highlight_range | mapping |
|---|---|---|---|
| t1 (driver) | `Mal.4.5-Mal.4.6` | `Mal.4.5-Mal.4.6` | `same` |
| t2 WLC | `Mal.3.23-Mal.3.24` | `Mal.3.23-Mal.3.24` | `exact` |
| t3 SBLGNT | `Mal.3.23-Mal.3.24` | `Mal.3.23-Mal.3.24` | `exact` |

and `nav.navigate {reference: "Acts.19.41"}` moves t2 and t3 to `Acts.19.40` (`nearest`).

## Text pack format (summary; specs/23 §4)

A directory of static files: `manifest.json` (schemas/text-pack.schema.json: format
`openlogos-text-pack` 1.x, resource id, revision, names, language, direction, scheme, book
order, licence, source, `fixture_only`, `complete`, books in reading order with their titles
and per chapter `{chapter, first_verse, last_verse, path, offset?, length, sha256}`) and one
JSON chunk per chapter (`chapters/John/3.json`) or all chunks in one `chapters.bin` found by
offset and length. Chunk: `{format: "openlogos-text-pack-chapter", resource_id, chapter_key,
verses: [ChapterVerse]}`, UTF-8, text byte-exact. Every chunk is checked against its SHA-256
before use; the manifest's SHA-256 is what a catalog records. Natively the core reads from
disk; in a browser the page fetches what `chunkFor` names (whole file or byte range) and
hands it to `supplyChunk`. `PackWriter` writes the format for B071. Fixture pack manifests:

| Pack | Layout | Manifest SHA-256 |
|---|---|---|
| `bsb` | `chapters/John/3.json` (2,837 bytes) | `60eb101895c9f3e2a35fca6c83b4bfeb460ee55510db8bcb72d9c05181f37653` |
| `sblgnt` | `chapters/John/3.json` | `e97e810b31b2b1590aaae32e54252e921b645e84f4f3e6b602ec14daad684100` |
| `wlc` | `chapters.bin`, offset 0, 1,728 bytes | `d6614dbfc3e5582a6c8edf46c4d1c7242d8fd05ba6d5407f1bb6f55167580071` |

## Conformance

`crates/conformance/fixtures/continuous-reading.json` (kind `reading`), run by
`crates/conformance/src/reading.rs` and so by every host through `run_conformance`:
**123 cases**, all passing; the whole suite is **1,717 cases, 0 failed (api 0.1.3)** (1,594
at B077).

| Group | Cases | What |
|---|---:|---|
| locate | 26 | verse ranges, OSIS, across chapters, several ranges, chapters, titled Psalm, Psalm title verse, chapter range, whole book, book abbreviation, one-chapter book, Hebrew Joel 3 and 4 in the WLC, English Joel 3; errors: Malachi 4 in the WLC, verse and range end past the end, chapter past the end, Psalm 3:0 in `org`, John in the WLC, Genesis in the SBLGNT, not a passage, an edition not installed; on the fixture packs a verse, a missing verse and a missing chapter |
| neighbours | 13 | Genesis 1 opens, Genesis → Exodus, Malachi 4 → Matthew 1 and back, Obadiah between Amos and Jonah, Psalms → Proverbs, Revelation ends, WLC ends at Malachi 3, Hebrew Joel 3 → 4, SBLGNT opens at Matthew 1, errors for a chapter the edition lacks and a book key, a one-chapter pack |
| chapters | 13 | BSB John 3 overlay (34 placeholders, the fixture verses real with 27 tokens), Genesis 1 heading and paragraphs, English Psalm 3 title as verse 0 with poetry, Hebrew Psalm 3 title as verse 1, Hebrew Psalm 51's two-verse title, WLC Genesis 1 overlay, SBLGNT John 3 in Greek placeholders, Hebrew Joel 4, Jude, Malachi 4 → Matthew, an error, two pack chapters (file and bundle) |
| segments | 1 | six segment ids, bridges and split verses included |
| canon | 18 + 14 | Joel, Malachi, Psalm titles (3, 51), Psalm 13 merge both ways, Genesis 31/32, Isaiah 9:1, Acts 19:41, a verse past the chapter, a chapter, an unknown scheme, within one scheme; plus the table checked against every `eng`/`org` case of the `joel-malachi-chapter-numbering` and `psalm-9-10-numbering` fixtures whose rows cover the verse |
| shapes | 10 | chapters and verse ranges of Joel, Malachi, Psalms (titles, 119), Genesis 32, Revelation 22, 3 John in both schemes |
| packs | 12 | the committed manifests' SHA-256; each pack reads its text fixture back byte for byte with its tokens; for a chapter file and a bundle range: where the chunk is, `chunk_not_loaded` before supply, a tampered chunk refused (`hash_mismatch`), the chapter read once supplied |
| shell scenarios | 16 steps | record_scroll moves tab, group and the active tab's follower (full positions, revision bumped, nothing recorded); a follower reporting where it was sent changes nothing; a scroll within a verse changes nothing; navigation marks and maps (full positions); scrolling inside the marked range keeps it; scrolling on keeps the mark; chapter navigation marks nothing; Back returns to the scrolled-to verse and clears the mark; chapters map by their first verse (`Joel.3` → `Joel.4`, `Mal.4` → `Mal.3.19`, `Ps.3` → `Ps.3`); a non-active tab drives only its group; a pinned tab scrolls alone; `nearest` for Acts 19:41; the host's reference wins over the segment's; compare opens at the mapped reference |

## Bench (`just bench-shell`, release, 100 rounds)

`target/release/openlogos-shell-bench --json target/bench-shell/b070.json`. The B078 rows
(the earlier rows are unchanged in kind and within their budgets):

| interaction | budget | samples | p50 µs | p95 µs | max µs |
|---|---:|---:|---:|---:|---:|
| locate a typed passage (ranges, chapters, books) | 5 ms | 800 | 38.8 | 76 | 84.4 |
| chapter content, placeholder pack (Ps 119 among them) | 5 ms | 700 | 10.8 | 62.6 | 68 |
| chapter content, file pack on disk (read, SHA-256, parse) | 5 ms | 220 | 393.8 | 665 | 1659.7 |
| chapter neighbours | 5 ms | 700 | 0.4 | 0.4 | 0.5 |
| scroll into the next verse: tab, group and follower move (tab.record_scroll) | 16 ms | 2000 | 8.3 | 8.7 | 18.2 |

The file-pack row writes John 1–21 and Psalm 119 in the pack format under
`target/bench-shell/b078-pack` with verses padded to real length and 25 tokens each (Psalm 119
is about 100 KB), so each sample is a file read, a SHA-256 over the chunk and a JSON parse.
Every row is far below the 100 ms open-chapter budget (specs/22 §7); the core's share of
opening a chapter is under 0.7 ms p95 from disk and under 0.1 ms generated.

The whole run at `2418735` (all rows; B070–B077 rows for comparison):

| interaction | specs/22 §7 budget | samples | p50 µs | p95 µs | max µs |
|---|---:|---:|---:|---:|---:|
| palette keystroke: passage, commands, tabs (index kept) | 16 ms | 11800 | 137.4 | 307.6 | 1134.1 |
| palette keystroke with the text section (fixture provider) | 50 ms | 3200 | 41.3 | 359.5 | 558.3 |
| palette index build after a command (120 tabs) | 16 ms | 200 | 250.7 | 259.8 | 897.4 |
| tab switch within a column (tab.next) | 16 ms | 1200 | 0.6 | 0.9 | 11.3 |
| column focus (column.focus N) | 16 ms | 300 | 0.6 | 0.7 | 1.3 |
| tool well for the focused tab or dock (Shell::tool_well) | 16 ms | 600 | 8.2 | 8.7 | 13.3 |
| context menu for every target kind (Shell::context_menu) | 16 ms | 2100 | 11.9 | 82.5 | 104.9 |
| workspace switch (workspace.next, 10 × 3 × 4) | 100 ms | 1000 | 0.5 | 0.7 | 1 |
| tool change (tool.*) | 16 ms | 300 | 0.9 | 1.2 | 2.2 |
| go to passage, group and follow tabs move (nav.navigate) | 100 ms | 200 | 70.3 | 80.3 | 125.1 |
| move tab to column 2, with undo record | 16 ms | 200 | 34.8 | 57 | 61.2 |
| undo | 16 ms | 200 | 102.4 | 114.5 | 161.5 |
| session view with tab labels (Shell::view, 120 tabs) | 1 ms | 200 | 99.5 | 106.6 | 353.5 |
| locate a typed passage (ranges, chapters, books) | 5 ms | 800 | 38.8 | 76 | 84.4 |
| chapter content, placeholder pack (Ps 119 among them) | 5 ms | 700 | 10.8 | 62.6 | 68 |
| chapter content, file pack on disk (read, SHA-256, parse) | 5 ms | 220 | 393.8 | 665 | 1659.7 |
| chapter neighbours | 5 ms | 700 | 0.4 | 0.4 | 0.5 |
| scroll into the next verse: tab, group and follower move (tab.record_scroll) | 16 ms | 2000 | 8.3 | 8.7 | 18.2 |

## Lanes

Run in order on the committed tree at `2418735` (`git status` clean after the run; the
evidence commit only adds this file's results).

| Lane | Exit | Summary lines |
|---|---|---|
| `nix develop .#core -c just ci` | 0 | `Summary [ 6.623s] 330 tests run: 330 passed, 0 skipped`; doc tests pass; `PASS: 36 schema/example validations; 14 agent records; 78 acyclic tasks; 21 conformance fixtures; …; 127 negative tests …`; clippy `-D warnings`, rustfmt, nixfmt, shellcheck, actionlint clean; `check-wasm` finished |
| `nix develop .#apple -c just test-swift` | 0 | `Executed 10 tests, with 0 failures (0 unexpected)`; `conformance cases: 1717 passed, 0 failed` |
| `nix develop .#android -c just test-kotlin` (test-results cleared first) | 0 | 10 `CoreHostTest > … PASSED` (the new `continuousReadingStreamsChaptersAndTheShellFollows()` among them); `BUILD SUCCESSFUL` |
| `nix develop .#web -c just test-web` | 0 | `openlogos_ffi_bg.wasm: 3135460 bytes before wasm-opt -Oz (691325 gzipped), 2903945 after (772483 gzipped)`; `tests 10`, `pass 10`, `fail 0`; `conformance cases: 1717 passed, 0 failed (api 0.1.3)` |
| `nix develop .#web -c just test-wasm` | 0 | headless Chrome, dedicated worker: `test result: ok. 9 passed; 0 failed` |
| `just bench-shell` (in `.#core`) | 0 | the table above; every row within its budget |

An earlier `test-swift` run caught a real host issue: the Swift test
decodes every fixture with one shape and read the reading fixture's top-level `segments`
(segment-reference cases) as text segments. The key is now `segment_references` (`5f11a13`); host agents decoding fixtures generically should
note that the `reading` fixture has keys of its own.

## Decisions for the owner

1. **Canon source: S51, fetched once.** The structure comes from the Copenhagen Alliance JSON
   form of the Paratext `eng.vrs`/`org.vrs` (already S51 in specs/10). I fetched the two
   structural files (verse counts and numbering rows, no text) into the scratchpad and
   committed only the generated table, with attribution. Their data licence is CC BY-SA 4.0;
   verse counts are facts, but if you want the table itself under a share-alike notice in the
   repo (or a rights record for S51), say so. No corpus was fetched.
2. **Schemes per edition: BSB `eng`, SBLGNT `org`, WLC `org`.** Paratext `eng` is modern
   English numbering (Psalm titles as verse 0, 3 John 15), which BSB follows more closely than
   the KJV's; `org` is the original numbering (Hebrew chapters in Joel and Malachi, titles as
   verse 1; NA-style New Testament, so SBLGNT has Acts 19:40 and 2 Cor 13:13 as last verses).
   B071 should confirm against the real texts and set `scheme` in each manifest.
3. **Book order: canonical Protestant order for all three.** `tanakh_bhs` (BHS: Torah,
   Prophets, Writings ending with Chronicles) is implemented and tested but unused; a WLC pack
   can declare it in its manifest if you want the Hebrew Bible in Hebrew order.
4. **Coverage per edition.** WLC streams Genesis 1 → Malachi 3 and stops; SBLGNT Matthew 1 →
   Revelation 22; BSB the whole Bible (Malachi 4 → Matthew 1). A linked WLC tab following the
   BSB into the New Testament lands on a reference the WLC does not have and shows the
   not-in-this-edition state.
5. **Placeholder text** is English for BSB (`Placeholder verse — John 3:15`), Modern Greek
   "placeholder text" for SBLGNT (`Κείμενο κράτησης θέσης — 3:17`) and Modern Hebrew for WLC
   (`טקסט ממלא מקום — 1:3`), never pseudo-scripture, every verse `placeholder: true`. Layout
   is heuristic (a paragraph every five verses; poetry in Job 3–42:6, Psalms, Proverbs, Song,
   Lamentations). specs/23 §5 asks hosts never to quote, copy as Scripture, collect or send a
   placeholder verse to the assistant.
6. **Headings.** Placeholder editions use English book titles ("Song of Songs", "Psalms",
   chapter label "Psalm 23"); a file pack carries its own titles in its language.
7. **Locate is strict.** Out-of-range verses and chapters are errors with plain words ("John 3
   has no verse 37", "Malachi has no chapter 4" in the WLC), not clamped. A chapter opens at its
   first segment (a titled Psalm at its title); a book at 1:1; of several ranges the first is
   located; only verse passages are marked.
8. **The tab's passage follows scrolling.** `tab.record_scroll` sets a Reading tab's
   `reference` to the verse at the top unless its stored verse range already holds it, updates
   the current history visit in place (Back returns to where you were) and adds no visit. It
   stays quiet (no undo) but **bumps the shell revision when a reference moved**, so palette
   rows and labels refresh; a scroll within a verse does not bump.
9. **Who follows a scroll.** The scrolled tab's link group always; follow-mode tabs only when
   the scrolled tab is the active one (specs/21 §2: they follow the active passage). Pinned
   tabs scroll alone.
10. **Mapping fallbacks.** A verse with no counterpart lands on the nearest earlier verse of
    its chapter that has one (`nearest`: English Acts 19:41 → SBLGNT 19:40); a chapter maps by
    its first verse and stays a chapter when that lands on a chapter's first verse (`Joel.3` →
    `Joel.4`, `Ps.3` → `Ps.3`), else becomes a verse (`Mal.4` → `Mal.3.19`). A follower of a
    chapter navigation is not marked.
11. **Navigation now maps followers in the core** when the host registers schemes
    (`set_resource_schemes`); without registration behaviour is as before (same reference).
    `ShellHandle.fixture()` does not register them, so existing host tests are unchanged;
    hosts call `setResourceSchemes(reading.schemes())`. `tab.open_edition` opens the comparison
    at the mapped reference (BSB Joel 2:28 → WLC Joel 3:1).
12. **`highlight_range` is persisted** in the workspace record (optional field, schema 2.0
    additive), so a relaunch keeps the mark; cleared by the next navigation, Back and Forward
    included.
13. **Pack format: JSON chunks, a file per chapter by default, a bundle optional.** Static
    files keep the browser path to plain fetches with no server; the bundle exists for hosts
    that prefer one file and range requests. Locate on a file pack reads only the manifest; an
    excerpt pack (`complete: false`) reports verses it lacks as out of range.
14. **API 0.1.3**: an additive PATCH while the MAJOR is 0 (the rule in crates/core/src/lib.rs).

## For the host parts

Both hosts build the same surface from the same calls (specs/22 §8, specs/23 §7). The core is
on branch `b078-core-reading`; rebuild the bindings (`just test-swift`, `just wasm-build`).

**Set-up, once per window.**
1. `reading = ReadingHandle.placeholder()` (web `core.ReadingHandle.placeholder()`). Keep the
   host `TextSource` for resource metadata (names, direction, language) and the study note;
   reading tabs take their text from `reading`. When B071 lands, `openPack(directory)` (Mac)
   or `addPackManifest` + fetched chunks (web) replaces the placeholder editions with the same
   resource ids, and nothing else changes.
2. `shell.setResourceSchemes(reading.schemes())` after `setResourceTitles` /
   `setResourceLanguages`, so linked tabs follow through versification.

**Opening a passage (typed, palette, Back/Forward, restore).**
1. The core already put the passage in `tab.reference` (OSIS). Call
   `reading.locate(tab.content_id, tab.reference)`; on `book_not_in_edition` /
   `chapter_out_of_range` / `verse_out_of_range` show specs/21's not-in-this-edition state
   with the error's words (“John is not in WLC”), never another edition's text.
2. Load `location.chapter_key` with `chapterContent`, then its `prev` and `next` (each block
   carries them; `chapterNeighbors` answers without loading text).
3. Scroll `tab.scroll_anchor.segment_id` to the top when restoring a scrolled tab, else
   `location.segment_id`. For a bridge, the verse whose `number…last_number` holds the verse.
4. If `tab.highlight_range` is set (`John.3.16-John.3.18`, possibly across chapters), flash
   those verses once and keep a quiet mark on them; the range is every rendered segment from
   `location.verse_start` to `location.verse_end` in document order. Clear the mark when the
   tab's `highlight_range` becomes null (the next navigation, Back or Forward).

**The continuous scroll.**
1. Render a window of chapter blocks: the block in view plus one or two each side. A block is
   a heading (`heading.book_title` as the book heading on a book's first chapter,
   `heading.chapter_label` as the chapter heading) and its verses: verse label (`label`;
   empty for a Psalm title) and text, a new paragraph at `paragraph_start`, a line of its own
   for `poetry`, the title styled for `superscription`. Keep the web reader's segment markup
   (`[data-segment]` with the `.t` text span) so `scrollAnchorOf`, selection anchors and the
   context menu keep working; overlay verses carry `tokens` for anchors, placeholder verses
   none.
2. When the reader nears the bottom, append `next`; near the top, prepend `prev` **without a
   jump**: note the top segment's offset before inserting, then restore it (web: adjust
   `scrollTop` by the inserted height; Mac: shift the clip view's bounds origin, or anchor a
   SwiftUI `ScrollView` position to the segment id). Release blocks far out of view the same
   way. Never render a whole Bible.
3. A sticky header shows the block in view: `heading.short_label` (`Jn 3`) with the edition.
4. `placeholder: true` verses look like placeholders (muted, a one-line notice per block
   such as “Placeholder text — this edition is not installed”), and selection actions that
   quote or collect text are disabled on them (specs/23 §5).

**Reporting position and following.**
1. When scrolling settles, send `tab.record_scroll {workspace_id, tab_id, segment_id,
   scalar}` for the segment at the top (as today). It is quiet (no undo, no history), but
   when it moved the passage the revision moves and `outcome.positions` is non-empty.
2. For each position: if it is the scrolled tab (`driver`), update its identity row
   (`reference_label`) and tab label (`resource_title` + `chapter_label`: “BSB Jn 4”) in
   place. **Do not rebuild the reading column because its tab's reference changed by
   scrolling**: the web shell's `columnSignature` includes `t.reference` today and must leave
   it out for reading tabs (key the reading surface on tab id and edition; navigations arrive
   as positions with `scrolled: false`). The Mac `ColumnsView` must likewise not re-identify
   the reading view on a reference change.
3. For each follower with `scrolled: true`: load the chapter of its `segment_id` if needed and
   scroll that segment to the top without animation; mark the scroll as programmatic and do
   not report it with `record_scroll` (the core would ignore it anyway: a follower at its
   mapped verse does not move). Update its labels like the driver's.
4. For positions with `scrolled: false` (a navigation): rebuild or re-scroll each listed tab at
   its `segment_id` and flash its `highlight_range` (followers carry their mapped range).
   `mapping` is `exact`, `partial`, `nearest`, `unmapped`, `same` or `unregistered`; a host
   may show a small “nearest verse” hint for `nearest`.

**Checks for the host evidence.** Scroll BSB John 3 into John 4 and see the label become
“BSB Jn 4” and the identity row “Jn 4:1” without a jump; type “Mal 4” in a BSB tab linked to
a WLC tab and see the WLC tab at Malachi 3:19; scroll from Malachi 4 into Matthew 1 in the
BSB; type “Jn 3:16-18” and see the three verses flash and stay marked; scroll upward from
Genesis 2 into Genesis 1 without the text moving under the eye; screenshots of the sticky
header, a Psalm title, a book heading and the placeholder notice.

## Skipped

- B071's importer and any real text; the fixture packs hold only the conformance verses.
- Deuterocanonical books (the hook is in place: S51 has them, `BookId` does not yet).
- Using the Tanakh order for the WLC (available, not chosen).
- A chapter cache inside `FilePack` (reads from disk are 0.4–0.6 ms; not needed yet).
- An `lxx` scheme in the canon table (the Psalm 9/10 fixture table still answers `mapVerse`).
- Host UI (the web and Mac B078 parts).

## Follow-up: books in the palette (lead request, 2026-11-03)

`paletteQuery("Genesis")` gave only Tabs, `":Genesis"` nothing and `"Acts"` only Commands.
Now a bare book name or abbreviation (`palette::book_matches`: anything
`AliasTable::english().lookup` resolves, such as `Genesis`, `Gen`, `1 Jn`, `Song`, `Acts`,
`Jude`) is a Passage row titled with the book (`BookId::title`, moved into core from
`text::reading`) at score 1000, dispatching `nav.navigate {reference: "Gen.1"}`. A prefix of two
or more letters offers up to five books (`Jo` → Joshua, Job, Joel, Jonah, John). ` @N` and
` @new` apply, and in `:` mode the parse error notice is dropped when a book matches.

Decision: the row navigates to `Book.1` rather than the bare book id. Opening it lands on the
same first verse, and the tab label, identity row and linked-tab mapping work from the start;
a bare `Gen` would leave the tab without labels until the first scroll.

Ten palette cases were added to `shell-commands` (16 checks). No existing case changed: none
of the existing palette queries in any fixture is a bare book name or prefix (checked by
running `book_matches` over every query). Lanes at `edae739`, all exit 0: ci `330 tests run:
330 passed, 0 skipped` with validate PASS; test-swift `Executed 10 tests, with 0 failures`;
test-kotlin 10 PASSED, `BUILD SUCCESSFUL`; test-web `tests 10, pass 10, fail 0`; test-wasm `9
passed`; conformance `1733 passed, 0 failed (api 0.1.3)`.
