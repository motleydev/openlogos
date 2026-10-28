#!/usr/bin/env bash
# B064 benchmark R3: what the embedded retrieval engine adds to the binding library.
# Builds crates/ffi as a release dylib (the workspace profile: opt-level z, fat LTO, stripped)
# for each Apple target twice, with the default features and with `--features retrieval`,
# and prints both sizes; the difference is the engine's contribution (bundled SQLite with
# FTS5 and the openlogos tokenizer, roaring, sqlite-vec, the engine and link index).
# Run inside `nix develop .#core` or `.#apple` on a Mac with Xcode (docs/evidence/B064.md).
#
#   scripts/retrieval-sizes.sh
#   OPENLOGOS_SIZE_TARGETS="aarch64-apple-darwin" scripts/retrieval-sizes.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

targets="${OPENLOGOS_SIZE_TARGETS:-aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim}"

# Xcode's clang must link and compile every Apple slice (the Nix clang wrapper on PATH
# would use Nix's macOS SDK, which the iOS targets reject); same rule as apple-bindings.sh,
# extended to the C compiler because this build has C sources (SQLite, sqlite-vec).
xcode_clang="$(xcrun -f clang)"
export CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER="$xcode_clang"
export CARGO_TARGET_AARCH64_APPLE_IOS_LINKER="$xcode_clang"
export CARGO_TARGET_AARCH64_APPLE_IOS_SIM_LINKER="$xcode_clang"
export CC_aarch64_apple_darwin="$xcode_clang"
export CC_aarch64_apple_ios="$xcode_clang"
export CC_aarch64_apple_ios_sim="$xcode_clang"

sdk_for() {
  case "$1" in
    *-apple-darwin) echo macosx ;;
    *-apple-ios-sim) echo iphonesimulator ;;
    *-apple-ios) echo iphoneos ;;
    *) echo "no Apple SDK for target $1" >&2; exit 1 ;;
  esac
}
size_of() { wc -c < "$1" | tr -d ' '; }

echo "rustc $(rustc --version | awk '{print $2}'), Xcode clang: $("$xcode_clang" --version | head -1)"
echo
echo "| target | libopenlogos_ffi.dylib, default features | with \`retrieval\` | engine contribution | build s (default / retrieval) |"
echo "|---|---:|---:|---:|---:|"
for target in $targets; do
  sdk="$(xcrun --sdk "$(sdk_for "$target")" --show-sdk-path)"
  dylib="$root/target/$target/release/libopenlogos_ffi.dylib"
  SECONDS=0
  SDKROOT="$sdk" cargo build -q -p openlogos-ffi --release --target "$target"
  base_s=$SECONDS
  base="$(size_of "$dylib")"
  SECONDS=0
  SDKROOT="$sdk" cargo build -q -p openlogos-ffi --release --target "$target" --features retrieval
  retrieval_s=$SECONDS
  with="$(size_of "$dylib")"
  printf '| %s | %s | %s | %s | %s / %s |\n' "$target" "$base" "$with" "$((with - base))" "$base_s" "$retrieval_s"
done
