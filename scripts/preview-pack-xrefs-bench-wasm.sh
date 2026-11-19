#!/usr/bin/env bash
# B057: a verse's cross-reference list latency in a dedicated worker in headless Chrome, over a
# built preview pack with its relation pack. Serves the pack's root on 127.0.0.1 with CORS and
# builds the worker test with the URL; the test (crates/ffi/tests/worker.rs) fetches
# relations/openbible-xrefs.sqlite and the BSB text pack and prints one `B057 wasm:` line. Run
# inside `nix develop .#web`.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
pack="${1:-${OPENLOGOS_PREVIEW_PACK:-$HOME/.cache/openlogos/preview-pack}}"
port="${OPENLOGOS_BENCH_PORT:-8792}"

for tool in python3 curl; do
  command -v "$tool" >/dev/null 2>&1 || { echo "$tool is not on PATH." >&2; exit 1; }
done
if [ ! -f "$pack/relations/openbible-xrefs.sqlite" ] || [ ! -f "$pack/packs/bsb/manifest.json" ]; then
  echo "no relation pack or BSB pack at $pack: run 'just preview-pack' and 'just preview-pack-xrefs' first." >&2
  exit 1
fi

cd "$pack"
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
  curl -fsI "http://127.0.0.1:$port/packs/bsb/manifest.json" >/dev/null 2>&1 && break
  sleep 0.5
done
cd "$root"
# As the B071 script: fat LTO does not link the test harness against sqlite-wasm-rs.
CARGO_PROFILE_RELEASE_LTO=off \
  OPENLOGOS_XREF_BENCH_URL="http://127.0.0.1:$port" \
  scripts/wasm-test.sh --release --features retrieval -- --nocapture
