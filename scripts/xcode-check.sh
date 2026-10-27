#!/usr/bin/env bash
# Verifies that the selected Xcode is the version recorded in ops/xcode-version.txt.
# Used by the `.#apple` shell hook and by `just doctor`. Nix cannot provide Xcode, Apple
# SDKs or simulator runtimes (specs/06-nix-operations.md), so this check is the honest
# substitute for pinning them: exit 0 when the versions match, 1 with a clear message
# otherwise. It only reads; it never changes the selected Xcode.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
record="$root/ops/xcode-version.txt"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "xcode-check: not macOS; Apple builds need a Mac (specs/20-orchestration-and-build-hosts.md)." >&2
  exit 1
fi

if [[ ! -f "$record" ]]; then
  echo "xcode-check: $record is missing; record the Xcode version the project builds with." >&2
  exit 1
fi
want="$(tr -d '[:space:]' <"$record")"

if ! command -v xcodebuild >/dev/null 2>&1; then
  echo "xcode-check: xcodebuild not found. Install Xcode $want from https://developer.apple.com/download/ and run 'sudo xcode-select -s /Applications/Xcode.app'." >&2
  exit 1
fi

# "Xcode 26.6" / "Build version 17F113". A Command Line Tools-only selection fails here
# with its own message, which is the right outcome: the app needs full Xcode.
if ! output="$(xcodebuild -version 2>&1)"; then
  echo "xcode-check: xcodebuild failed:" >&2
  echo "$output" >&2
  echo "xcode-check: select full Xcode $want with 'sudo xcode-select -s /Applications/Xcode.app'." >&2
  exit 1
fi
got="$(printf '%s\n' "$output" | awk '/^Xcode/ { print $2; exit }')"
build="$(printf '%s\n' "$output" | awk '/^Build version/ { print $3; exit }')"
path="$(xcode-select -p 2>/dev/null || echo unknown)"

if [[ "$got" == "$want" ]]; then
  echo "xcode-check: Xcode $got ($build) at $path matches ops/xcode-version.txt."
  exit 0
fi

cat >&2 <<EOF
xcode-check: WARNING: selected Xcode is $got ($build) at $path, but ops/xcode-version.txt records $want.
  Builds and simulator runs may differ from CI and from the reviewed evidence.
  Fix: install Xcode $want and 'sudo xcode-select -s /Applications/Xcode-$want.app',
  or, if $got is the new project standard, update ops/xcode-version.txt in its own commit.
EOF
exit 1
