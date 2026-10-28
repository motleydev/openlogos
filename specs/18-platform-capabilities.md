# Platform roles and capability tiers

Revision 6 (B062). Every platform is a first-class reader; they are not identical. The product owner's working assumption is that people in this audience typically **study and teach from an iPad**, prepare at a desk alongside Obsidian, and use a phone for quick lookup and capture. The architecture in docs/adr/0001-rust-core-native-ui.md lets each platform add abilities its hardware supports without forking data or behavior.

Revision 6 adds the display-class review, the layout-profile derivation rules, the unsupported-state and degradation table for every matrix row, and the B062 review note. The teaching view section is unchanged from revision 5 (decision D18-1).

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
| Visible reading panes (default max) | width-derived, up to 3 on the iPad's own screen (2 at 744–834 pt portrait, 3 at 1024 pt and above); wide and ultra-wide profiles on an external display | width-derived, 3–6 | 1 + tabs (the compact profile in every orientation) | width-derived, 1–6 | width-derived, 3–6 | width-derived, 1–3 on the device's screen |
| Pencil/stylus ink and margin mode (P2) | Apple Pencil, hover, Scribble | trackpad/mouse edit, view | finger edit, view | pointer edit, view | pointer/pen | Android stylus ink |
| Keyboard command launcher | with hardware keyboard | yes | with hardware keyboard | yes | yes | with keyboard |
| Multiple windows / detachable panes | Stage Manager windows | yes | — | browser windows | yes | freeform where supported |
| External display | extended reading window | yes | mirroring | — | yes | where supported |
| Search workbench (builder + results grid + KWIC + facets side by side) | compact | full | summary view | full | full | compact |
| Drag passages/collection items to other apps | yes | yes | yes | limited | yes | yes |
| OS integrations | App Intents, Spotlight, Handoff, share sheet | App Intents, Spotlight, Handoff, Services | App Intents, widgets, share sheet | — | — | intents, share sheet |
| Same-machine Obsidian vault access | via plugin/file transfer | direct, user-granted folder | via plugin/file transfer | file transfer | direct, user-granted folder | via plugin/file transfer |
| Local AI (optional) | small on-device models | larger local models on Apple silicon | small on-device models | WebGPU where available | local runtime if installed | small on-device models |
| Maintainer tools (ingestion, review) | — | yes | — | review UI only | yes | — |

Values are design targets reviewed in B062 against published display classes and B004's measurements (below), not guarantees measured on hardware. Width-derived pane counts follow the content-based minimums in specs/12-workspace-ux.md (roughly 300–340 CSS px per pane at normal text, larger for original-language text); text is never shrunk to fit a pane count. The pane counts in the first row are what the derivation rules below produce at the widths in the display-class review; a user's saved profile can show fewer.

## Unsupported states and degradation (B062)

Every row of the matrix has an explicit state on the platforms that lack the capability and a path by which data created with the capability opens without loss. Nothing a platform enhancement creates is stored in a form only that platform can read.

