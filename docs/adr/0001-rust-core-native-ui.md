# ADR 0001 — Rust core with native platform interfaces

Status: accepted by the project owner for blueprint revision 5 (27 October 2026); implementation evidence pending B005/B061. Supersedes the React Native + Expo / React + Vite / pure-TypeScript-domain baseline in revision 4 of specs/02-architecture.md.

## Context

Revision 4 chose a TypeScript-first stack mainly to minimize engineering effort. The owner has stated that engineering effort is not the binding constraint: every language is assumed to be available at expert level. The deciding criteria are therefore reading quality, interaction smoothness, measured performance, correctness of anchors and search, and the ability to deliver power-user text tools offline.

Three problems dominate the product:

1. **Exact text surfaces.** Hebrew with vowels and cantillation, mixed bidirectional text, Greek diacritics, arbitrary cross-verse selection mapped to stable token anchors, three synchronized panes, VoiceOver/TalkBack, dynamic type and stylus input.
2. **A corpus query engine.** Lemma/morphology/proximity/syntax/alignment queries, faceted concordances and prose full text, offline and well under the specs/01 budgets. See specs/19-search-and-retrieval.md.
3. **Cross-platform agreement.** An anchor, reference parse, versification mapping or search result must be identical on iPad, Mac, phone, web, Android and in the Obsidian plugin.

React Native's text component cannot provide precise custom selection ranges, highlight geometry or layout control without native modules, which is the risk revision 4 already named as its first gate. A TypeScript domain running on Hermes or in a browser is adequate for logic but is a poor host for a columnar corpus engine. Maintaining separate Swift, Kotlin and TypeScript domain logic would violate the existing requirement to avoid divergent domain implementations.

## Decision

- **One Rust core** (`crates/`) owns references and versification mappings, anchors and reanchoring, the package format and signature verification, the search and retrieval engines, collection/export contracts and the sync operation log. It exposes a stable, versioned API through UniFFI to Swift and Kotlin [S44], through `wasm-bindgen` to the web and Obsidian plugin, and directly to Rust desktop hosts.
- **Native interfaces per platform**, kept thin: all behavior that affects data lives in the core.
  - Apple: one SwiftUI application for iPadOS, macOS and iOS, with a TextKit 2 reading surface (UITextView/NSTextView-class views with virtualized paragraphs) [S45]. iPadOS is the primary teaching and study device; see specs/18-platform-capabilities.md.
  - Android: Kotlin + Jetpack Compose shell with a text surface built on the platform text stack (Minikin/HarfBuzz shaping) and an extended selection action mode.
  - Web: thin TypeScript interface over semantic DOM text; the Rust core runs as WebAssembly in a worker with OPFS persistence.
  - Windows/Linux desktop: Tauri shell [S46] over the web interface, with the core linked natively rather than as WebAssembly.
  - Obsidian: TypeScript plugin (required by the host) using the core's WebAssembly build so scripture blocks parse and render identically to the apps.
- **The core returns render-ready content**: attributed runs (verse/footnote markers, poetry indentation, divine-name small caps, supported red-letter markup) plus token boundaries pre-translated into each platform's string-offset unit (UTF-16 for Apple/Android/DOM). Platforms render; they do not reinterpret text.
- **Build-time and runtime share code.** The package builder and index builder are the same Rust crates the reader uses, so a shipped index is byte-identical to the one produced at build time and devices never re-index on install.
- **Python is limited** to experimental ML, OCR and linguistic evaluation work (P4/P5). Deterministic ingestion that produces published artifacts is Rust.
- **Optional services** (hub, sync, catalog, companion) are Rust (axum-class HTTP) over PostgreSQL, sharing core types. Storage and retrieval technology choices are decided in ADR 0002 after the revision 5 research.

## Consequences

- Three to four interface codebases can drift. Mitigation: a **shared conformance suite** of fixtures (Hebrew passage with cantillation, cross-verse selection, Psalm 9/10 numbering, bidi isolation, large text) run against every platform renderer, asserting that a selection produces the same core anchor, a highlight paints the same scalar range and screenshot baselines match within tolerance. Design tokens are generated from one source file.
- Android custom selection is the largest single native effort. The Apple reader ships first; Android follows the same conformance gate.
- UniFFI and WebAssembly bindings must be versioned with the core API. Breaking core API changes require coordinated platform updates in one change set.
- Platform-exclusive enhancements (Pencil, ultra-wide layouts, App Intents, Spotlight) are permitted, but every feature's data remains portable through the core; see specs/18-platform-capabilities.md.
- B005 is redirected from "can React Native select text?" to "TextKit 2 and DOM surfaces round-trip core anchors and meet the performance budgets on real iPad, Mac and browser targets".

