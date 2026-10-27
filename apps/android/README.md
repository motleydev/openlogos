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

Release signing keys stay outside the repository and outside agent runs, as for Apple.
