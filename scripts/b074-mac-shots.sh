#!/usr/bin/env bash
# B074 evidence for the Mac user store (docs/evidence/B074-mac.md), reproducibly: the pictures
# and the speed runs. Every run uses a store directory under artifacts/b074/ (never the real
# ~/Library/Application Support/OpenLogos store).
#
#   scripts/b074-mac-shots.sh              the pictures into docs/evidence/B074/mac-store-*.png
#   scripts/b074-mac-shots.sh measure      the acceptance store built through the API, 20 timed
#                                          restores, the store rows, and the specs/22 §7 run
#
# As scripts/b078-mac-shots.sh: the app draws its own window into the PNG (-OPENLOGOS_SNAPSHOT)
# and quits; the store's launch arguments are listed in Sources/Shell/StoreLaunch.swift. Build
# first with `just test-reader mac` (Debug, artifacts/b005); `measure` uses the Release build in
# artifacts/b074/release when there is one (OPENLOGOS_B074_APP overrides).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
out_dir="$root/docs/evidence/B074"
work="$root/artifacts/b074"
mkdir -p "$out_dir" "$work"
debug_app="$root/artifacts/b005/DerivedData/Build/Products/Debug/OpenLogos.app"

run() { # app args…
  local app="$1"; shift
  pkill -f "$app/Contents/MacOS/OpenLogos" 2>/dev/null || true
  sleep 0.3
  open -n -W "$app" --args -OPENLOGOS_WINDOW 1440x900 "$@"
}

shot() { # name store args…
  local name="$1" store="$2"; shift 2
  local out="$out_dir/$name.png"
  rm -f "$out"
  run "$debug_app" -OPENLOGOS_STORE "$store" -OPENLOGOS_SNAPSHOT "$out" "$@"
  printf '%s  %s KB\n' "$out" "$(( $(stat -f %z "$out") / 1024 ))"
}

fresh() { rm -rf "$work/$1"; mkdir -p "$work/$1"; echo "$work/$1"; }

case "${1:-shots}" in
  shots)
    # The owner's scenario, then a relaunch over the same store.
    store="$(fresh demo)"
    shot mac-store-before-quit "$store" -OPENLOGOS_STORE_DEMO 1 -OPENLOGOS_SNAPSHOT_DELAY 1
    shot mac-store-after-relaunch "$store" -OPENLOGOS_SNAPSHOT_DELAY 1.5
    # Cross-workspace rows: the Romans workspace and its tabs from the Study workspace.
    shot mac-store-palette-workspaces "$store" -OPENLOGOS_PALETTE "Rom" -OPENLOGOS_SNAPSHOT_DELAY 0.8
    # A tab dragged onto column 2's strip (the slot shown), then dropped: Undo names it.
    store="$(fresh drag)"
    shot mac-store-drag-slot "$store" -OPENLOGOS_DRAG "t2>2:0" -OPENLOGOS_DRAG_HOLD 1
    shot mac-store-drag-undo "$store" -OPENLOGOS_DRAG "t2>2:0" -OPENLOGOS_PALETTE "undo"
    # A damaged file: the notice, a backup beside it, the seed session.
    store="$(fresh damaged)"
    printf '{"format":"openlogos-user-store","schema_version":"1.0","session":{' > "$store/user-store.json"
    shot mac-store-damaged-notice "$store" -OPENLOGOS_SNAPSHOT_DELAY 0.8
    ls -1 "$store"
    ;;
  measure)
    app="${OPENLOGOS_B074_APP:-$work/release/Build/Products/Release/OpenLogos.app}"
    [[ -d "$app" ]] || app="$debug_app"
    echo "app: $app"
    store="$(fresh acceptance)"
    run "$app" -OPENLOGOS_STORE "$store" -OPENLOGOS_BUILD_STORE 1
    cp "$store/user-store.json" "$work/acceptance-store.json"
    printf 'acceptance store: %s bytes\n' "$(stat -f %z "$store/user-store.json")"
    rm -f "$work/restore.jsonl"
    for _ in $(seq 1 20); do
      run "$app" -OPENLOGOS_STORE "$store" -OPENLOGOS_RESTORE_MEASURE "$work/restore.jsonl"
    done
    python3 - "$work/restore.jsonl" <<'EOF'
import json, sys
rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
def pct(values, p):
    v = sorted(values); return v[min(len(v) - 1, round((len(v) - 1) * p))]
for key in ("read_ms", "import_ms", "highlights_ms", "ink_ms", "relayout_ms", "relayouts", "laid_out_ms", "first_frame_ms"):
    values = [r[key] for r in rows]
    print(f"{key}: n={len(values)} p50={pct(values, .5):.2f} p95={pct(values, .95):.2f} max={max(values):.2f}")
EOF
    # The same launches with no file: what a launch costs without a restore.
    empty="$(fresh empty)"
    rm -f "$work/restore-empty.jsonl"
    for _ in $(seq 1 20); do
      run "$app" -OPENLOGOS_STORE "$empty" -OPENLOGOS_RESTORE_MEASURE "$work/restore-empty.jsonl"
      rm -f "$empty"/user-store.json
    done
    python3 - "$work/restore-empty.jsonl" <<'EOF'
import json, sys
rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
def pct(values, p):
    v = sorted(values); return v[min(len(v) - 1, round((len(v) - 1) * p))]
for key in ("highlights_ms", "ink_ms", "relayout_ms", "relayouts", "laid_out_ms", "first_frame_ms"):
    values = [r[key] for r in rows]
    print(f"no file, {key}: n={len(values)} p50={pct(values, .5):.2f} p95={pct(values, .95):.2f} max={max(values):.2f}")
EOF
    cp "$work/acceptance-store.json" "$store/user-store.json"
    rm -f "$work/store-measure.json"
    run "$app" -OPENLOGOS_STORE "$store" -OPENLOGOS_STORE_MEASURE "$work/store-measure.json"
    echo "$work/store-measure.json"
    rm -f "$work/measure-1440.json"
    run "$app" -OPENLOGOS_SHELL_MEASURE "$work/measure-1440.json"
    echo "$work/measure-1440.json"
    ;;
  *) echo "usage: $0 [shots|measure]" >&2; exit 2 ;;
esac
