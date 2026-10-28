# OpenLogos — every workflow lives here. Run `just` to see them all.
# New to the repo? `just doctor`, then `just setup`, then `just test`.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Crates that must build for the browser worker (ADR 0001), including the binding surface and,
# since B064, the retrieval engine (SQLite compiled for wasm32 by sqlite-wasm-rs; the dev
# shells point the `cc` crate at an unwrapped clang for it) and, since B021, the sync crate
# with its pinned crypto crates (ADR 0004).
wasm_crates := "-p openlogos-core -p openlogos-text -p openlogos-collections -p openlogos-workspaces -p openlogos-notes -p openlogos-ink -p openlogos-conformance -p openlogos-sync -p openlogos-retrieval -p openlogos-retrieval-sqlite -p openlogos-ffi"

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

# B064 retrieval benchmarks (docs/evidence/B064.md): R5 vectors and R9 relations on synthetic data under `dir`, R6 if `tf` names an N1904 Text-Fabric folder (tf/1.0.0); extra arguments go to every subcommand
[group('2 · core')]
bench-retrieval dir="target/bench-retrieval" tf="" *args:
    cargo build --release -p openlogos-retrieval-sqlite --bin openlogos-retrieval-bench
    mkdir -p {{ dir }}
    if [ -n "{{ tf }}" ]; then target/release/openlogos-retrieval-bench ingest-tf "{{ tf }}" {{ dir }}/n1904.sqlite && target/release/openlogos-retrieval-bench r6 {{ dir }}/n1904.sqlite crates/retrieval-sqlite/bench/r6-queries.json --json {{ dir }}/r6-engine.json {{ args }}; fi
    target/release/openlogos-retrieval-bench r5 {{ dir }}/r5 {{ args }}
    target/release/openlogos-retrieval-bench r9 {{ dir }}/r9 {{ args }}

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

# B005: the DOM reader (apps/web/reader) round-trips its selection cases in headless Chrome through WebDriver; extra arguments go to the runner, e.g. `just test-dom-reader --shots artifacts/b005/shots --case cross-verse`
[group('6 · web')]
test-dom-reader *args:
    @scripts/dom-reader-test.sh {{ args }}

# B039: the web ink prototype (apps/web/ink) runs its palm cases, latency runs and export in headless Chrome through WebDriver; extra arguments go to the runner, e.g. `just test-ink-web --shots artifacts/b039/shots`
[group('6 · web')]
test-ink-web *args:
    @scripts/ink-web-test.sh {{ args }}

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

# B005: the TextKit 2 round-trip unit tests in the app on the iPad simulator `just sim` booted (apple shell); `just test-reader mac` runs them natively in the macOS app
[group('7 · apple app')]
test-reader platform="ipad":
    #!/usr/bin/env bash
    set -euo pipefail
    cd apps/apple/OpenLogos && xcodegen generate --quiet
    if [ "{{ platform }}" = mac ]; then
      xcodebuild -project OpenLogos.xcodeproj -scheme OpenLogosMac -configuration Debug -destination "platform=macOS,arch=arm64" -derivedDataPath ../../../artifacts/b005/DerivedData CODE_SIGNING_ALLOWED=NO -only-testing:OpenLogosMacTests test
    else
      xcodebuild -project OpenLogos.xcodeproj -scheme OpenLogos -configuration Debug -destination "id=$(cat ../../../artifacts/sim/device)" -derivedDataPath ../../../artifacts/sim/DerivedData CODE_SIGNING_ALLOWED=NO -only-testing:OpenLogosTests test
    fi

# B005: a picture of the TextKit 2 surface with one anchor case: `just b005-shot ipad cross-verse 1024` or `just b005-shot mac hebrew-elohim 1440` (apple shell)
[group('7 · apple app')]
b005-shot platform case width="1024":
    @scripts/b005-shots.sh {{ platform }} "{{ platform }}-anchors-{{ width }}-{{ case }}" {{ width }} {{ case }}

# A signed test build onto the shelf devices on the tailnet install from: `just ship ios` (apple shell; .envrc.local sets APPLE_TEAM_ID)
[group('7 · apple app')]
ship platform:
    @scripts/ship.sh {{ platform }}

# What this Mac still needs before it can ship, with the fix for each
[group('7 · apple app')]
ship-check:
    @scripts/ship.sh check

# Serve the shelf from this Mac at login (launchd) and share it with the tailnet (Tailscale Serve, own HTTPS port)
[group('7 · apple app')]
shelf-install:
    @scripts/ship.sh install

# Stop serving the shelf (its builds stay in its folder)
[group('7 · apple app')]
shelf-uninstall:
    @scripts/ship.sh uninstall

# Negative test: can THIS user read signing identities, ASC keys or profiles? Exits 1 if so (specs/20 wants agents isolated)
[group('7 · apple app')]
signing-isolation-check:
    @scripts/signing-isolation-check.sh
