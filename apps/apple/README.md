# apps/apple

One SwiftUI application for iPadOS, macOS and iOS with a TextKit 2 reading surface
(docs/adr/0001-rust-core-native-ui.md, specs/18-platform-capabilities.md). What exists today is
the **app shell** in `OpenLogos/` (B063): enough app to prove the simulator and signed-build
lanes, not the reader. B062 builds the reader in its place.

## Environment

- `nix develop .#apple`: Rust with `aarch64-apple-darwin`, `aarch64-apple-ios` and
  `aarch64-apple-ios-sim` targets, plus the shared repo tools. On entry it runs
  `scripts/xcode-check.sh`, which compares `xcodebuild -version` with `ops/xcode-version.txt`
  and warns on mismatch. Xcode, Apple SDKs and simulator runtimes are external licensed
  prerequisites; the shell verifies them and does not claim hermeticity.
- The shell carries no Nix compiler and drops a Nix `DEVELOPER_DIR`/`SDKROOT` so Xcode's own
  toolchain links Apple targets.

## OpenLogos: the app shell (B063)

`OpenLogos/project.yml` is the project; XcodeGen (in the apple shell) writes
`OpenLogos.xcodeproj` and `Generated/` from it, both gitignored. Targets: `OpenLogos` (iOS and
iPadOS 18+, bundle id `org.openlogos.app`, device family iPhone and iPad) and `OpenLogosMac`
(macOS 15+, same sources, unsigned local run). Both compile the generated
`artifacts/apple/generated/OpenLogosCore.swift` into the app module and link
`artifacts/apple/OpenLogosCoreFFI.xcframework`, exactly as `scripts/apple-bindings.sh` leaves
them: one copy of the bindings, no import statement in app code (CoreHost, a package, imports
`OpenLogosCore` instead).

The one screen, `ReadingShellView`, is a reading surface: the fixture texts the core ships
through `conformanceFixturesJson()` (Genesis 1:1–2 from the public-domain WLC, right to left
with points and cantillation untouched; John 3:16–17 from the CC0 Berean Standard Bible; the
bidi study-note fixture), set in the system serif with Dynamic Type, a 620-point measure and
scaled leading, monochrome, no chrome; under them a reference field over `parseReference`
showing the OSIS and human forms, and `apiVersion()`. No fonts are downloaded or bundled yet;
specs/16-design-guide.md's open-license book serif is B004/B062 work.

```sh
just sim                      # bindings if missing, xcodegen, unsigned build, boot the iPad
                              # Pro 11-inch (M5) simulator, install, launch
just sim "iPad mini (A17 Pro)"
just sim-shot                 # artifacts/sim/shots/<utc>.png
OPENLOGOS_SIM_TEXT_SIZE=UICTContentSizeCategoryAccessibilityL just sim   # large type
just sim-stop
just preview-mac              # Debug macOS app, unsigned, opened for hand review
just feedback "note"          # drag a region of the screen; shot and note in ~/OpenLogosFeedback/<utc>/
```

A first boot of a simulator writes about 2.7 GB of device data under
`~/Library/Developer/CoreSimulator/Devices/` and can take ten minutes; `xcrun simctl erase
<udid>` gives the space back. Logs: `artifacts/sim/build.log`.

### Signed builds onto the shelf

```sh
cp .envrc.local.example .envrc.local   # APPLE_TEAM_ID; optional ASC_* key ids and paths; shelf settings
just shelf-install                     # once: launchd agent serving ~/OpenLogosShelf on 127.0.0.1:8767,
                                       # `tailscale serve --https=8767` sharing it with the tailnet
just ship-check                        # everything the lane needs, with the fix for each
just ship ios                          # archive (automatic signing, team from .envrc.local), export
                                       # with method release-testing (ad hoc), publish
```

`scripts/ship.sh` writes `builds/<number>/OpenLogos.ipa` and an itms-services `manifest.plist`,
updates `shelf.json` and `index.html`, and keeps the newest ten builds. The build number is
the UTC time (`yyjjjHHMM`), the marketing version comes from `project.yml`, nothing is
committed by the script; a dirty tree is marked `+`. Open
`https://<mac>.<tailnet>.ts.net:8767/` in Safari on a device registered with the team, with
Tailscale on, and tap Install. The shelf is static files behind `/usr/bin/python3 -m
http.server`; sudoku-color's shelf (a .NET tool on port 443 of the same Mac) is not shared.
`just shelf-uninstall` removes the agent and only this port's Serve entry.

## The TextKit 2 reading surface (B005)

`OpenLogos/Sources/Reader/` is the reading surface the panes use since B005: a `UITextView`
(iPadOS/iOS) or `NSTextView` (macOS) created with `NSTextLayoutManager`, fed an attributed
display string built from the fixture verses (`ReaderLayout`), with every selection change
turned into core anchors through `selectionToAnchors` and painted back through
`anchorToUtf16` (`AnchorRoundTrip`). Verse numbers and the attribution are non-core runs that
`RunMap` maps out of a selection. The project actions (Highlight, Collect, Study, More) sit in
the native edit menu; a VoiceOver rotor "Verses" walks the markers on iOS. The "B005 anchors"
view of the prototype shows three texts and the conformance study note in the notes dock, and
launch arguments drive it (see `Sources/Prototype/PrototypeOptions.swift`):

