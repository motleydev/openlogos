#!/usr/bin/env bash
# Look at the interfaces by hand and leave feedback on them, with no signing at all.
#
#   scripts/preview.sh web [--tailnet]   # build the wasm core if missing, serve apps/web on
#                                        # 127.0.0.1:8780 (OPENLOGOS_PREVIEW_PORT), print the pages;
#                                        # --tailnet also shares it at https://<mac>.<tailnet>.ts.net:8780
#   scripts/preview.sh web-stop          # stop that server (and its Tailscale Serve entry)
#   scripts/preview.sh mac               # build the macOS app (Debug, unsigned) and open it
#   scripts/preview.sh feedback "note"   # drag a region of the screen; the shot and the note land
#                                        # in ~/OpenLogosFeedback/<utc>/ (OPENLOGOS_FEEDBACK_DIR)
#
# The web server sends no-cache headers, so `just wasm-build` or an edit to a page shows on reload.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

port="${OPENLOGOS_PREVIEW_PORT:-8780}"
state="$root/artifacts/preview"
feedback_dir="${OPENLOGOS_FEEDBACK_DIR:-$HOME/OpenLogosFeedback}"
mkdir -p "$state"

# Re-enter through a dev shell when the tools it provides are needed.
in_shell() {
  local shell="$1"
  shift
  if [[ "${OPENLOGOS_DEVSHELL:-}" != "$shell" ]]; then
    command -v nix >/dev/null 2>&1 || PATH="/nix/var/nix/profiles/default/bin:$PATH"
    exec nix develop "$root#$shell" --command bash "$root/scripts/preview.sh" "$@"
  fi
}

web() {
  local tailnet=0
  [[ "${1:-}" == --tailnet ]] && tailnet=1
  if [[ ! -f apps/web/core-host/pkg/openlogos_ffi_bg.wasm ]]; then
    in_shell web web "$@"
    echo "▸ wasm core missing, building it (just wasm-build)"
    scripts/wasm-bindings.sh
  fi
  if [[ -f "$state/web.pid" ]] && kill -0 "$(cat "$state/web.pid")" 2>/dev/null; then
    echo "  ✓ already serving on port $port"
  elif lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
    echo "  ✗ port $port is taken by another program; set OPENLOGOS_PREVIEW_PORT" >&2
    exit 1
  else
    # A plain static server: .wasm needs its media type for streaming compilation, and
    # no-cache keeps a reload honest after a rebuild.
    nohup /usr/bin/python3 -c '
import http.server, sys
class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map,
                      ".wasm": "application/wasm", ".js": "text/javascript", ".mjs": "text/javascript"}
    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()
http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])),
    lambda *a, **k: Handler(*a, directory=sys.argv[2], **k)).serve_forever()
' "$port" "$root/apps/web" >"$state/web.log" 2>&1 &
    echo $! >"$state/web.pid"
    for _ in $(seq 1 50); do
      curl -fsS -o /dev/null "http://127.0.0.1:$port/" 2>/dev/null && break
      sleep 0.1
    done
    echo "  ✓ serving apps/web on http://127.0.0.1:$port (log: artifacts/preview/web.log)"
  fi
  local base="http://127.0.0.1:$port"
  if [[ $tailnet == 1 ]]; then
    tailscale serve --bg --https="$port" "http://127.0.0.1:$port" >/dev/null
    local host
    host="$(tailscale status --json | /usr/bin/python3 -c 'import json,sys; print(json.load(sys.stdin)["Self"]["DNSName"].rstrip("."))')"
    base="https://$host:$port"
    echo "  ✓ shared with the tailnet"
  fi
  echo
  echo "  $base/                 index of the pages"
  echo "  $base/reader/          DOM reader: three panes, selection, highlights"
  echo "  $base/ink/             ink margin: pen, highlighter, eraser"
  echo
  echo "  Feedback: just feedback \"what you saw\"   (shots and notes in $feedback_dir)"
}

web_stop() {
  if [[ -f "$state/web.pid" ]] && kill "$(cat "$state/web.pid")" 2>/dev/null; then
    echo "  ✓ stopped the server on port $port"
  else
    echo "  ✓ no preview server was running"
  fi
  rm -f "$state/web.pid"
  tailscale serve --https="$port" off >/dev/null 2>&1 || true
}

mac() {
  in_shell apple mac
  for variable in DEVELOPER_DIR SDKROOT; do
    if [[ "${!variable:-}" == /nix/store/* ]]; then unset "$variable"; fi
  done
  if [[ ! -f artifacts/apple/generated/OpenLogosCore.swift ]]; then
    echo "▸ bindings missing, building them (scripts/apple-bindings.sh --no-test)"
    scripts/apple-bindings.sh --no-test >"$state/bindings.log" 2>&1 \
      || { tail -20 "$state/bindings.log"; exit 1; }
  fi
  echo "▸ building OpenLogosMac (Debug, unsigned; log: artifacts/preview/mac-build.log)"
  (cd apps/apple/OpenLogos && xcodegen generate --quiet)
  xcodebuild -project apps/apple/OpenLogos/OpenLogos.xcodeproj -scheme OpenLogosMac \
    -configuration Debug -destination "platform=macOS,arch=arm64" \
    -derivedDataPath artifacts/mac/DerivedData CODE_SIGNING_ALLOWED=NO build \
    >"$state/mac-build.log" 2>&1 || { grep -E "error:" "$state/mac-build.log" | head -20; exit 1; }
  local app
  app="$(find artifacts/mac/DerivedData/Build/Products/Debug -maxdepth 1 -name '*.app' | head -1)"
  pkill -f "$app/Contents/MacOS/" 2>/dev/null || true
  open "$app"
  echo "  ✓ opened $app"
  echo "  The bar at the top switches view, layout, state and width."
  echo "  Feedback: just feedback \"what you saw\"   (shots and notes in $feedback_dir)"
}

feedback() {
  local note="${*:-}"
  local stamp dir
  stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  dir="$feedback_dir/$stamp"
  mkdir -p "$dir"
  echo "▸ drag a region (space toggles window capture, esc skips the picture)"
  screencapture -i -x "$dir/shot.png" || true
  {
    echo "# Feedback $stamp"
    echo
    echo "- commit: $(git rev-parse --short HEAD)$(git diff --quiet || echo '+') on $(git branch --show-current)"
    echo "- picture: $([[ -f "$dir/shot.png" ]] && echo shot.png || echo none)"
    echo
    echo "${note:-(no note)}"
  } >"$dir/note.md"
  echo "  ✓ $dir"
}

case "${1:-}" in
  web) shift; web "$@" ;;
  web-stop) web_stop ;;
  mac) mac ;;
  feedback) shift; feedback "$@" ;;
  *)
    echo "usage: $0 {web [--tailnet]|web-stop|mac|feedback \"note\"}" >&2
    exit 2
    ;;
esac
