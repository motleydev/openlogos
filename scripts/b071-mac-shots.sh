#!/usr/bin/env bash
# B071 evidence for the Mac app over the real editions (docs/evidence/B071-mac.md), reproducibly.
# Needs a built preview pack (`just preview-pack`, docs/preview-pack.md); says SKIPPED without one.
#
#   scripts/b071-mac-shots.sh              the pictures into docs/evidence/B071/mac-*.png
#   scripts/b071-mac-shots.sh measure      Release app: the keystroke, chapter and workspace rows
#                                          (-OPENLOGOS_PACK_MEASURE), the specs/22 §7 run and 20 cold
#                                          launches, JSON under artifacts/b071/
#
# The app draws its own window into the PNG (-OPENLOGOS_SNAPSHOT) and quits. Build first with
# `just test-reader mac` (Debug, artifacts/b005) and, for `measure`, a Release build into
# artifacts/b071/release. Every run names its store; the real Application Support store is
# never touched.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
pack="${OPENLOGOS_PREVIEW_PACK:-$HOME/.cache/openlogos/preview-pack}"
if [[ ! -f "$pack/packs/bsb/manifest.json" ]]; then
  echo "SKIPPED: no built preview pack at $pack (just preview-pack)"
  exit 2
fi
out_dir="$root/docs/evidence/B071"
work="$root/artifacts/b071"
mkdir -p "$out_dir" "$work"
debug_app="$root/artifacts/b005/DerivedData/Build/Products/Debug/OpenLogos.app"

run() { # app args…
  local app="$1"; shift
  pkill -f "$app/Contents/MacOS/OpenLogos" 2>/dev/null || true
  sleep 0.3
  open -n -W "$app" --args -OPENLOGOS_WINDOW 1440x900 -OPENLOGOS_PREVIEW_PACK "$pack" "$@"
}

shot() { # name args…
  local name="$1"; shift
  local path="$out_dir/$name.png"
  rm -f "$path"
  run "$debug_app" -OPENLOGOS_SNAPSHOT "$path" -OPENLOGOS_SNAPSHOT_DELAY 1 "$@"
  printf '%s  %s KB\n' "$path" "$(( $(/usr/bin/stat -f %z "$path") / 1024 ))"
}

fresh() { rm -rf "${work:?}/${1:?}"; mkdir -p "$work/$1"; echo "$work/$1"; }

case "${1:-shots}" in
  shots)
    # The three editions: the seed session (BSB and SBLGNT), the WLC right to left, its order.
    shot mac-real-john-3
    shot mac-real-wlc-rtl -OPENLOGOS_RUN "column.focus:1,tab.next" -OPENLOGOS_TYPE "Gen 1:1"
    shot mac-real-wlc-mal-psalms -OPENLOGOS_RUN "column.focus:1,tab.next" -OPENLOGOS_TYPE "Mal 3:22"
    # Search: words in each language, on the real packs.
    shot mac-search-english -OPENLOGOS_PALETTE "#loved the world"
    shot mac-search-greek -OPENLOGOS_PALETTE "#αγαπη"
    shot mac-search-hebrew -OPENLOGOS_PALETTE "#בראשית"
    # Followers: an absent verse, and a verse past the chapter's last.
    shot mac-follow-absent -OPENLOGOS_TYPE "Mk 9:44"
    shot mac-follow-past-end -OPENLOGOS_TYPE "Rom 16:25"
    # The licence's words.
    shot mac-about-sblgnt -OPENLOGOS_ABOUT 2
    shot mac-about-wlc -OPENLOGOS_RUN "column.focus:1,tab.next" -OPENLOGOS_ABOUT 1
    # Texts the owner has not ruled on, as the packs hold them.
    shot mac-open-psalm-titles -OPENLOGOS_TYPE "Ps 3"
    shot mac-open-psalm-poetry -OPENLOGOS_TYPE "Ps 11"
    # No pack: the placeholder, which says so.
    rm -f "$out_dir/mac-no-pack.png"
    pkill -f "$debug_app/Contents/MacOS/OpenLogos" 2>/dev/null || true
    sleep 0.3
    open -n -W "$debug_app" --args -OPENLOGOS_WINDOW 1440x900 -OPENLOGOS_PREVIEW_PACK none \
      -OPENLOGOS_SNAPSHOT "$out_dir/mac-no-pack.png" -OPENLOGOS_SNAPSHOT_DELAY 1
    printf '%s  %s KB\n' "$out_dir/mac-no-pack.png" "$(( $(/usr/bin/stat -f %z "$out_dir/mac-no-pack.png") / 1024 ))"
    # A session saved on the placeholder library, restored over the pack.
    store="$(fresh placeholder-session)"
    open -n -W "$debug_app" --args -OPENLOGOS_WINDOW 1440x900 -OPENLOGOS_PREVIEW_PACK none -OPENLOGOS_STORE "$store" \
      -OPENLOGOS_STORE_DEMO 1 -OPENLOGOS_SNAPSHOT "$out_dir/mac-store-on-placeholder.png" -OPENLOGOS_SNAPSHOT_DELAY 1
    shot mac-store-restored-on-pack -OPENLOGOS_STORE "$store" -OPENLOGOS_SNAPSHOT_DELAY 1.5
    du -sh "$out_dir"
    ls "$out_dir" | wc -l
    ;;
  measure)
    app="${OPENLOGOS_B071_APP:-$work/release/Build/Products/Release/OpenLogos.app}"
    [[ -d "$app" ]] || { echo "no Release build at $app"; exit 1; }
    echo "app: $app"
    /usr/bin/python3 - "$work/queries.txt" <<'PY'
