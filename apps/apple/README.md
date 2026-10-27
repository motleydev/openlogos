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

## Signing prerequisites (documented, not provided)

Building for a simulator needs no signing. Device builds, TestFlight and App Store releases need:

1. An Apple Developer Program membership and an App ID for the app.
2. A development or distribution certificate and matching provisioning profiles.
3. An App Store Connect API key for automated uploads.

Per specs/20-orchestration-and-build-hosts.md these live in a separate macOS user and keychain
used only by the trusted release workflow after human approval. Agent runs and pull-request CI
must not be able to read them; B063 includes a negative test proving that. Nothing in this
repository, in Nix store paths or in `.envrc.local` may contain them.
