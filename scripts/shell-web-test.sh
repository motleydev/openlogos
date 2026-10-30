#!/usr/bin/env bash
# `just test-shell-web`: the B072 web workspace shell (apps/web/shell) runs its cases in headless
# Chrome through WebDriver Actions (trusted key and pointer events), with the chromedriver
# scripts/wasm-test.sh caches for the Chrome installed on this machine (set CHROMEDRIVER to use
# another). Needs the wasm build (`just wasm-build`). Extra arguments go to the Node runner, e.g.
#   scripts/shell-web-test.sh --case seed-session --bench --samples 200 --shots docs/evidence/B072
#   scripts/shell-web-test.sh --headed --bench          # a visible Chrome window
# After scripts/dom-reader-test.sh.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cache="${XDG_CACHE_HOME:-$HOME/.cache}/openlogos/chromedriver"

if [ ! -f "$root/apps/web/core-host/pkg/openlogos_ffi_bg.wasm" ]; then
  echo "apps/web/core-host/pkg is missing: run 'just wasm-build' first." >&2
  exit 1
fi
command -v node >/dev/null 2>&1 || { echo "node is not on PATH: run this from 'nix develop .#web'." >&2; exit 1; }

if [ -z "${WEBDRIVER_URL:-}" ] && [ -z "${CHROMEDRIVER:-}" ]; then
  chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
  [ -x "$chrome" ] || chrome="$(command -v google-chrome || command -v chromium || true)"
  [ -n "$chrome" ] || { echo "Google Chrome was not found; set CHROMEDRIVER or WEBDRIVER_URL." >&2; exit 1; }
  version="$("$chrome" --version | grep -Eo '[0-9]+(\.[0-9]+){3}' | head -1)"
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) os=mac-arm64 ;; Darwin-x86_64) os=mac-x64 ;; Linux-x86_64) os=linux64 ;;
    *) echo "no Chrome for Testing build for this platform; set CHROMEDRIVER." >&2; exit 1 ;;
  esac
  CHROMEDRIVER="$cache/$version/chromedriver-$os/chromedriver"
  if [ ! -x "$CHROMEDRIVER" ]; then
    url="https://storage.googleapis.com/chrome-for-testing-public/$version/$os/chromedriver-$os.zip"
    echo "Fetching chromedriver $version for $os..."
    mkdir -p "$cache/$version"
    curl -fsSL -o "$cache/$version/chromedriver.zip" "$url"
    unzip -qo "$cache/$version/chromedriver.zip" -d "$cache/$version"
    rm -f "$cache/$version/chromedriver.zip"
  fi
  export CHROMEDRIVER
fi

cd "$root"
exec node apps/web/shell/test/run-chrome.mjs "$@"
