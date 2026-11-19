#!/usr/bin/env bash
# `just ci-hosts`: every host lane that GitHub's `nix flake check` does not run, one after
# another, each in the dev shell it needs. Run it on the Mac mini before opening a pull
# request that touches the core's API, the bindings or an app; `just ci` covers the Rust
# workspace itself. Lanes:
#   web       wasm build, Node test host, wasm-bindgen tests, web shell cases   (.#web)
#   kotlin    Kotlin bindings and the JVM test host                             (.#android)
#   swift     Swift bindings and the Swift test host                            (.#apple, macOS)
#   mac-app   the Mac app's tests through xcodebuild                            (.#apple, macOS)
# The lanes never overlap: test-swift regenerates artifacts/apple, which xcodebuild reads.
#   scripts/ci-hosts.sh                  # every lane this machine can run
#   scripts/ci-hosts.sh web kotlin       # only the lanes named
# Each lane's output goes to artifacts/ci-hosts/<lane>.log; a failed lane does not stop the
# ones after it, and the exit status is 1 if any lane failed.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
logs="$root/artifacts/ci-hosts"
mkdir -p "$logs"
cd "$root"

lanes=("$@")
if [ ${#lanes[@]} -eq 0 ]; then
  lanes=(web kotlin)
  [ "$(uname -s)" = Darwin ] && lanes+=(swift mac-app)
fi

# lane <name> <shell> <just recipe and arguments>...
run_lane() {
  local name="$1" shell="$2"
  shift 2
  local started=$SECONDS
  echo "▸ $name: nix develop .#$shell --command $*"
  if nix develop ".#$shell" --command "$@" >>"$logs/$name.log" 2>&1; then
    return 0
  fi
  echo "  ✗ failed after $((SECONDS - started)) s; last lines of $logs/$name.log:"
  tail -15 "$logs/$name.log" | sed 's/^/    /'
  return 1
}

failed=()
summary=()
for lane in "${lanes[@]}"; do
  : >"$logs/$lane.log"
  started=$SECONDS
  ok=1
  case "$lane" in
    web)
      run_lane web web just test-web \
        && run_lane web web just test-wasm \
        && run_lane web web just test-shell-web || ok=0
      ;;
    kotlin) run_lane kotlin android just test-kotlin || ok=0 ;;
    swift) run_lane swift apple just test-swift || ok=0 ;;
    mac-app) run_lane mac-app apple just test-reader mac || ok=0 ;;
    *)
      echo "unknown lane '$lane' (web, kotlin, swift, mac-app)" >&2
      exit 2
      ;;
  esac
  if [ "$ok" = 1 ]; then
    summary+=("✓ $lane ($((SECONDS - started)) s)")
  else
    summary+=("✗ $lane ($((SECONDS - started)) s)")
    failed+=("$lane")
  fi
done

echo
printf '%s\n' "${summary[@]}"
if [ ${#failed[@]} -gt 0 ]; then
  echo "failed: ${failed[*]} (logs in artifacts/ci-hosts/)"
  exit 1
fi