## Alternatives considered

| Option | Why not chosen |
|---|---|
| React Native + Expo with TypeScript domain | Selection/layout control is the weakest part of the framework; requires native modules for the core experience anyway |
| Flutter or a custom GPU text renderer (Skia, wgpu + cosmic-text/parley) | One consistent renderer, but selection, accessibility and platform text services become our own implementation |
| Kotlin Multiplatform core | Strong for Android/Apple, weaker for WebAssembly and the Obsidian plugin; less mature corpus/search ecosystem |
| Swift-only Apple app | Best Apple experience but no shared core for web, Android, Obsidian or the build pipeline |
| C++ core | Comparable reach, weaker memory safety and packaging ergonomics than Rust |

## B005 findings

Spike B005 (docs/evidence/B005.md, 28 October 2026) ran the redirected question against the
Mac mini natively, the iPad Pro 11-inch (M5) simulator and headless Chromium, with the Rust
core unchanged and the B061 binding surface as is.

**Both surfaces round-trip the core's anchors on every case.** A TextKit 2 surface
(`UITextView` and `NSTextView` created with `NSTextLayoutManager`) and a semantic-DOM surface
(plain HTML, the browser `Selection`, the wasm core) each took thirteen driven selections
(cross-verse in BSB John 3:16–17, inside and across Hebrew combining marks in WLC Genesis
1:1–2, Greek diacritics, a note with an emoji modifier sequence and bidi isolates) through
`selectionToAnchors` and `anchorToUtf16` and painted the ranges the core handed back. On every
case the painted text, the anchors' quoted text and the selected core text were identical, and
the anchors matched the conformance fixtures. Verse numbers and other non-core runs are mapped
out by the surface before the core sees an offset, and are never painted.

**Grapheme clusters are the platform's job, and the platforms differ.** No user can select
between a letter and its marks on any of the three; programmatically, `NSTextView` widens such a
range itself, `UITextView` and Chromium keep it, and the surface widens it (composed character
sequences, `Intl.Segmenter`) before calling the core. The core's `NotGraphemeBoundary` was never
raised. The consequence stands: the core measures scalars, the surface owns clusters.

**One measured cost, not a failing surface: the paragraph crosses the binding on every
selection change.** `selectionToAnchors` takes the whole `Paragraph` by value. For a two-verse
pane the two core calls take 0.03–0.3 ms; for a synthetic 1,000-verse paragraph set (27,000
token spans) they took 28–43 ms on the Mac and 35 ms on the simulator, and the full selection
latency (text view selection, scroll-to-visible, core calls, paint) was about 80–95 ms at p50,
against the specs/01 budget of 100 ms for a visible highlight acknowledgement. The alternative,
measured in principle and compatible with the present API, is to pass only the segments the
selection touches (the surface already knows their offsets), which is O(selected segments); a
paragraph handle kept by the core is the cleaner binding follow-up. B062 should pick one before
a whole-book paragraph set exists. Scrolling the same 1,000-verse set at 720 pt/s held 60 Hz on
the simulator at both widths and on the Mac mini at 1024 points; on the Mac at 1440 × 900 the
programmatic scroll (bounds origin set and reflected every frame, which bypasses responsive
scrolling) took two display intervals for 40 % of frames, which is a profiling task for B062,
not a verdict on NSTextView; see the evidence for the numbers and the caveats.

**Decision kept, with two notes for B062.** The verse-marker run layout is produced by the
surfaces today (`RunMap` in Swift, the `.marker` rule in the DOM) and should come from the core
as this ADR already says, so the three surfaces cannot drift on it. TextKit 2 rendering
attributes did not paint a background in either view; the spike used storage attributes, and the
production reader should draw highlights in a layout-fragment subclass. Android remains a sketch
(apps/android/README.md) and its gate its own task.
