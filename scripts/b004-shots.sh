#!/usr/bin/env bash
# B004 evidence screenshots: launch the installed prototype on the booted iPad simulator with
# the launch arguments that select a view, layout, state and simulated viewport, take a
# screenshot, crop it to the simulated viewport, and keep it under 400 KB.
#
#   scripts/b004-shots.sh all                      # the whole matrix into docs/evidence/B004/
#   scripts/b004-shots.sh one read 1024 A normal   # one picture: view width layout state [typesize]
#   scripts/b004-shots.sh metrics                  # pull Documents/b004-metrics.jsonl out of the app container
#
# Requires `just sim` to have built, installed and booted first (scripts/sim.sh). Only the device
# scripts/sim.sh recorded in artifacts/sim/device is used; no other simulator is touched.
# The screenshot geometry assumes the iPad Pro 11-inch (M5) in portrait (834 × 1210 points at 2×);
# a viewport that fits only rotated is drawn rotated by the app and rotated back here.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
bundle_id="org.openlogos.app"
out_dir="$root/docs/evidence/B004"
raw_dir="$root/artifacts/sim/b004-raw"
device="$(cat "$root/artifacts/sim/device")"
screen_w=834 screen_h=1210 scale=2 safe_top=24 # points; the frame sits under the status bar
large_type="UICTContentSizeCategoryAccessibilityL"

mkdir -p "$out_dir" "$raw_dir"

launch() { # view width layout state typesize(default|large) metrics(0|1) focus(0|1)
  local view="$1" width="$2" layout="$3" state="$4" type="${5:-default}" metrics="${6:-0}" focus="${7:-0}"
  local args=(-OPENLOGOS_VIEW "$view" -OPENLOGOS_WIDTH "$width" -OPENLOGOS_LAYOUT "$layout" -OPENLOGOS_STATE "$state" -OPENLOGOS_DEBUGBAR 0)
  [[ "$metrics" == 1 ]] && args+=(-OPENLOGOS_METRICS 1)
  [[ "$focus" == 1 ]] && args+=(-OPENLOGOS_FOCUS 1)
  [[ "$type" == large ]] && args+=(-UIPreferredContentSizeCategoryName "$large_type")
  xcrun simctl terminate "$device" "$bundle_id" >/dev/null 2>&1 || true
  xcrun simctl launch "$device" "$bundle_id" "${args[@]}" >/dev/null
  sleep "${B004_SETTLE:-3}"
}

height_for() { case "$1" in 1440) echo 900 ;; 1024) echo 768 ;; 768) echo 1024 ;; 390) echo 844 ;; *) echo 900 ;; esac; }

shoot() { # name view width layout state typesize metrics focus
  local name="$1"; shift
  local view="$1" width="$2" layout="$3" state="$4" type="${5:-default}" metrics="${6:-0}" focus="${7:-0}"
  local height; height="$(height_for "$width")"
  launch "$view" "$width" "$layout" "$state" "$type" "$metrics" "$focus"
  local raw="$raw_dir/$name.png" out="$out_dir/$name.png"
  xcrun simctl io "$device" screenshot "$raw" >/dev/null 2>&1
  # Where the app drew the viewport: centred horizontally at the top of the safe area; rotated
  # by -90° when it only fits that way; scaled down when it fits neither way.
  local avail_h=$((screen_h - safe_top)) draw_w draw_h rotated=0 factor=1
  if (( width <= screen_w && height <= avail_h )); then
    draw_w=$width; draw_h=$height
  elif (( height <= screen_w && width <= avail_h )); then
    rotated=1; draw_w=$height; draw_h=$width
  else
    factor="$(python3 -I -c "print(min($screen_w/$width, $avail_h/$height))")"
    draw_w="$(python3 -I -c "print(int($width*$factor))")"; draw_h="$(python3 -I -c "print(int($height*$factor))")"
  fi
  local x=$(( (screen_w - draw_w) * scale / 2 )) y=$(( safe_top * scale ))
  sips -c $((draw_h * scale)) $((draw_w * scale)) --cropOffset "$y" "$x" "$raw" --out "$out" >/dev/null
  if (( rotated )); then sips -r 90 "$out" --out "$out" >/dev/null; fi
  # Under 400 KB: downscale the long edge until it is.
  local long=1600
  while (( $(stat -f %z "$out") > 400000 )) && (( long > 600 )); do
    sips -Z "$long" "$out" --out "$out" >/dev/null
    long=$((long - 200))
  done
  printf '%s  %s KB%s%s\n' "$name" "$(( $(stat -f %z "$out") / 1024 ))" "$([[ $rotated == 1 ]] && echo '  (rotated back)')" "$([[ $factor != 1 ]] && echo "  (drawn at scale $factor)")"
}