| Capability | Unsupported or reduced state (what the reader sees) | Lossless degradation (what happens to the data) |
|---|---|---|
| Visible reading panes | A platform always shows at least one pane; there is no unsupported state. When a width holds fewer panes than the workspace has, the rest are tabs under the identity row with their link state shown and announced (specs/21 §10). | The workspace keeps every pane, link group and scroll anchor; only the per-class layout profile differs (rules below). A pane never leaves the workspace because a screen could not show it. |
| Pencil/stylus ink and margin mode | Without a stylus the ink tools are the same palette driven by finger or pointer; hover previews and Scribble are simply absent (no greyed placeholder for them). A renderer without the native fidelity blob shows the portable geometry and says "Shown from portable strokes" in the sheet's metadata, not as an error. | `InkDocument` stores portable stroke geometry in the snapshot's immutable coordinate space (specs/03, specs/13); the native blob is an optional attachment. Coordinates are unchanged on every platform (acceptance addition 2). |
| Keyboard command launcher | Without a hardware keyboard the launcher is still a searchable command list opened from the workspace menu; ⌘K and the other shortcuts appear in menus only when a keyboard is attached. Every command keeps a visible menu or touch route (specs/21 §9). | Nothing is stored; shortcuts are not part of workspace data. |
| Multiple windows / detachable panes | On a single-window platform "Open in new window" and "Detach pane" do not appear. | A detached pane is still a member of its workspace; window geometry is never stored in the workspace. On a single-window platform the pane takes its place in the class's profile (visible or tab) like any other pane. |
| External display | With no external display the audience view of the teaching view (D18-1) and the extended reading window are absent from menus; the teaching view runs on the device screen. Mirroring is the operating system's, not a product feature. | A display is a presentation target, not data. A workspace or teaching sequence shown on an external display is the same record as on the device. |
| Search workbench | Compact (iPad): builder, results grid, KWIC and facets are two columns that swap on demand. Summary view (phone): the query text, scope (editions, collections), result count and the result list, with facets and KWIC one tap away. Nothing is dropped from the query. | One query object (editions, scope, filters, expert syntax) is stored and reopened identically everywhere; the result count and edition scope are the same on macOS and iPhone (acceptance addition 3). |
| Drag passages/collection items to other apps | Where drag is limited (web) or absent, "Copy with reference" and Share/Export produce the same payload. | Drag never carries anything Copy cannot: text, reference, edition identity and the stable item link. |
| OS integrations | Without App Intents, Spotlight, Handoff, widgets or Services, the same actions (open passage, search, open collection) are available from the passage field, Search and the workspace menu. The app never asks the reader to install an OS feature. | Intents act on the same records; no intent or shortcut is stored in a workspace, collection or note. |
| Same-machine Obsidian vault access | With no user-granted folder the handoff preview shows "Export file" as the destination instead of the vault (specs/21 §6). | The export package (collection index, evidence blocks, materialized notes with stable block IDs) is identical whether written into a vault folder, through the plugin or to a file; the receipt records which. |
| Local AI | With no local model the research pane's model picker shows "No local model on this device" and offers only explicitly configured providers; there is no automatic paid fallback (specs/research-provider-policy.json). Deterministic search, cross references and word study are unaffected. | Threads and evidence bundles record the provider and model tier used; they are readable on every platform. A thread started with a local model continues on another device only with a provider the reader chooses. |
| Maintainer tools | Not shown on platforms without them; the web shows the review UI only. | Packages are built once, on a maintainer platform, and install everywhere; a device never re-indexes. |
| Teaching view (D18-1) | Same on every platform; the audience view needs an external display (row above). | The teaching sequence is a Collection with the `teaching` role and opens everywhere. |

## Display-class review (B062)

No physical devices were used in B062. The classes below are the published logical sizes (points) of current and recent devices, reviewed against the derivation rules in the next section. The only rendered numbers are B004's, measured on the iPad simulator with viewport overrides at 1440 × 900, 1024 × 768, 768 × 1024 and 390 × 844 points (docs/evidence/B004.md). Everything else in this table is a desk calculation and is marked so.

Derived pane counts assume layout A (no side chrome), drawers closed and the content minimums of the next section: 340 pt per pane at standard text, 480 pt at accessibility text sizes. "Class" is the device class the width selects when the platform has no size class of its own.

