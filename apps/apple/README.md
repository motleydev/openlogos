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

## CoreHost: the UniFFI Swift bindings test host (B061)

`CoreHost/` is a SwiftPM package for macOS, not the app. It proves that the Swift bindings
generated from `crates/ffi` link against the Rust core and give the same answers as the Rust
tests: `apiVersion()`, `parseReference("Jn 3:16, 18; Rom 8:28-30")`, every fixture selection as
a UTF-16 round trip through `selectionToAnchors` and `anchorToUtf16`, `mapVerse` on the Psalm
9/10 table, `reanchor`, and `runConformance().failed == 0` (512 cases).

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
