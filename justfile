# OpenLogos — every workflow lives here. Run `just` to see them all.
# New to the repo? `just doctor`, then `just setup`, then `just test`.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Crates that must build for the browser worker (ADR 0001), including the binding surface.
wasm_crates := "-p openlogos-core -p openlogos-text -p openlogos-conformance -p openlogos-ffi"

[private]
default:
    @just --list --unsorted

# Check this machine has every expected tool, with the fix for anything missing
[group('1 · start here')]
doctor:
    @scripts/doctor.sh

# One-time setup after cloning: git hooks
[group('1 · start here')]
setup:
    lefthook install
    @echo "Done. Next: just doctor, then just test."

# Run every Rust test (unit, integration and doc tests); extra arguments go to nextest, e.g. `just test -p openlogos-core`
[group('2 · core')]
test *args:
    cargo nextest run --workspace --all-targets {{ args }}
    # nextest does not run doc tests yet.
    cargo test --workspace --doc

# Type-check the crates that ship as WebAssembly, without a browser toolchain
[group('2 · core')]
check-wasm:
    cargo check --target wasm32-unknown-unknown {{ wasm_crates }}

# Format Rust, Nix and this justfile in place
[group('2 · core')]
fmt:
    cargo fmt --all
    find . -name '*.nix' -not -path './target/*' -not -path './.direnv/*' -print0 | xargs -0 nixfmt
    just --fmt --unstable

# Everything that must be clean before merging: formatting, clippy, shell, workflows, blueprint
[group('2 · core')]
lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    find . -name '*.nix' -not -path './target/*' -not -path './.direnv/*' -print0 | xargs -0 nixfmt --check
    just --fmt --check --unstable
    LANG="${LANG:-C.UTF-8}" shellcheck scripts/*.sh
    actionlint
    just validate

# Exactly what CI runs inside the dev shell (CI also runs `just check`)
[group('2 · core')]
ci: lint test check-wasm

# Validate the blueprint data (schemas, backlog DAG, source IDs); needs the pipeline or ops shell
[group('3 · blueprint')]
validate:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! python3 -c 'import jsonschema' 2>/dev/null; then
      echo "python3 with jsonschema is not on PATH: run this from 'nix develop .#pipeline' or '.#ops'." >&2
      exit 1
    fi
    python3 tools/validate.py

# Every Nix check (Rust, lint and blueprint) as CI runs them; Linux-only checks are named and skipped on macOS
[group('4 · nix')]
check:
    nix flake check --print-build-logs

# Update the pinned toolchains (nixpkgs and rust-overlay) to their newest releases
[group('4 · nix')]
nix-update:
    nix flake update
    @echo "Review flake.lock, bump rustVersion in flake.nix if you want the newer Rust, run 'just ci' and 'just check', then commit."

# Remove build output
[group('4 · nix')]
clean:
    cargo clean
    rm -rf result result-*

# Build crates/ffi for macOS, iOS and the simulator, generate the Swift bindings and assemble artifacts/apple/OpenLogosCoreFFI.xcframework (apple shell)
[group('5 · bindings')]
bindings-swift:
    scripts/apple-bindings.sh --no-test

# bindings-swift, then `swift test` in apps/apple/CoreHost (apple shell)
[group('5 · bindings')]
test-swift:
    scripts/apple-bindings.sh

# Build crates/ffi as a shared library for this machine and generate the Kotlin bindings into apps/android/CoreHost (android shell)
[group('5 · bindings')]
bindings-kotlin:
    scripts/kotlin-bindings.sh --no-test

# bindings-kotlin, then `gradle test` in apps/android/CoreHost (android shell)
[group('5 · bindings')]
test-kotlin:
    scripts/kotlin-bindings.sh

# Build the web bindings (wasm-pack --target web, then wasm-opt -Oz) into apps/web/core-host/pkg, printing sizes
[group('6 · web')]
wasm-build:
    @scripts/wasm-bindings.sh

# Clippy for the wasm32 target, including the browser worker tests
[group('6 · web')]
lint-wasm:
    cargo clippy --target wasm32-unknown-unknown -p openlogos-ffi --all-targets -- -D warnings

# Run the wasm-bindgen tests of crates/ffi in a dedicated worker in headless Chrome (installed on the machine; the matching chromedriver is fetched once); extra arguments go to wasm-pack, e.g. `just test-wasm -- --nocapture`
[group('6 · web')]
test-wasm *args:
    @scripts/wasm-test.sh {{ args }}

# Run the Node test host (apps/web/core-host) against the built bindings; builds them first
[group('6 · web')]
test-web: wasm-build
    cd apps/web/core-host && npm test

# Build the app shell for an iPad simulator (unsigned), boot it, install and launch: `just sim` or `just sim "iPad mini (A17 Pro)"` (apple shell)
[group('7 · apple app')]
sim device="":
    @scripts/sim.sh run "{{ device }}"

# A screenshot of the running simulator into artifacts/sim/shots/ (or the file named)
[group('7 · apple app')]
sim-shot file="":
    @scripts/sim.sh shot "{{ file }}"

# Shut down the simulator `just sim` booted
[group('7 · apple app')]
sim-stop:
    @scripts/sim.sh stop
