#!/usr/bin/env bash
# B039: bundles the Excalidraw comparison page (apps/web/ink/excalidraw) so it loads offline.
# `npm ci` installs the pinned @excalidraw/excalidraw 0.18.1 + React 18.3.1 from the lockfile
# (node_modules is gitignored), then esbuild (nixpkgs, pinned by the flake's nixpkgs input; not
# in the web shell, fetched with `nix run`) bundles entry.jsx into dist/ (gitignored) with the
# editor's CSS, fonts and locale chunks beside it. Run inside `nix develop .#web`.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$root/apps/web/ink/excalidraw"
cd "$dir"
command -v npm >/dev/null 2>&1 || { echo "npm is not on PATH: run this from 'nix develop .#web'." >&2; exit 1; }

if [ ! -d node_modules/@excalidraw/excalidraw ]; then
  npm ci --no-audit --no-fund --ignore-scripts
fi
version="$(node -p 'JSON.parse(require("fs").readFileSync("node_modules/@excalidraw/excalidraw/package.json","utf8")).version')"
[ "$version" = "0.18.1" ] || { echo "@excalidraw/excalidraw is $version, expected 0.18.1" >&2; exit 1; }

rm -rf dist
mkdir -p dist
nix run nixpkgs#esbuild -- entry.jsx --bundle --format=esm --splitting --outdir=dist \
  --conditions=production --jsx=automatic --define:process.env.NODE_ENV='"production"' --define:process.env.IS_PREACT='"false"' \
  --loader:.woff2=file --loader:.woff=file --loader:.ttf=file --loader:.svg=dataurl --loader:.png=dataurl \
  --target=es2022 --log-level=warning --minify-whitespace
# Excalidraw loads its fonts and locales at runtime from EXCALIDRAW_ASSET_PATH (./dist/).
cp -R node_modules/@excalidraw/excalidraw/dist/prod/fonts dist/fonts
cp -R node_modules/@excalidraw/excalidraw/dist/prod/locales dist/locales 2>/dev/null || true
du -sh dist | awk '{print "dist: "$1}'
for f in dist/entry.js dist/entry.css; do printf '%s %s bytes\n' "$f" "$(wc -c <"$f" | tr -d ' ')"; done