matrix() {
  # name  view  width  layout  state  type  metrics  focus
  for layout in A B; do
    shoot "read-1024-$layout-default"        read       1024 "$layout" normal
    shoot "read-390-$layout-default"         read       390  "$layout" normal
    shoot "three-1024-$layout-default"       three      1024 "$layout" normal
    shoot "three-390-$layout-default"        three      390  "$layout" normal
    shoot "collection-1024-$layout-default"  collection 1024 "$layout" normal
    shoot "collection-390-$layout-default"   collection 390  "$layout" normal
    shoot "selection-1024-$layout-default"   selection  1024 "$layout" normal
    shoot "selection-390-$layout-default"    selection  390  "$layout" normal
    shoot "search-1024-$layout-default"      search     1024 "$layout" normal
    shoot "search-390-$layout-default"       search     390  "$layout" normal
    shoot "margin-1024-$layout-default"      margin     1024 "$layout" normal
    shoot "margin-390-$layout-default"       margin     390  "$layout" normal
    shoot "handoff-1024-$layout-default"     handoff    1024 "$layout" normal
    shoot "handoff-390-$layout-default"      handoff    390  "$layout" normal
    # measurement set: three texts, drawers closed, with the metrics overlay, at the four widths
    for width in 1440 1024 768 390; do
      shoot "three-$width-$layout-metrics"   three "$width" "$layout" normal default 1
    done
  done
  # large type (Dynamic Type accessibility L), layout A
  shoot "read-1024-A-largetype"        read       1024 A normal large
  shoot "read-390-A-largetype"         read       390  A normal large
  shoot "three-1024-A-largetype"       three      1024 A normal large
  shoot "collection-1024-A-largetype"  collection 1024 A normal large
  shoot "selection-1024-A-largetype"   selection  1024 A normal large
  shoot "search-1024-A-largetype"      search     1024 A normal large
  shoot "margin-1024-A-largetype"      margin     1024 A normal large
  shoot "handoff-1024-A-largetype"     handoff    1024 A normal large
  shoot "three-390-B-largetype"        three      390  B normal large
  # missing-data and other required states
  shoot "read-1024-A-loading"          read       1024 A loading
  shoot "read-1024-A-readonly"         read       1024 A readOnly
  shoot "three-1024-A-unavailable"     three      1024 A unavailable
  shoot "three-390-B-unavailable"      three      390  B unavailable
  shoot "collection-1024-A-empty"      collection 1024 A empty
  shoot "selection-1024-A-unavailable" selection  1024 A unavailable
  shoot "selection-1024-A-offline"     selection  1024 A offline
  shoot "search-1024-A-invalid"        search     1024 A invalidQuery
  shoot "search-390-A-empty"           search     390  A empty
  shoot "margin-1024-A-readonly"       margin     1024 A readOnly
  shoot "handoff-1024-A-unavailable"   handoff    1024 A unavailable
  shoot "handoff-1024-A-conflict"      handoff    1024 A conflict
  shoot "read-1024-A-focus"            read       1024 A normal default 0 1
}

case "${1:-all}" in
  all) matrix ;;
  one) shift; shoot "$1-$2-$3-$4${5:+-$5}" "$1" "$2" "$3" "$4" "${5:-default}" "${6:-0}" "${7:-0}" ;;
  metrics)
    container="$(xcrun simctl get_app_container "$device" "$bundle_id" data)"
    cat "$container/Documents/b004-metrics.jsonl" ;;
  *) echo "usage: $0 {all|one view width layout state [default|large] [metrics] [focus]|metrics}" >&2; exit 1 ;;
esac
