#!/usr/bin/env bash
# B077 evidence pictures of the context menu in the Mac shell (docs/evidence/B077-mac.md),
# reproducibly, and its speed run.
#
#   scripts/b077-mac-shots.sh              every picture into docs/evidence/B077/mac-menu-*.png
#   scripts/b077-mac-shots.sh measure      the specs/22 §7 run (Release build) into
#                                          artifacts/b077/measure-<width>.json
#
# As scripts/b076-mac-shots.sh: the app draws its own window into the PNG (-OPENLOGOS_SNAPSHOT)
# and quits. An open menu is its own window; the app draws it over its window where it is on
# screen (-OPENLOGOS_MENU opens it, -OPENLOGOS_MENU_SUBMENU walks to a submenu with ↓ and →).
# Build first with `just preview-mac` (Debug, artifacts/mac) and, for `measure`, the Release
# build (artifacts/mac-release, ARCHS=arm64).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
out_dir="$root/docs/evidence/B077"
mkdir -p "$out_dir" "$root/artifacts/b077"

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
  local out="$root/artifacts/b077/measure-$1.json"
  rm -f "$out"
  open -n -W "$app" --args -OPENLOGOS_WINDOW "$1x$2" -OPENLOGOS_SHELL_MEASURE "$out"
  echo "$out"
}

case "${1:-shots}" in
  shots)
    shot mac-menu-word-greek -OPENLOGOS_MENU "word:2:ἠγάπησεν"
    shot mac-menu-word-bsb-highlight -OPENLOGOS_MENU "word:1:loved" -OPENLOGOS_MENU_SUBMENU Highlight
    shot mac-menu-selection -OPENLOGOS_MENU "selection:1:loved the world"
    shot mac-menu-tab-move -OPENLOGOS_MENU "tab:1:1" -OPENLOGOS_MENU_SUBMENU "Move to"
    shot mac-menu-note-link -OPENLOGOS_NOTE_DEMO 1 -OPENLOGOS_MENU "note_link:t4"
    shot mac-menu-find-in-chapter -OPENLOGOS_MENU "word:1:world" -OPENLOGOS_MENU_CHOOSE "Find in this chapter"
    shot mac-menu-search-scope -OPENLOGOS_MENU "word:2:ἠγάπησεν" -OPENLOGOS_MENU_CHOOSE "Search “ἠγάπησεν” in this edition"
    shot mac-menu-dark -OPENLOGOS_THEME dark -OPENLOGOS_MENU "word:2:κόσμον" -OPENLOGOS_MENU_SUBMENU "Copy"
    ;;
  measure)
    measure 1440 900
    measure 1024 768
    ;;
  *) echo "usage: $0 [shots|measure]" >&2; exit 2 ;;
esac
