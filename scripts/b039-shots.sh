#!/usr/bin/env bash
# B039 evidence pictures of the ink margin (docs/evidence/B039.md), after scripts/b039-shots.sh, reproducibly:
#
#   scripts/b039-shots.sh ipad <name> <width> [measure 1|-] [extra launch args…]
#       launch the installed app on the iPad simulator scripts/sim.sh booted, in the "ink"
#       scenario at a simulated width, optionally running the in-process latency measurement (1), screenshot, crop to
#       the viewport (after scripts/b004-shots.sh) and keep it under 400 KB
#   scripts/b039-shots.sh mac <name> <width> [measure 1|-] [extra launch args…]
#       the same with the macOS build (artifacts/b039/DerivedData), captured by window id with
#       `screencapture -l` so other windows and dialogs never show (no accessibility permission
#       is needed; a small Swift helper lists the window)
#   scripts/b039-shots.sh ipad-log | mac-log
#       print Documents/b039-ink.jsonl and the PencilKit export from the app container (or artifacts/b039/logs on the Mac)
#
# Pictures go to artifacts/b039/shots/<name>.png; the few that ship are copied into
# docs/evidence/B039/ by hand. Only the iPad scripts/sim.sh recorded is used.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
bundle_id="org.openlogos.app"
out_dir="$root/artifacts/b039/shots"
mkdir -p "$out_dir"
screen_w=834 screen_h=1210 scale=2 safe_top=32 # iPad Pro 11-inch (M5) portrait, points; the frame's top edge measured at 32 pt

height_for() { case "$1" in 1440) echo 900 ;; 1024) echo 768 ;; 768) echo 1024 ;; 390) echo 844 ;; *) echo 900 ;; esac; }

shrink() { # file: under 400 KB by downscaling the long edge
  local out="$1" long=1600
  while (( $(stat -f %z "$out") > 400000 )) && (( long > 600 )); do
    sips -Z "$long" "$out" --out "$out" >/dev/null
    long=$((long - 200))
  done
  printf '%s  %s KB\n' "$out" "$(( $(stat -f %z "$out") / 1024 ))"
}

ipad() { # name width case extra…
  local name="$1" width="$2" case_="${3:--}"; shift 3 || shift $#
  local device; device="$(cat "$root/artifacts/sim/device")"
  local height; height="$(height_for "$width")"
  local args=(-OPENLOGOS_VIEW ink -OPENLOGOS_WIDTH "$width" -OPENLOGOS_HEIGHT "$height" -OPENLOGOS_LAYOUT A -OPENLOGOS_DEBUGBAR 0 -OPENLOGOS_LOG 1)
  [[ "$case_" != - ]] && args+=(-OPENLOGOS_INK_MEASURE "$case_")
  args+=("$@")
  xcrun simctl terminate "$device" "$bundle_id" >/dev/null 2>&1 || true
  xcrun simctl launch "$device" "$bundle_id" "${args[@]}" >/dev/null
  sleep "${B039_SETTLE:-4}"
  local raw="$out_dir/$name.raw.png" out="$out_dir/$name.png"
  xcrun simctl io "$device" screenshot "$raw" >/dev/null 2>&1
  local avail_h=$((screen_h - safe_top)) draw_w draw_h rotated=0 factor=1
  if (( width <= screen_w && height <= avail_h )); then
    draw_w=$width; draw_h=$height
  elif (( height <= screen_w && width <= avail_h )); then
    rotated=1; draw_w=$height; draw_h=$width
  else
    factor="$(python3 -I -c "print(min($screen_w/$width, $avail_h/$height))")"
    draw_w="$(python3 -I -c "print(int($width*$factor))")"; draw_h="$(python3 -I -c "print(int($height*$factor))")"
  fi
  # Inside the 1-point frame border the app draws around a simulated viewport.
  local x=$(( (screen_w - draw_w) * scale / 2 + scale )) y=$(( (safe_top + 1) * scale ))
  sips -c $(( (draw_h - 2) * scale )) $(( (draw_w - 2) * scale )) --cropOffset "$y" "$x" "$raw" --out "$out" >/dev/null
  if (( rotated )); then sips -r 90 "$out" --out "$out" >/dev/null; fi
  rm -f "$raw"
  shrink "$out"
}

window_id() { # the CGWindowID of the frontmost OpenLogos window, via a tiny compiled helper
  local helper="$root/artifacts/b039/windowid"
  if [[ ! -x "$helper" ]]; then
    cat >"$helper.swift" <<'SWIFT'
import CoreGraphics
import Foundation
let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let list = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else { exit(1) }
for window in list where (window[kCGWindowOwnerName as String] as? String) == "OpenLogos" && (window[kCGWindowLayer as String] as? Int) == 0 {
    if let id = window[kCGWindowNumber as String] as? Int { print(id); exit(0) }
}
exit(1)
SWIFT
    xcrun swiftc -O -o "$helper" "$helper.swift" 2>/dev/null
  fi
  "$helper"
}

mac() { # name width case extra…
  local name="$1" width="$2" case_="${3:--}"; shift 3 || shift $#
  local height; height="$(height_for "$width")"
  local app="$root/artifacts/b039/DerivedData/Build/Products/Debug/OpenLogos.app"
  local args=(-OPENLOGOS_VIEW ink -OPENLOGOS_WIDTH "$width" -OPENLOGOS_HEIGHT "$height" -OPENLOGOS_LAYOUT A -OPENLOGOS_DEBUGBAR 0 -OPENLOGOS_LOG 1 -OPENLOGOS_LOG_DIR "$root/artifacts/b039/logs")
  [[ "$case_" != - ]] && args+=(-OPENLOGOS_INK_MEASURE "$case_")
  args+=("$@")
  pkill -f "$app/Contents/MacOS/OpenLogos" 2>/dev/null || true
  sleep 1
  open -n "$app" --args "${args[@]}"
  sleep "${B039_SETTLE:-5}"
  local id; id="$(window_id)" || { echo "no OpenLogos window on screen" >&2; exit 1; }
  local out="$out_dir/$name.png"
  screencapture -x -o -l "$id" "$out"
  shrink "$out"
}

logs() { # ipad|mac
  if [[ "$1" == ipad ]]; then
    local device; device="$(cat "$root/artifacts/sim/device")"
    local container; container="$(xcrun simctl get_app_container "$device" "$bundle_id" data)"
    for f in b039-ink.jsonl b039-pencilkit-notes.txt b039-pencilkit-export.json; do [[ -f "$container/Documents/$f" ]] && { echo "## $f"; cat "$container/Documents/$f"; }; done
  else
    for f in b039-ink.jsonl b039-pencilkit-notes.txt b039-pencilkit-export.json; do [[ -f "$root/artifacts/b039/logs/$f" ]] && { echo "## $f"; cat "$root/artifacts/b039/logs/$f"; }; done
  fi
}

case "${1:-}" in
  ipad) shift; ipad "$@" ;;
  mac) shift; mac "$@" ;;
  ipad-log) logs ipad ;;
  mac-log) logs mac ;;
  *) echo "usage: $0 {ipad|mac} <name> <width> [measure 1|-] [launch args…] | {ipad-log|mac-log}" >&2; exit 1 ;;
esac