| Display | Logical width × height (pt) | Class | Panes at standard text | Panes at accessibility text | Verified how |
|---|---|---|---|---|---|
| iPhone, 375 wide (SE, 13 mini) | 375 × 667 or 812 | compact | 1 + tabs | 1 + tabs | published size; not rendered |
| iPhone, 390 wide (12, 13, 14) | 390 × 844 | compact | 1 + tabs | 1 + tabs | B004 rendered 390 × 844: one pane, 34 characters per line |
| iPhone, 393 and 402 wide (15, 16, 16 Pro) | 393 × 852, 402 × 874 | compact | 1 + tabs | 1 + tabs | published size; not rendered |
| iPhone, 430 and 440 wide (Plus and Pro Max) | 430 × 932, 440 × 956 | compact | 1 + tabs | 1 + tabs | published size; not rendered |
| iPhone landscape | 667–956 × 375–440 | compact (platform size class) | 1 + tabs (the width would hold 2; iOS stays single-pane, see decisions) | 1 + tabs | published size; not rendered |
| iPad mini portrait | 744 × 1133 | regular | 2 + tab | 1 + tabs | published size; not rendered |
| iPad 11-inch class portrait (iPad, Air 11, Pro 11) | 820 or 834 × 1180–1210 | regular | 2 + tab | 1 + tabs | B004 rendered 768 × 1024: two panes of 384 pt, 34 characters per line |
| iPad 13-inch class portrait (Pro 12.9, Pro 13, Air 13) | 1024 or 1032 × 1366–1376 | regular | 3 | 2 + tab | B004 rendered 1024 × 768 (the width, not the orientation): three panes of 341 pt, 31 characters per line; accessibility L: two panes plus a tab |
| iPad mini landscape | 1133 × 744 | regular | 3 | 2 + tab | published size; not rendered |
| iPad 11-inch class landscape | 1180–1210 × 820–834 | regular | 3 | 2 + tab | published size; the 1024 render is the nearest measurement |
| iPad 13-inch class landscape | 1366–1376 × 1024–1032 | regular (capped at 3 by the class) | 3 | 2 + tab | published size; not rendered |
| iPad Slide Over | 320 wide | compact | 1 + tabs | 1 + tabs | published size; not rendered |
| iPad Split View, one third | about 320–400 wide | compact | 1 + tabs | 1 + tabs | published size (varies by model and orientation); not rendered |
| iPad Split View, half | about 580–690 wide | compact | 1 + tabs | 1 + tabs | published size; not rendered |
| iPad Split View, two thirds | about 780–920 wide | regular | 2 + tab | 1 + tabs | published size; not rendered |
| iPad Stage Manager window | resizable, about 320 up to the full screen | by width | by width | by width | published behavior; not rendered |
| iPad with Stage Manager on an external display | the display's logical size (Studio Display 2560 × 1440, Pro Display XDR 3008 × 1692) | wide or ultra-wide | as the Mac rows below | as the Mac rows below | published behavior; not rendered |
| MacBook Air 13 (M1 default scaling) | 1440 × 900 | wide | 4 (3 for a three-pane workspace, 480 pt each) | 3 | B004 rendered 1440 × 900 on the iPad simulator at scale 0.579: three panes of 480 pt, 43 characters per line; not a Mac window |
| MacBook Air 13 (M2 and later, default) | 1470 × 956 | wide | 4 | 3 | published size; not rendered |
| MacBook Pro 14 | 1512 × 982 | wide | 4 | 3 | published size; not rendered |
| MacBook Air 15, MacBook Pro 16 | 1710 × 1112, 1728 × 1117 | wide | 5 | 3 | published size; not rendered |
| Studio Display (default 2× scaling) | 2560 × 1440 | ultra-wide | 6 (cap) at 426 pt | 5 | published size; not rendered |
| Pro Display XDR (default) | 3008 × 1692 | ultra-wide | 6 (cap) at 501 pt | 6 (cap) | published size; not rendered |
| Ultra-wide 34-inch, 3440 × 1440 px at 1× | 3440 × 1440 | ultra-wide | 6 (cap) at 573 pt | 6 (cap) | published size; not rendered |
| 5K2K 40-inch, 5120 × 2160 px at 1× (2560 × 1080 at 2×) | 5120 × 2160 | ultra-wide | 6 (cap) at 853 pt, text column centred at the measure cap | 6 (cap) | published size; not rendered |
| Web and Windows/Linux at 320 CSS px (WCAG reflow test) | 320 wide | compact | 1 + tabs | 1 + tabs | specs/16 requirement; not rendered |

