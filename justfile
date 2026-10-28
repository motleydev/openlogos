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
