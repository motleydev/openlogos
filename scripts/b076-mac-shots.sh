#!/usr/bin/env bash
# B076 evidence pictures of the contextual tool well in the Mac shell
# (docs/evidence/B076-mac.md), reproducibly, and its speed run.
#
#   scripts/b076-mac-shots.sh              every picture into docs/evidence/B076/mac-*.png
#   scripts/b076-mac-shots.sh measure      the specs/22 §7 run (Release build) into
#                                          artifacts/b076/measure-<width>.json
#
# As scripts/b072-mac-shots.sh: the app draws its own window into the PNG
# (-OPENLOGOS_SNAPSHOT) and quits. Build first with `just preview-mac` (Debug, artifacts/mac)
# and, for `measure`, the Release build (artifacts/mac-release, ARCHS=arm64).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
out_dir="$root/docs/evidence/B076"
mkdir -p "$out_dir" "$root/artifacts/b076"

shot() { # name args…
  local name="$1"; shift
  local app="$root/artifacts/mac/DerivedData/Build/Products/Debug/OpenLogos.app"
  local out="$out_dir/$name.png"
  rm -f "$out"
  pkill -f "$app/Contents/MacOS/OpenLogos" 2>/dev/null || true
  sleep 0.5
  open -n -W "$app" --args -OPENLOGOS_WINDOW 1440x900 -OPENLOGOS_SNAPSHOT "$out" -OPENLOGOS_SNAPSHOT_DELAY 1.5 "$@"
  printf '%s  %s KB\n' "$out" "$(( $(stat -f %z "$out") / 1024 ))"
}

measure() { # width height
  local app="$root/artifacts/mac-release/DerivedData/Build/Products/Release/OpenLogos.app"
  local out="$root/artifacts/b076/measure-$1.json"
  rm -f "$out"
  open -n -W "$app" --args -OPENLOGOS_WINDOW "$1x$2" -OPENLOGOS_SHELL_MEASURE "$out"
  echo "$out"
}

case "${1:-shots}" in
  shots)
    shot mac-translation-well
    shot mac-original-word-study -OPENLOGOS_RUN "column.focus:2,tool.word_study" -OPENLOGOS_WORD "2:ἠγάπησεν"
    shot mac-notes-editing -OPENLOGOS_RUN "column.focus:3" -OPENLOGOS_NOTE_DEMO 1
    shot mac-selection-group -OPENLOGOS_SELECT "1:loved the world"
    shot mac-fallback-pen-notes -OPENLOGOS_RUN "tool.pen,view.notes_dock" -OPENLOGOS_FOCUS dock
    shot mac-original-selection-dark -OPENLOGOS_THEME dark -OPENLOGOS_RUN "column.focus:2" -OPENLOGOS_SELECT "2:τὸν κόσμον"
    ;;
  measure)
    measure 1440 900
    measure 1024 768
    ;;
  *) echo "usage: $0 [shots|measure]" >&2; exit 2 ;;
esac
