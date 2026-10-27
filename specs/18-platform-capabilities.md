# Platform roles and capability tiers

Revision 5. Every platform is a first-class reader; they are not identical. The product owner's working assumption is that people in this audience typically **study and teach from an iPad**, prepare at a desk alongside Obsidian, and use a phone for quick lookup and capture. The architecture in docs/adr/0001-rust-core-native-ui.md lets each platform add abilities its hardware supports without forking data or behavior.

## Principle: capability-gated, data-portable

1. **Core parity.** Every platform can read offline, navigate, run all deterministic searches, view cross references, highlight, take notes, collect resources, export/import, install packages and hand off to Obsidian. These behaviors live in the Rust core, so results are identical everywhere.
2. **Platform enhancements.** A platform may add interactions its input, display or compute make possible. Each enhancement declares the capabilities it requires (input: pencil, touch, pointer, keyboard; display: width class, external display; compute: local model tier; integration: OS services).
3. **Lossless degradation.** Anything created with an enhancement opens everywhere without loss. A six-pane desktop workspace opens on an iPad as three visible panes plus preserved tabs; Pencil ink is viewable, exportable and finger/mouse-editable on other platforms; a desktop search workbench query reopens as the same query on a phone.
4. **No enhancement is a hidden prerequisite.** A workflow listed in specs/01-product.md must complete on every platform, with different ergonomics.

## Platform roles

| Platform | Role | Priority |
|---|---|---|
| iPadOS | Primary study and teaching surface: reading, comparing, Pencil annotation, collecting | First |
| macOS | Preparation and power-study workstation; ultra-wide layouts; search workbench; local AI | First (shares the Apple codebase) |
| iOS (phone) | Companion: lookup, reading, capture into collections, review notes | First-class, single-pane-first |
| Web | Install-free access, shareable static deployment, Windows/Linux fallback | Second |
| Windows/Linux desktop (Tauri) | Desktop workstation features on non-Apple hardware | Second |
| Android phone/tablet | Parity with iOS/iPadOS minus Apple-specific services | Third, same conformance gate |
| Obsidian plugin | Writing-environment integration (desktop and mobile Obsidian) | P1 bounded, see specs/13 and 15 |

## Capability matrix

| Capability | iPadOS | macOS | iOS | Web | Win/Linux | Android |
|---|---|---|---|---|---|---|
| Visible reading panes (default max) | 3 landscape / 2 portrait | width-derived, 3–6 | 1 + tabs | width-derived | width-derived, 3–6 | 1–3 by width |
| Pencil/stylus ink and margin mode (P2) | Apple Pencil, hover, Scribble | trackpad/mouse edit, view | finger edit, view | pointer edit, view | pointer/pen | Android stylus ink |
| Keyboard command launcher | with hardware keyboard | yes | — | yes | yes | with keyboard |
| Multiple windows / detachable panes | Stage Manager windows | yes | — | browser windows | yes | freeform where supported |
| External display | extended reading window | yes | mirroring | — | yes | where supported |
| Search workbench (builder + results grid + KWIC + facets side by side) | compact | full | summary view | full | full | compact |
| Drag passages/collection items to other apps | yes | yes | yes | limited | yes | yes |
| OS integrations | App Intents, Spotlight, Handoff, share sheet | App Intents, Spotlight, Handoff, Services | App Intents, widgets, share sheet | — | — | intents, share sheet |
| Same-machine Obsidian vault access | via plugin/file transfer | direct, user-granted folder | via plugin/file transfer | file transfer | direct, user-granted folder | via plugin/file transfer |
| Local AI (optional) | small on-device models | larger local models on Apple silicon | small on-device models | WebGPU where available | local runtime if installed | small on-device models |
| Maintainer tools (ingestion, review) | — | yes | — | review UI only | yes | — |

Values are design targets for B062, not measured guarantees. Width-derived pane counts follow the content-based minimums in specs/12-workspace-ux.md (roughly 300–340 CSS px per pane at normal text, larger for original-language text); text is never shrunk to fit a pane count.

## Ultra-wide and large desktop layouts

At 3440–5120 px widths the desktop may show four to six reading panes, or pane **groups**: for example a Bible group (three translations linked), an original-language group (Greek text + lexicon) and a research group (commentary + collection). Groups share link state and can be collapsed to one column. The shared command bar remains single; each pane keeps the one-row identity header from specs/16-design-guide.md. Measure prose width per pane and offer a centered focus mode so a single pane does not stretch to an unreadable measure.

## Workspace layout profiles

A `Workspace` keeps one set of panes and link groups, with **per-device-class layout profiles** (compact, regular, wide, ultra-wide). Opening a Mac workspace on an iPad selects or derives the regular profile; it never discards panes. Changing a layout on one device class does not rewrite another's profile. This extends the Workspace entity in specs/03-data-model.md and the fixture contract in schemas/workspace.schema.json (B040).

## Teaching view (D18-1, decided)

The owner accepted a bounded **teaching view** on 27 October 2026: a large-text mode for preaching and teaching from the device in hand. It displays what was prepared in the ordinary study workspace; it does not add a place to write.

- **What it shows.** An ordered **teaching sequence** of passages (in one or more editions) and items from a collection (quotations, the user's own notes, resource excerpts within rights). The sequence is a Collection with a `teaching` role, built with the existing collection tools (specs/15-reformed-study-and-collections.md); no new editor, document type or workflow state is introduced.
- **How it reads.** Large type with a remembered per-device scale, high-contrast light, dark and warm themes, generous measure, verse numbers optional, reference always visible. Optional current-item emphasis dims the rest. The screen stays awake while the view is open.
- **How it moves.** Navigation is locked to the sequence: previous and next by tap, swipe, hardware keyboard, Pencil tap or a paired page-turn remote, plus a jump list. A quick reference lookup opens any passage without leaving the view and returns to the sequence position. Accidental edits are impossible: highlighting and note editing are disabled until the view is closed.
- **External display.** On iPadOS and macOS, an external display can show either a mirror or an **audience view** limited to the current passage text and reference (no notes, no collection items). This is display of existing text, not slide authoring or export.
- **Where it runs.** iPadOS first, macOS and iPhone with the same sequence data (phone: single column), web and Android through the conformance suite. A sequence prepared on one device opens on every other (lossless degradation, above).
- **Still excluded.** Sermon manuscript editing, teleprompter scrolling of a manuscript, slide generation or export, timers tied to a manuscript, and any generated sermon content (specs/01-product.md).

Task B069 (P2) implements the view after saved workspaces (B041) and collections (B058) exist. Until then, focus mode plus large text is the supported path.

## Acceptance additions (B062)

- One workspace created on macOS with five panes in two link groups opens on iPad and iPhone without lost panes, link groups or scroll anchors.
- Ink created with Apple Pencil opens, exports and is edited by pointer on macOS and web; coordinates are unchanged.
- A search workbench query from macOS reopens identically on iPhone with the same result count and edition scope.
- Each capability in the matrix has an explicit unsupported-state design on platforms that lack it.