Findings of the review that changed the matrix: the iPadOS row said "3 landscape / 2 portrait"; by the derivation the 13-inch iPads show three panes in portrait (1024 pt and above, which B004 rendered), the 11-inch class and the mini show two, and a 13-inch iPad in landscape (1366–1376 pt) would hold four at the minimum but is capped at three by the regular class. The iOS row gains the keyboard launcher with a hardware keyboard, which the earlier matrix omitted although specs/21 §9 provides it on every platform with a keyboard. The web row's pane count becomes 1–6 because a browser window spans phone to ultra-wide.

## Ultra-wide and large desktop layouts

At 3440–5120 px widths the desktop may show four to six reading panes, or pane **groups**: for example a Bible group (three translations linked), an original-language group (Greek text + lexicon) and a research group (commentary + collection). Groups share link state and can be collapsed to one column. The shared command bar remains single; each pane keeps the one-row identity header from specs/16-design-guide.md. Measure prose width per pane and offer a centered focus mode so a single pane does not stretch to an unreadable measure.

## Workspace layout profiles

A `Workspace` keeps one set of panes and link groups, with **per-device-class layout profiles** (compact, regular, wide, ultra-wide). Opening a Mac workspace on an iPad selects or derives the regular profile; it never discards panes. Changing a layout on one device class does not rewrite another's profile. This extends the Workspace entity in specs/03-data-model.md and the fixture contract in schemas/workspace.schema.json (B040); schema 1.1 (B062) carries the profiles and per-pane scroll anchors, and `crates/workspaces` implements the rules below as the pure function `derive_profile(workspace, device_class, width_pt, text_size)`.

### What is stored where

| Lives on the pane (one copy, every platform) | Lives in a class profile (one per device class) | Never stored |
|---|---|---|
| resource identity and revision, local passage, **link group or pin state**, **scroll anchor** (segment id and scalar offset), per-pane history | the ordered list of panes the reader arranged as visible on that class, the order of the remaining panes as tabs, optional pane widths for the visible panes, whether the notes dock is open, which link groups are collapsed to one column | window geometry, hover or focus-mode state, selections, credentials, copied private text |

Because link state and scroll anchors live on the pane, no profile can lose them: a profile only says where a pane is shown. A profile must list every pane of the workspace exactly once (visible or tab); the validator and the core reject one that drops a pane or lists one twice.

### Device classes

The class comes from the platform's own size class where one exists (an iPhone is compact in every orientation; an iPad window is compact or regular by its horizontal size class) and from the width otherwise (Mac, web, desktop, an iPad on an external display). The width thresholds are the smallest widths that hold two, four and seven panes at the 340-pt standard minimum, rounded up for chrome; they are prototype defaults in the sense of specs/12, not immutable breakpoints.

| Class | Width (pt) | Columns at most (a collapsed group is one column) | Typical surfaces |
|---|---|---|---|
| compact | under 700 | 1 | iPhone, Slide Over, one-third and half Split View, narrow browser windows |
| regular | 700–1399 | 3 | iPad in either orientation, two-thirds Split View, small laptop windows |
| wide | 1400–2399 | 6 | MacBook Air and Pro, external monitors at 2×, iPad with Stage Manager on a display |
| ultra-wide | 2400 and above | 6, plus collapsed group columns | Studio Display, Pro Display XDR, 3440 and 5120 px ultra-wides at 1× |

### Derivation rules

