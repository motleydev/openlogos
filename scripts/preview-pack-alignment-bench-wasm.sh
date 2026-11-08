#!/usr/bin/env bash
# B067: alignment token lookup and lemma rendering latency in a dedicated worker in headless
# Chrome, over built layers. Serves the morphology and alignment outputs (their layers/ and
# the bench-samples.json that `just preview-pack-alignment-bench` writes) on 127.0.0.1 with
# CORS and builds the worker test with the URL; the test (crates/ffi/tests/worker.rs) prints
# one `B067 wasm:` line. Run inside `nix develop .#web`.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
morphology="${OPENLOGOS_MORPHOLOGY:-$HOME/.cache/openlogos/morphology}"
alignments="${1:-${OPENLOGOS_ALIGNMENTS:-$HOME/.cache/openlogos/alignments}}"
port="${OPENLOGOS_BENCH_PORT:-8793}"

for tool in python3 curl; do
  command -v "$tool" >/dev/null 2>&1 || { echo "$tool is not on PATH." >&2; exit 1; }
done
if [ ! -f "$alignments/bench-samples.json" ]; then
  echo "no bench samples at $alignments: run 'just preview-pack-alignment' and 'just preview-pack-alignment-bench' first." >&2
  exit 1
fi
if [ ! -d "$morphology/layers" ]; then
  echo "no morphology layers at $morphology: run 'just preview-pack-morphology' first." >&2
  exit 1
fi

served="$(mktemp -d)"
ln -s "$morphology" "$served/morphology"
ln -s "$alignments" "$served/alignments"
cd "$served"
python3 -I -c '
import http.server, sys
class Handler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        super().end_headers()
    def log_message(self, *args):
        pass
http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
' "$port" &
server=$!
trap 'kill "$server" 2>/dev/null || true; rm -rf "$served"' EXIT
for _ in 1 2 3 4 5 6 7 8 9 10; do
  curl -fsI "http://127.0.0.1:$port/alignments/bench-samples.json" >/dev/null 2>&1 && break
  sleep 0.5
done
cd "$root"
# As in preview-pack-bench-wasm.sh: fat LTO does not link the test harness against
# sqlite-wasm-rs's C library.
CARGO_PROFILE_RELEASE_LTO=off \
  OPENLOGOS_ALIGNMENT_BENCH_URL="http://127.0.0.1:$port" \
  scripts/wasm-test.sh --release --features retrieval -- --nocapture
