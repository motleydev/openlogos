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