```sh
just sim                                    # build, boot, install, launch on the iPad simulator
just b005-shot ipad cross-verse 1024        # one driven case, cropped picture in artifacts/b005/shots
just test-reader                            # the round-trip unit tests in the app on the simulator
just test-reader mac                        # the same natively in the macOS app (it builds now)
```

`Tests/ReaderTests` pushes each `AnchorCase` through the real text view on both platforms;
`Tests/UITests` runs the XCUITest accessibility audit on the simulator. The evidence, with the
measurements and what the two text views do with a range inside a grapheme cluster, is
docs/evidence/B005.md.

## The ink margin (B039)

`OpenLogos/Sources/Ink/` is the B039 spike: a margin sheet beside the B005 reader (prototype
view "B039 ink margin", `-OPENLOGOS_VIEW ink`). `InkCanvasEngine` keeps one `CAShapeLayer` per
stroke whose path is the core's `inkOutline` (the perfect-freehand 1.2.3 port in crates/ink,
through UniFFI; no Swift port) filled once with the non-zero rule at the stroke's opacity, so a
highlighter never darkens over itself. `InkSheet` holds the UniFFI `InkDocument` record with
undo/redo and stroke-eraser tombstones and exports it through `inkDocumentJson`.
`InkTouchPolicy` mirrors the palm and finger/stylus arbiter of `crates/ink/src/touch.rs`
(same cases in `Tests/ReaderTests/InkTests.swift`). The UIKit view takes coalesced touches,
Pencil force, altitude and azimuth (the ink-document 1.1 tilt fields, null for a finger)
and `majorRadius` for the palm rule; the AppKit view takes mouse and tablet events.
`PencilKitProbe` is a `PKCanvasView` whose strokes are exported into the same record with every
PencilKit limitation named on screen. Launch arguments: `-OPENLOGOS_INK_DEMO 1`,
`-OPENLOGOS_INK_MEASURE 1` (in-process synthesized latency run), `-OPENLOGOS_INK_PENCILKIT 1`,
`-OPENLOGOS_INK_PKDEMO 1`, `-OPENLOGOS_INK_PALM_RADIUS <pt>`, `-OPENLOGOS_LOG 1`.

```sh
just sim && scripts/b039-shots.sh ipad ipad-ink-1024-measure 1024 1 -OPENLOGOS_INK_DEMO 1 -OPENLOGOS_LOG 1
scripts/b039-shots.sh mac mac-ink-1440-measure 1440 1 -OPENLOGOS_INK_DEMO 1   # run outside the Nix shell (BSD stat)
xcodebuild … -only-testing:OpenLogosTests/InkTests test        # unit cases; OpenLogosUITests/InkUITests for the XCUITest drags
```

The measurements, the PencilKit export gaps and the Excalidraw comparison are docs/evidence/B039.md.

## The workspace shell (B072)

`OpenLogos/Sources/Shell/` is the Mac app's view (specs/22): a SwiftUI shell around the B005
reading surface, driven by the UniFFI `ShellHandle` (crates/workspaces through crates/ffi).
The palette field sits in the window's toolbar row beside the traffic lights, with the
workspace switcher and the text menu; below it the numbered columns (tab strip, identity row,
text capped at the core's measure from `layoutConstants()`), and the floating tool well. Every
button, menu item and shortcut dispatches a registered command through `ShellModel.run`;
shortcuts are read from the registry's Mac list (`ShellKeymap`). Reading settings, ink width
and swatches, column widths and scroll anchors are session state in the core
(`session.reading`, `session.ink`, `column.resize`, `tab.record_scroll`); the host keeps only
the palette's field, the well's place, highlights, ink strokes and toasts. `ShellTokens`
carries the values of design/tokens/shell.json; `TextSource` puts the conformance fixture
texts behind one interface for B071's pack. The B004 prototype is the Debug menu's
"B004 prototype" item, and opens directly when `-OPENLOGOS_VIEW` names a prototype view.

Launch arguments (`ShellLaunch.swift`): `-OPENLOGOS_WINDOW 1440x900`, `-OPENLOGOS_THEME`,
`-OPENLOGOS_PALETTE "text"` with `-OPENLOGOS_PALETTE_EXPAND <section>`, `-OPENLOGOS_TOOL`,
`-OPENLOGOS_RUN "cmd,cmd:arg"`, `-OPENLOGOS_SHELL_DEMO 1`, and two that quit when done:

- `-OPENLOGOS_SNAPSHOT <png>`: the app draws its own window (the frame view, toolbar row
  included, through `NSView.cacheDisplay`) into a PNG after `-OPENLOGOS_SNAPSHOT_DELAY`
  seconds. No screen-recording permission is needed, so it works where `screencapture -l`
  cannot (an agent session without the grant) and never catches another window or dialog.
- `-OPENLOGOS_SHELL_MEASURE <json>`: the specs/22 §7 interactions over ten workspaces of three
  columns × four tabs, one per display-link frame, each timed through a synchronous layout and
  display of the window.

```sh
just preview-mac                     # Debug build into artifacts/mac, opens the shell
scripts/b072-mac-shots.sh            # the evidence pictures into artifacts/b072/shots/
scripts/b072-mac-shots.sh measure    # Release build in artifacts/mac-release first; JSON per width
just test-reader mac                 # Tests/ReaderTests and Tests/ShellTests in the Mac app
```

## The contextual tool well (B076)

The well is drawn from the core's `toolWell(context:)` for what has focus (`ShellModel.focus`:
the active column's tab, or the notes dock's editor) and whether text is selected; every
command goes through `applyIn` / `commandsIn` with that context. Translation, original
(Word study), notes (Type and the note actions) and reference tabs each get their tools; the
ink group shows on Bible text and the selection group below it while text is selected, with
the well's top kept in place. Notes tabs and the dock are `NSTextView` editors in memory
(`Sources/Shell/NoteEditor.swift`), Word study opens the clicked word in a Lexicon tab
(`tab.open_lexicon`, `Sources/Shell/LexiconView.swift`). Extra launch arguments:
`-OPENLOGOS_SELECT "2:the world"`, `-OPENLOGOS_WORD "2:ἠγάπησεν"`, `-OPENLOGOS_NOTE_DEMO 1`,
`-OPENLOGOS_FOCUS dock`.

