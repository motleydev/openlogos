#!/usr/bin/env bash
# `just bindings-swift` / `just test-swift`: builds crates/ffi for the Apple targets, generates
# the Swift bindings with this workspace's uniffi-bindgen, assembles an XCFramework under
# artifacts/apple/ and runs the Swift test host in apps/apple/CoreHost. Run it inside
# `nix develop .#apple` (Rust with the Apple targets; Xcode supplies the linker and SDKs).
#
#   scripts/apple-bindings.sh            build, generate, assemble and test
#   scripts/apple-bindings.sh --no-test  everything but `swift test`
#   OPENLOGOS_APPLE_TARGETS="aarch64-apple-darwin" scripts/apple-bindings.sh   host slice only
#
# Everything it writes is gitignored: artifacts/apple/, the XCFramework link inside the host
# package and the generated Sources/OpenLogosCore/OpenLogosCore.swift.
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

case "$(uname -m)" in
  arm64) host_target="aarch64-apple-darwin" ;;
  x86_64) host_target="x86_64-apple-darwin" ;;
  *) echo "unsupported Mac architecture: $(uname -m)" >&2; exit 1 ;;
esac
# The host slice first: the XCFramework needs it for `swift test` on this Mac and bindgen
# reads the metadata out of its dylib. The iOS slices prove the cross builds.
targets="${OPENLOGOS_APPLE_TARGETS:-$host_target aarch64-apple-ios aarch64-apple-ios-sim}"

out="$root/artifacts/apple"
generated="$out/generated"
headers="$out/include"
xcframework="$out/OpenLogosCoreFFI.xcframework"
host_pkg="$root/apps/apple/CoreHost"

step() { printf '\n==> %s\n' "$*"; }
size_of() { wc -c < "$1" | tr -d ' '; }

# The Nix shell puts a Nix clang wrapper on PATH as `cc` (the rust-overlay toolchain
# propagates it); it links every Apple target against Nix's macOS SDK, which the iOS
# simulator rejects. Xcode's clang with the SDK of each target does it right.
xcode_clang="$(xcrun -f clang)"
export CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER="$xcode_clang"
export CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER="$xcode_clang"
export CARGO_TARGET_AARCH64_APPLE_IOS_LINKER="$xcode_clang"
export CARGO_TARGET_AARCH64_APPLE_IOS_SIM_LINKER="$xcode_clang"
sdk_for() {
  case "$1" in
    *-apple-darwin) echo macosx ;;
    *-apple-ios-sim) echo iphonesimulator ;;
    *-apple-ios) echo iphoneos ;;
    *) echo "no Apple SDK for target $1" >&2; exit 1 ;;
  esac
}

step "Building openlogos-ffi (release) for: $targets"
library_args=()
for target in $targets; do
  SECONDS=0
  SDKROOT="$(xcrun --sdk "$(sdk_for "$target")" --show-sdk-path)" \
    cargo build -p openlogos-ffi --release --target "$target"
  archive="$root/target/$target/release/libopenlogos_ffi.a"
  printf '   %s: %s bytes (%ss)\n' "$target" "$(size_of "$archive")" "$SECONDS"
  library_args+=(-library "$archive" -headers "$headers")
done

dylib="$root/target/$host_target/release/libopenlogos_ffi.dylib"
if [[ ! -f "$dylib" ]]; then
  echo "no host dylib at $dylib: OPENLOGOS_APPLE_TARGETS must include $host_target" >&2
  exit 1
fi

step "Generating Swift bindings from $dylib"
rm -rf "$generated" "$headers"
cargo run -q -p openlogos-ffi --features bindgen --bin uniffi-bindgen -- \
  generate --library "$dylib" --language swift --out-dir "$generated"
# The header and module map become the XCFramework's public headers; Clang only picks a
# module map up under the name module.modulemap.
mkdir -p "$headers"
cp "$generated/OpenLogosCoreFFI.h" "$headers/"
cp "$generated/OpenLogosCoreFFI.modulemap" "$headers/module.modulemap"
mkdir -p "$host_pkg/Sources/OpenLogosCore"
cp "$generated/OpenLogosCore.swift" "$host_pkg/Sources/OpenLogosCore/OpenLogosCore.swift"
printf '   %s\n' "$generated/OpenLogosCore.swift ($(size_of "$generated/OpenLogosCore.swift") bytes)"

step "Assembling $xcframework"
rm -rf "$xcframework"
xcodebuild -create-xcframework "${library_args[@]}" -output "$xcframework" | sed 's/^/   /'
du -sh "$xcframework" | sed 's/^/   /'
# SwiftPM wants the binary target inside the package; a link keeps one copy in artifacts/.
mkdir -p "$host_pkg/Frameworks"
ln -sfn "$xcframework" "$host_pkg/Frameworks/OpenLogosCoreFFI.xcframework"

if ((run_tests)); then
  step "swift test (apps/apple/CoreHost)"
  SECONDS=0
  swift test --package-path "$host_pkg"
  printf '   swift test: %ss\n' "$SECONDS"
fi
