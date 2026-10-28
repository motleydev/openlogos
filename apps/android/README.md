# apps/android

Kotlin + Jetpack Compose shell with a platform text surface and extended selection
(docs/adr/0001-rust-core-native-ui.md). No project exists yet.

## Environment

`nix develop .#android` pins JDK 21, Gradle and `cargo-ndk`, and Rust with the
`aarch64-linux-android` target. It does **not** provide the Android SDK or NDK:

- nixpkgs' `androidenv.composeAndroidPackages` would make the SDK, build tools, platform and NDK
  reproducible, but its closure is several gigabytes and the host this skeleton was built on had
  no room for it; the SDK also carries its own license terms that must be accepted explicitly
  (specs/06-nix-operations.md). Revisit when the Mac mini build host is provisioned (B063).
- Until then install the SDK and an NDK outside Nix (Android Studio or `sdkmanager`), accept the
  licenses, and export the paths in `.envrc.local`:

  ```sh
  export ANDROID_HOME="$HOME/Library/Android/sdk"
  export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/<version>"
  ```

  The shell warns on entry when either is missing, and `just doctor` reports both.

## CoreHost: the UniFFI Kotlin bindings test host (B061)

`CoreHost/` is a plain JVM Gradle project, not the app and not an Android module. It proves
that the Kotlin bindings generated from `crates/ffi` load the Rust core through JNA and give
the same answers as the Rust and Swift tests: `apiVersion()`, `parseReference("Jn 3:16, 18;
Rom 8:28-30")`, every fixture selection as a UTF-16 round trip through `selectionToAnchors`
and `anchorToUtf16`, `mapVerse` on the Psalm 9/10 table, `reanchor`, and
`runConformance().failed == 0u` (576 cases). It needs neither the Android SDK nor the NDK.

```sh
nix develop .#android
just bindings-kotlin  # release build of libopenlogos_ffi.dylib for this Mac; uniffi-bindgen
                      # into apps/android/CoreHost/generated/kotlin (gitignored)
just test-kotlin      # the same, then `gradle test` in apps/android/CoreHost
```

What `scripts/kotlin-bindings.sh` does, and what is checked in:

- `cargo build -p openlogos-ffi --release --target aarch64-apple-darwin` (Xcode's clang on a
  Mac, as `scripts/apple-bindings.sh`). When `ANDROID_NDK_HOME` is set it also runs
  `cargo ndk --target arm64-v8a --platform 26 -- build -p openlogos-ffi --release` for the
  `aarch64-linux-android` `.so` the app will ship; without it that step is reported as skipped.
- `cargo run -p openlogos-ffi --features bindgen --bin uniffi-bindgen -- generate --library
  <dylib> --language kotlin`: library mode, package `org.openlogos.core`
  (`crates/ffi/uniffi.toml`).
- `gradle test -PopenlogosLibDir=<dir of the dylib>`; the build sets `jna.library.path` from it.
  Committed: `settings.gradle.kts`, `build.gradle.kts` (Kotlin 2.2.0, JNA 5.17.0, JUnit 5.12.2,
  org.json for reading the fixtures), `gradle.properties`, `gradle.lockfile` (every
  configuration locked) and `src/test/kotlin/.../CoreHostTest.kt`. Gradle itself and JDK 21
  come from the Nix shell, so there is no wrapper.

The Android app will use the same generated Kotlin with the `.so` from `cargo ndk` in
`jniLibs`; `docs/evidence/B061-bindings.md` records sizes and timings.

Release signing keys stay outside the repository and outside agent runs, as for Apple.
