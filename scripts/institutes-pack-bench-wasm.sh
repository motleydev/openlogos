#!/usr/bin/env bash
# B016: the Institutes work pack in a dedicated worker in headless Chrome. Serves the works
# directory on 127.0.0.1 with CORS and builds the worker test with the URL; the test
# (crates/ffi/tests/worker.rs) opens every chapter, a section and the reverse index, replays the
# bench queries through paletteQuery and prints one `B016 wasm:` line. Run inside
# `nix develop .#web`.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
works="${1:-${OPENLOGOS_WORKS:-$HOME/.cache/openlogos/works}}"
port="${OPENLOGOS_BENCH_PORT:-8792}"

for tool in python3 curl; do
  command -v "$tool" >/dev/null 2>&1 || { echo "$tool is not on PATH." >&2; exit 1; }
done
if [ ! -f "$works/institutes-allen.sqlite" ]; then
  echo "no Institutes pack in $works: run 'just institutes-pack' first." >&2
  exit 1
fi

cd "$works"
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
trap 'kill "$server" 2>/dev/null || true' EXIT
for _ in 1 2 3 4 5 6 7 8 9 10; do
  curl -fsI "http://127.0.0.1:$port/institutes-allen.sqlite" >/dev/null 2>&1 && break
  sleep 0.5
done
cd "$root"
# As scripts/preview-pack-bench-wasm.sh: fat LTO does not link the test harness.
CARGO_PROFILE_RELEASE_LTO=off \
  OPENLOGOS_WORK_BENCH_URL="http://127.0.0.1:$port" \
  scripts/wasm-test.sh --release --features retrieval -- --nocapture
