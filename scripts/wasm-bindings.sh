#!/usr/bin/env bash
# Builds the web bindings of crates/ffi (B061): wasm-pack --target web into the gitignored
# apps/web/core-host/pkg/, then wasm-opt -Oz over the module, printing its size before and
# after (raw and gzip -9) for the budgets in specs/01-product.md. One build serves the Node
# test host and the browser. Run inside `nix develop .#web`; `just wasm-build` does so.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/apps/web/core-host/pkg"
module="$out/openlogos_ffi_bg.wasm"

for tool in cargo wasm-pack wasm-bindgen wasm-opt gzip; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool is not on PATH: run this from 'nix develop .#web'." >&2
    exit 1
  fi
done

# The wasm-bindgen CLI and crate must be the same release, or the generated JS does not
# match the module's ABI. flake.nix pins the CLI; crates/ffi/Cargo.toml pins the crate.
cli_version="$(wasm-bindgen --version | awk '{print $2}')"
crate_version="$(cd "$root" && cargo pkgid wasm-bindgen | sed 's/.*@//')"
if [ "$cli_version" != "$crate_version" ]; then
  echo "wasm-bindgen CLI is $cli_version but the crate is $crate_version; pin the crate to the CLI (crates/ffi/Cargo.toml)." >&2
  exit 1
fi

size() { wc -c <"$1" | tr -d ' '; }
gzipped() { gzip -9 -c "$1" | wc -c | tr -d ' '; }

echo "wasm-pack $(wasm-pack --version | awk '{print $2}'), wasm-bindgen $cli_version, $(wasm-opt --version)"
(cd "$root" && wasm-pack build crates/ffi --target web --release --out-dir "$out")

before="$(size "$module")"
before_gz="$(gzipped "$module")"
# wasm-bindgen drops rustc's target_features section, so the features Rust's
# wasm32-unknown-unknown target enables by default are named for the validator.
wasm-opt -Oz \
  --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
  --enable-mutable-globals --enable-reference-types --enable-multivalue \
  -o "$module.opt" "$module"
mv "$module.opt" "$module"
after="$(size "$module")"
after_gz="$(gzipped "$module")"

echo "openlogos_ffi_bg.wasm: $before bytes before wasm-opt -Oz ($before_gz gzipped), $after after ($after_gz gzipped)"
echo "output: $out"