import json, sys
d = json.load(open("crates/preview-pack/bench-queries.json"))
open(sys.argv[1], "w").write("\n".join(d["en"] + d["grc"] + d["hbo"]) + "\n")
PY
    run "$app" -OPENLOGOS_PACK_MEASURE "$work/pack.json" -OPENLOGOS_PACK_QUERIES "$work/queries.txt"
    run "$app" -OPENLOGOS_SHELL_MEASURE "$work/shell.json"
    # The acceptance store (B074), built on the placeholder library, restored over the pack.
    store="$(fresh acceptance)"
    open -n -W "$app" --args -OPENLOGOS_PREVIEW_PACK none -OPENLOGOS_STORE "$store" -OPENLOGOS_BUILD_STORE 1
    rm -f "$work/launch.jsonl"
    for _ in $(seq 1 20); do
      before="$(/usr/bin/python3 -c 'import time; print(time.time())')"
      run "$app" -OPENLOGOS_STORE "$store" -OPENLOGOS_RESTORE_MEASURE "$work/launch.jsonl"
      /usr/bin/python3 - "$work/launch.jsonl" "$before" <<'PY'
import json, sys
path, before = sys.argv[1], float(sys.argv[2])
lines = open(path).read().splitlines()
row = json.loads(lines[-1]); row["open_to_frame_ms"] = (row["frame_epoch"] - before) * 1000
lines[-1] = json.dumps(row, sort_keys=True)
open(path, "w").write("\n".join(lines) + "\n")
PY
    done
    /usr/bin/python3 - "$work" <<'PY'
import json, sys
work = sys.argv[1]
rows = [json.loads(l) for l in open(f"{work}/launch.jsonl")]
def pct(v, p):
    v = sorted(v); return v[min(len(v) - 1, round((len(v) - 1) * p))]
for key in ["pack_open_ms", "laid_out_ms", "first_frame_ms", "process_to_frame_ms", "open_to_frame_ms"]:
    v = [r[key] for r in rows]
    print(f"{key:24} p50 {pct(v,.5):8.1f}  p95 {pct(v,.95):8.1f}  max {max(v):8.1f}  n {len(v)}")
for name in ["pack", "shell"]:
    print("---", name)
    for s in json.load(open(f"{work}/{name}.json"))["stats"]:
        over = " OVER" if s["p95_ms"] > s["budget_ms"] else ""
        print(f'{s["p50_ms"]:8.2f} {s["p95_ms"]:8.2f} {s["max_ms"]:8.2f} n={s["samples"]:<5} budget {s["budget_ms"]:<5}{over}  {s["interaction"]}')
PY
    ;;
  *)
    echo "usage: $0 [shots|measure]" >&2
    exit 2
    ;;
esac