```sh
scripts/b076-mac-shots.sh            # the B076 pictures into docs/evidence/B076/mac-*.png
scripts/b076-mac-shots.sh measure    # the §7 run with the well rows (Release, ARCHS=arm64)
```

## CoreHost: the UniFFI Swift bindings test host (B061)

`CoreHost/` is a SwiftPM package for macOS, not the app. It proves that the Swift bindings
generated from `crates/ffi` link against the Rust core and give the same answers as the Rust
tests: `apiVersion()`, `parseReference("Jn 3:16, 18; Rom 8:28-30")`, every fixture selection as
a UTF-16 round trip through `selectionToAnchors` and `anchorToUtf16`, `mapVerse` on the Psalm
9/10 table, `reanchor`, and `runConformance().failed == 0` (1337 cases).

```sh
nix develop .#apple
just bindings-swift   # release builds for macOS arm64, iOS and the simulator; uniffi-bindgen;
                      # artifacts/apple/OpenLogosCoreFFI.xcframework
just test-swift       # the same, then `swift test` in apps/apple/CoreHost
```

What `scripts/apple-bindings.sh` does, and what is checked in:

- `cargo build -p openlogos-ffi --release --target <t>` for `aarch64-apple-darwin`,
  `aarch64-apple-ios` and `aarch64-apple-ios-sim`, linking with Xcode's clang and each target's
  SDK (`xcrun --sdk ... --show-sdk-path`). The Nix shell's `cc` is a Nix clang wrapper that
  links against Nix's macOS SDK; the simulator rejects that, so the script sets
  `CARGO_TARGET_<T>_LINKER` and `SDKROOT` itself.
- `cargo run -p openlogos-ffi --features bindgen --bin uniffi-bindgen -- generate --library
  target/aarch64-apple-darwin/release/libopenlogos_ffi.dylib --language swift`: library mode,
  no UDL; `crates/ffi/uniffi.toml` names the modules `OpenLogosCore` and `OpenLogosCoreFFI`.
- `xcodebuild -create-xcframework` with the three static libraries and the generated header
  and module map, into `artifacts/apple/` (gitignored), linked into `CoreHost/Frameworks/`.
- The generated `OpenLogosCore.swift` is copied to `CoreHost/Sources/OpenLogosCore/` (gitignored).
  Committed: `Package.swift` and `Tests/CoreHostTests/CoreHostTests.swift`.

The app shell consumes the same XCFramework and Swift file straight from `artifacts/apple/`
(above). `docs/evidence/B061-bindings.md` records sizes and timings.

## Signing prerequisites (documented, not provided)

Building for a simulator needs no signing. Device builds, TestFlight and App Store releases need:

1. An Apple Developer Program membership and an App ID for the app.
2. A development or distribution certificate and matching provisioning profiles.
3. An App Store Connect API key for automated uploads.

Per specs/20-orchestration-and-build-hosts.md these live in a separate macOS user and keychain
used only by the trusted release workflow after human approval. Agent runs and pull-request CI
must not be able to read them. `just signing-isolation-check` (`scripts/signing-isolation-check.sh`)
is the negative test: it reports whether the current user can read a code-signing identity,
an App Store Connect key or a provisioning profile and exits 1 if so. On the Mac mini today it
exits 1 for the owner's account, which is the account agents still run as; the isolated user
and keychain are not yet in place (docs/evidence/B063.md). Nothing in this repository, in Nix
store paths or in `.envrc.local` may contain signing material; `.envrc.local` holds the team ID
and, optionally, the key's id and path.
