# apps/apple

One SwiftUI application for iPadOS, macOS and iOS with a TextKit 2 reading surface
(docs/adr/0001-rust-core-native-ui.md, specs/18-platform-capabilities.md). No project exists yet.

## Environment

- `nix develop .#apple`: Rust with `aarch64-apple-darwin`, `aarch64-apple-ios` and
  `aarch64-apple-ios-sim` targets, plus the shared repo tools. On entry it runs
  `scripts/xcode-check.sh`, which compares `xcodebuild -version` with `ops/xcode-version.txt`
  and warns on mismatch. Xcode, Apple SDKs and simulator runtimes are external licensed
  prerequisites; the shell verifies them and does not claim hermeticity.
- The shell carries no Nix compiler and drops a Nix `DEVELOPER_DIR`/`SDKROOT` so Xcode's own
  toolchain links Apple targets.

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

The app will consume the same XCFramework and Swift file; where they live in the Xcode project
is B062's decision. `docs/evidence/B061-bindings.md` records sizes and timings.

## Signing prerequisites (documented, not provided)

Building for a simulator needs no signing. Device builds, TestFlight and App Store releases need:

1. An Apple Developer Program membership and an App ID for the app.
2. A development or distribution certificate and matching provisioning profiles.
3. An App Store Connect API key for automated uploads.

Per specs/20-orchestration-and-build-hosts.md these live in a separate macOS user and keychain
used only by the trusted release workflow after human approval. Agent runs and pull-request CI
must not be able to read them; B063 includes a negative test proving that. Nothing in this
repository, in Nix store paths or in `.envrc.local` may contain them.
