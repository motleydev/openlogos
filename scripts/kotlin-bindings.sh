#!/usr/bin/env bash
# `just bindings-kotlin` / `just test-kotlin`: builds crates/ffi as a shared library for this
# Mac, generates the Kotlin bindings with this workspace's uniffi-bindgen and runs the JVM
# test host in apps/android/CoreHost with Gradle. Run it inside `nix develop .#android`
# (JDK 21, Gradle, cargo-ndk). The Android SDK is not needed: the host is a plain JVM project
# that loads the macOS dylib through JNA, which is what the Android app will do with the
# `.so` built by `cargo ndk` once ANDROID_NDK_HOME is set (apps/android/README.md).
#
#   scripts/kotlin-bindings.sh            build, generate and test
#   scripts/kotlin-bindings.sh --no-test  everything but `gradle test`
#
# Everything it writes is gitignored: the generated Kotlin under the host's generated/ and
# Gradle's build/ and .gradle/.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

run_tests=1
for arg in "$@"; do
  case "$arg" in
    --no-test) run_tests=0 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) host_target="aarch64-apple-darwin"; ext="dylib" ;;
  Darwin-x86_64) host_target="x86_64-apple-darwin"; ext="dylib" ;;
  Linux-x86_64) host_target="x86_64-unknown-linux-gnu"; ext="so" ;;
  Linux-aarch64) host_target="aarch64-unknown-linux-gnu"; ext="so" ;;
  *) echo "unsupported host: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

host_pkg="$root/apps/android/CoreHost"
generated="$host_pkg/generated/kotlin"
lib_dir="$root/target/$host_target/release"
dylib="$lib_dir/libopenlogos_ffi.$ext"

step() { printf '\n==> %s\n' "$*"; }
size_of() { wc -c < "$1" | tr -d ' '; }

# On a Mac the Nix shell's `cc` is a Nix clang wrapper that links against Nix's macOS SDK;
# use Xcode's clang and SDK as scripts/apple-bindings.sh does, so both scripts produce the
# same host dylib and never relink each other's output.
if [[ "$(uname -s)" == Darwin ]] && command -v xcrun >/dev/null; then
  linker_var="CARGO_TARGET_$(echo "$host_target" | tr '[:lower:]-' '[:upper:]_')_LINKER"
  export "$linker_var=$(xcrun -f clang)"
  SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
  export SDKROOT
fi

step "Building openlogos-ffi (release) for $host_target"
SECONDS=0
cargo build -p openlogos-ffi --release --target "$host_target"
printf '   %s: %s bytes (%ss)\n' "$dylib" "$(size_of "$dylib")" "$SECONDS"

if [[ -n "${ANDROID_NDK_HOME:-}" ]]; then
  step "Building openlogos-ffi for aarch64-linux-android with cargo ndk"
  cargo ndk --target arm64-v8a --platform 26 -- build -p openlogos-ffi --release
  so="$root/target/aarch64-linux-android/release/libopenlogos_ffi.so"
  printf '   %s: %s bytes\n' "$so" "$(size_of "$so")"
else
  step "Skipping the aarch64-linux-android build: ANDROID_NDK_HOME is not set (apps/android/README.md)"
fi

step "Generating Kotlin bindings from $dylib"
rm -rf "$generated"
cargo run -q -p openlogos-ffi --features bindgen --bin uniffi-bindgen -- \
  generate --library "$dylib" --language kotlin --out-dir "$generated"
find "$generated" -name '*.kt' -exec sh -c 'printf "   %s (%s bytes)\n" "$1" "$(wc -c < "$1" | tr -d " ")"' _ {} \;

if ((run_tests)); then
  step "gradle test (apps/android/CoreHost)"
  SECONDS=0
  (cd "$host_pkg" && gradle --console=plain --no-daemon test "-PopenlogosLibDir=$lib_dir")
  printf '   gradle test: %ss\n' "$SECONDS"
fi