Given a workspace, a class, the width available to panes (the window width minus any side chrome such as layout B's launcher or a pinned drawer) and the text size:

1. **Slots.** `slots = max(1, min(class maximum, floor(width ÷ pane minimum)))`. The pane minimum is 340 pt at standard text sizes (B004: 341-pt panes at 1024 carry 31 Latin characters per line) and 480 pt at accessibility text sizes (B004 decision 3). With the notes dock open on a regular or wider class, the dock takes one of the slots as a column of equal width (never the last one: a single slot stays with the panes and the dock becomes a sheet); on compact the dock is a sheet and takes no slot. The class maximum counts columns: the visible members of a collapsed link group share one column.
2. **Priority order.** If the workspace stores a profile for the class, the order is its visible list followed by its tab order, and only the panes in its visible list may take a slot (the reader's arrangement is honoured even when the width would hold more). Otherwise the order is the nearest stored profile's (compact looks at regular, wide, ultra-wide; regular at wide, compact, ultra-wide; wide at ultra-wide, regular, compact; ultra-wide at wide, regular, compact) and every pane may take a slot; with no profile at all the order is the workspace's pane order. Panes the profile does not know are appended as tabs; ids the workspace no longer has are ignored.
3. **Columns.** Walk the order. A pane whose link group is collapsed for this class and already has a column stacks into that column (shown as a tab inside it, one slot for the group). Otherwise it takes the next free slot. Panes beyond the slots, or beyond the profile's visible list, become tabs in order.
4. **Active pane.** The active pane is always shown. If it landed as a stacked pane it becomes its column's shown pane; if it landed as a tab and its collapsed group already has a column, it becomes that column's shown pane with the previous one stacked; otherwise it takes the last column without a stack (the last column if every column has one) and the displaced pane becomes the first tab. Nothing else moves.
5. **Widths.** The profile's widths are used when it has one for every column's shown pane, normalised to sum to one; otherwise columns are equal. A column wider than the measure cap (640 pt at standard text, scaled with the text) centres its text column at the cap (specs/16 "Prose measure").
6. **Result.** The derived layout names the columns (shown pane, stacked panes, width), the tabs, the dock placement (hidden, pane or sheet) and the effective profile for this class, which is what the host saves if the reader saves. Saving writes only this class's entry.

Worked example (the first acceptance addition): a workspace made on a Mac with five panes, three translations in group 1 and a Greek text with its lexicon in group 2, saved as a wide profile with all five visible. On an iPad at 1024 pt the regular profile is derived from the wide order: three slots, so the three translations are columns and the Greek text and lexicon are tabs, still in group 2, still at their scroll anchors. At accessibility text sizes the same width gives two slots. On an iPhone at 390 pt the compact profile shows the active pane with four tabs. Saving the iPad arrangement writes the regular entry and leaves the wide entry byte for byte as the Mac wrote it. The unit tests in `crates/workspaces` and the conformance fixture `workspace-layout-five-panes` check exactly this.

### Behaviour that follows

- Resizing a window re-derives within the same class; crossing a class boundary switches to that class's profile. A pane that was a tab keeps its scroll anchor and link membership, so it rejoins a group's navigation the moment it is shown.
- Opening a drawer over a pane (specs/21 §5) is transient and does not change the profile; pinning the drawer makes it a pane like any other.
- Focus mode (specs/21 §8) is transient and is not a profile.
- A 1.0 workspace file (no profiles) opens on every class through rule 2's last case; the first save on a class writes that class's entry.

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

## B062 review note

What was verified and how (details in docs/evidence/B062.md):

- **Verified by rendering (B004, iPad simulator, viewport overrides):** pane counts and widths at 1440, 1024, 768 and 390 pt at standard text, and at 1024 pt at accessibility L; characters per line at those widths; the measure cap of 640 pt in a single pane. These are simulator renders of a fixture prototype, not device measurements, and the 1440 render was a scaled drawing on the iPad simulator, not a Mac window.
- **Verified by test (B062):** the derivation rules above as `crates/workspaces` unit tests and the conformance fixture, for the five-pane two-group workspace on wide, regular (standard and accessibility text), compact and ultra-wide widths; the schema and validator reject a profile that drops a pane, lists one twice, exceeds the class maximum or carries another class's layout.
- **Reviewed on paper only:** every row of the display-class table marked "published size; not rendered"; the Mac, iPhone, Stage Manager and ultra-wide classes in particular. No iPad, Mac, iPhone or external display was used in B062, and no web or Windows/Linux rendering exists yet.
- **Not verified at all:** the matrix rows for ink, windows, external displays, OS integrations, vault access, local AI and maintainer tools describe designs; none has an implementation to check against. Their unsupported states are specifications for the tasks that build them.
