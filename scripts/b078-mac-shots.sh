#!/usr/bin/env bash
# B078 evidence pictures of continuous reading in the Mac shell (docs/evidence/B078-mac.md),
# reproducibly, and its speed run.
#
#   scripts/b078-mac-shots.sh              every picture into docs/evidence/B078/mac-read-*.png
#   scripts/b078-mac-shots.sh measure      the specs/22 §7 run with the B078 rows (Release build)
#                                          into artifacts/b078/measure-<width>.json
#
# As scripts/b077-mac-shots.sh: the app draws its own window into the PNG (-OPENLOGOS_SNAPSHOT)
# and quits. -OPENLOGOS_TYPE types a passage in the palette and runs its row after the columns
# have laid out; -OPENLOGOS_SCROLL scrolls a column as the reader would. Build first with
# `just test-reader mac` (Debug, artifacts/b005) and, for `measure`, a Release build into
# artifacts/b078/release (ARCHS=arm64).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
out_dir="$root/docs/evidence/B078"
mkdir -p "$out_dir" "$root/artifacts/b078"

shot() { # name args…
  local name="$1"; shift
  local app="$root/artifacts/b005/DerivedData/Build/Products/Debug/OpenLogos.app"
  local out="$out_dir/$name.png"
  rm -f "$out"
  pkill -f "$app/Contents/MacOS/OpenLogos" 2>/dev/null || true
  sleep 0.5
  open -n -W "$app" --args -OPENLOGOS_WINDOW 1440x900 -OPENLOGOS_SNAPSHOT "$out" "$@"
  printf '%s  %s KB\n' "$out" "$(( $(stat -f %z "$out") / 1024 ))"
}

measure() { # width height
  local app="$root/artifacts/b078/release/Build/Products/Release/OpenLogos.app"
  local out="$root/artifacts/b078/measure-$1.json"
  rm -f "$out"
  open -n -W "$app" --args -OPENLOGOS_WINDOW "$1x$2" -OPENLOGOS_SHELL_MEASURE "$out"
  echo "$out"
}

case "${1:-shots}" in
  shots)
    # The flash at its peak, 0.3 s after the jump; the SBLGNT column followed.
    shot mac-read-jump-john-3-16 -OPENLOGOS_TYPE "John 3:16" -OPENLOGOS_SNAPSHOT_DELAY 0.3
    shot mac-read-john-acts-boundary -OPENLOGOS_TYPE "Acts 1:1" -OPENLOGOS_SCROLL "1:-160" -OPENLOGOS_SNAPSHOT_DELAY 0.6
    shot mac-read-linked-mid-scroll -OPENLOGOS_TYPE "John 3:1" -OPENLOGOS_SCROLL "1:520" -OPENLOGOS_SNAPSHOT_DELAY 0.6
    shot mac-read-psalm-3 -OPENLOGOS_TYPE "Ps 3" -OPENLOGOS_SNAPSHOT_DELAY 1.2
    shot mac-read-not-in-edition -OPENLOGOS_TYPE "John 22" -OPENLOGOS_SNAPSHOT_DELAY 0.8
    shot mac-read-dark-range -OPENLOGOS_THEME dark -OPENLOGOS_TYPE "John 3:16-18" -OPENLOGOS_SNAPSHOT_DELAY 1.4
    shot mac-read-placeholder-menu -OPENLOGOS_TYPE "John 3:17" -OPENLOGOS_MENU "selection:1:verse — John 3:19"
    shot mac-read-genesis-wlc -OPENLOGOS_RUN "tab.next" -OPENLOGOS_TYPE "Genesis" -OPENLOGOS_SNAPSHOT_DELAY 1.2
    ;;
  measure)
    measure 1440 900
    measure 1024 768
    ;;
  *) echo "usage: $0 [shots|measure]" >&2; exit 2 ;;
esac
