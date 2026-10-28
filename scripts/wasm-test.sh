#!/usr/bin/env bash
# Runs the wasm-bindgen tests of crates/ffi (a dedicated worker in headless Chrome) with a
# chromedriver that matches the Chrome installed on this machine. wasm-pack 0.15.0 fetches
# a chromedriver by itself, but the one it installed here (ChromeDriver 148) could not drive
# Chrome 156 and every session failed with HTTP 404, so this script fetches the exact
# release from Chrome for Testing into a cache directory and passes it with --chromedriver.
# Set CHROMEDRIVER to use a driver you already have. Extra arguments go to wasm-pack, e.g.
# `-- --nocapture`. Run inside `nix develop .#web`; `just test-wasm` does so.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cache="${XDG_CACHE_HOME:-$HOME/.cache}/openlogos/chromedriver"

for tool in wasm-pack curl unzip; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool is not on PATH: run this from 'nix develop .#web' (curl and unzip come from the OS)." >&2
    exit 1
  fi
done

chrome_version() {
  local candidate
  for candidate in \
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
    "$HOME/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
    google-chrome google-chrome-stable chromium chromium-browser; do
    if command -v "$candidate" >/dev/null 2>&1; then
      "$candidate" --version | grep -Eo '[0-9]+(\.[0-9]+){3}' | head -1
      return 0
    fi
  done
  return 1
}

platform() {
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) echo mac-arm64 ;;
    Darwin-x86_64) echo mac-x64 ;;
    Linux-x86_64) echo linux64 ;;
    *)
      echo "no Chrome for Testing build for $(uname -s)-$(uname -m); set CHROMEDRIVER." >&2
      exit 1
      ;;
  esac
}

if [ -z "${CHROMEDRIVER:-}" ]; then
  version="$(chrome_version)" || {
    echo "Google Chrome was not found; install it or set CHROMEDRIVER." >&2
    exit 1
  }
  os="$(platform)"
  CHROMEDRIVER="$cache/$version/chromedriver-$os/chromedriver"
  if [ ! -x "$CHROMEDRIVER" ]; then
    url="https://storage.googleapis.com/chrome-for-testing-public/$version/$os/chromedriver-$os.zip"
    echo "Fetching chromedriver $version for $os from Chrome for Testing..."
    mkdir -p "$cache/$version"
    if ! curl -fsSL -o "$cache/$version/chromedriver.zip" "$url"; then
      echo "no chromedriver published for Chrome $version at $url; set CHROMEDRIVER to a matching driver." >&2
      exit 1
    fi
    unzip -qo "$cache/$version/chromedriver.zip" -d "$cache/$version"
    rm -f "$cache/$version/chromedriver.zip"
  fi
fi

echo "Chrome $("$CHROMEDRIVER" --version | awk '{print $2}') driver: $CHROMEDRIVER"
cd "$root"
exec wasm-pack test --headless --chrome --chromedriver "$CHROMEDRIVER" crates/ffi "$@"
