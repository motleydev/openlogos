#!/usr/bin/env bash
# B072 evidence pictures of the Mac workspace shell (docs/evidence/B072.md), reproducibly.
#
#   scripts/b072-mac-shots.sh              every picture into artifacts/b072/shots/mac-*.png
#   scripts/b072-mac-shots.sh measure      the specs/22 §7 run (Release build) into
#                                          artifacts/b072/measure-<width>.json
#
# The app draws its own window into the PNG (-OPENLOGOS_SNAPSHOT, NSView.cacheDisplay) and
# quits, so no screen-recording permission and no other window is involved. Build first with
# `just preview-mac` (Debug, artifacts/mac) and, for `measure`, the Release build below.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
out_dir="$root/artifacts/b072/shots"
mkdir -p "$out_dir"

shot() { # name args…
  local name="$1"; shift
  local app="$root/artifacts/mac/DerivedData/Build/Products/Debug/OpenLogos.app"
  local out="$out_dir/$name.png"
  rm -f "$out"
  pkill -f "$app/Contents/MacOS/OpenLogos" 2>/dev/null || true
  sleep 0.5
  open -n -W "$app" --args -OPENLOGOS_SNAPSHOT "$out" -OPENLOGOS_SNAPSHOT_DELAY 1.5 "$@"
  printf '%s  %s KB\n' "$out" "$(( $(stat -f %z "$out") / 1024 ))"
}

measure() { # width height
  local app="$root/artifacts/mac-release/DerivedData/Build/Products/Release/OpenLogos.app"
  local out="$root/artifacts/b072/measure-$1.json"
  rm -f "$out"
  open -n -W "$app" --args -OPENLOGOS_WINDOW "$1x$2" -OPENLOGOS_SHELL_MEASURE "$out"
  echo "$out"
}

case "${1:-shots}" in
  shots)
    w=(-OPENLOGOS_WINDOW 1440x900)
    shot mac-light-1440 "${w[@]}"
    shot mac-sepia-1440 "${w[@]}" -OPENLOGOS_THEME sepia
    shot mac-dark-1440 "${w[@]}" -OPENLOGOS_THEME dark
    shot mac-light-1024 -OPENLOGOS_WINDOW 1024x768
    shot mac-dark-1024 -OPENLOGOS_WINDOW 1024x768 -OPENLOGOS_THEME dark
    shot mac-palette-passage "${w[@]}" -OPENLOGOS_PALETTE "Jn 3:16 @2"
    shot mac-palette-commands "${w[@]}" -OPENLOGOS_PALETTE ">tab"
    shot mac-palette-commands-expanded "${w[@]}" -OPENLOGOS_PALETTE ">tab" -OPENLOGOS_PALETTE_EXPAND commands
    shot mac-palette-tabs "${w[@]}" -OPENLOGOS_THEME sepia -OPENLOGOS_PALETTE "@jn"
    shot mac-palette-text "${w[@]}" -OPENLOGOS_THEME dark -OPENLOGOS_PALETTE "#loved"
    shot mac-palette-all "${w[@]}" -OPENLOGOS_PALETTE "gen"
    shot mac-tools-demo "${w[@]}" -OPENLOGOS_SHELL_DEMO 1 -OPENLOGOS_TOOL highlight
    shot mac-tools-pen-dark "${w[@]}" -OPENLOGOS_THEME dark -OPENLOGOS_SHELL_DEMO 1 -OPENLOGOS_TOOL pen
    shot mac-tools-collapsed-1024 -OPENLOGOS_WINDOW 1024x768 -OPENLOGOS_RUN view.tool_palette
    shot mac-workspace-hebrew "${w[@]}" -OPENLOGOS_THEME sepia -OPENLOGOS_RUN workspace.next
    shot mac-placeholder-and-size "${w[@]}" -OPENLOGOS_RUN "column.focus:1,nav.navigate:Rom.8.28,view.text_size_up,view.text_size_up"
    shot mac-notes-dock "${w[@]}" -OPENLOGOS_RUN "view.notes_dock,column.new"
    ;;
  measure)
    measure 1440 900
    measure 1024 768
    ;;
  *) echo "usage: $0 [shots|measure]" >&2; exit 2 ;;
esac
