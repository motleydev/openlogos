#!/usr/bin/env bash
# `just doctor`: checks that this machine has everything the project expects and says
# exactly how to fix what is missing. Exits non-zero only when something required is
# missing; Xcode and the Android SDK are optional for work on the Rust core.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root" || exit 1

if [[ -t 1 ]]; then
  green=$'\e[32m' yellow=$'\e[33m' red=$'\e[31m' dim=$'\e[2m' bold=$'\e[1m' reset=$'\e[0m'
else
  green='' yellow='' red='' dim='' bold='' reset=''
fi

failures=0
warnings=0
ok() { printf '  %s✓%s %s\n' "$green" "$reset" "$1"; }
warn() { printf '  %s!%s %s\n    %s→ %s%s\n' "$yellow" "$reset" "$1" "$dim" "$2" "$reset"; warnings=$((warnings + 1)); }
fail() { printf '  %s✗%s %s\n    %s→ %s%s\n' "$red" "$reset" "$1" "$dim" "$2" "$reset"; failures=$((failures + 1)); }
section() { printf '\n%s%s%s\n' "$bold" "$1" "$reset"; }
have() { command -v "$1" >/dev/null 2>&1; }

section "Environment"
if have nix; then
  ok "nix $(nix --version 2>/dev/null | awk '{print $NF}')"
  if nix config show experimental-features 2>/dev/null | grep -q flakes; then
    ok "nix flakes enabled"
  else
    fail "nix flakes are not enabled" "add 'experimental-features = nix-command flakes' to ~/.config/nix/nix.conf"
  fi
else
  fail "nix is not installed" "install it: https://nixos.org/download (or the Determinate installer), then 'direnv allow'"
fi

if have direnv; then
  ok "direnv $(direnv version 2>/dev/null)"
else
  warn "direnv is not installed (the shell will not load automatically)" "install direnv + nix-direnv, hook it into your shell, then run 'direnv allow' here"
fi

shell_name="${OPENLOGOS_DEVSHELL:-}"
if [[ -n "$shell_name" ]]; then
  ok "inside the '$shell_name' dev shell"
else
  fail "not inside a project dev shell" "run 'direnv allow' in the repo (or 'nix develop'), then re-run 'just doctor'"
fi

# The toolchain downloads are large; a nearly full store fails in confusing ways.
# POSIX `df -Pk` works for both BSD df and the GNU coreutils df a Nix shell puts first.
if [[ -d /nix/store ]]; then
  free_gb="$(df -Pk /nix/store 2>/dev/null | awk 'NR == 2 { printf "%d", $4 / 1048576 }')"
  if [[ -n "$free_gb" ]]; then
    if ((free_gb < 10)); then
      warn "only ${free_gb} GB free for /nix/store" "free space or run 'nix store gc' before adding shells (the Rust toolchain alone is over 1 GB)"
    else
      ok "${free_gb} GB free for /nix/store"
    fi
  fi
fi

section "Toolchain"
want="$(sed -n 's/^ *rustVersion = "\([^"]*\)";.*/\1/p' flake.nix | head -1)"
if have rustc; then
  got="$(rustc --version 2>/dev/null | awk '{print $2}')"
  if [[ "$got" == "$want" ]]; then
    ok "rustc $got (flake.nix pins $want)"
  else
    fail "rustc $got does not match flake.nix ($want)" "use the dev shell's toolchain: 'direnv reload' (rustup proxies on PATH shadow it)"
  fi
else
  if [[ "$shell_name" == "ops" ]]; then
    ok "no Rust toolchain (the ops shell does not carry one)"
  else
    fail "rustc missing" "it comes from the dev shell: 'direnv allow'"
  fi
fi

for tool in just lefthook git jq nixfmt shellcheck actionlint; do
  if have "$tool"; then ok "$tool"; else fail "$tool missing" "it comes from the dev shell: 'direnv allow'"; fi
done

if have cargo-nextest; then
  ok "cargo-nextest"
elif [[ "$shell_name" != "ops" ]]; then
  warn "cargo-nextest missing ('just test' needs it)" "it comes from the dev shell: 'direnv reload'"
fi

if python3 -c 'import jsonschema' 2>/dev/null; then
  ok "python3 with jsonschema ('just validate')"
else
  warn "python3 with jsonschema not on PATH" "'just validate' needs the pipeline or ops shell: 'nix develop .#pipeline'"
fi

section "Repository"
hook_file="$(git rev-parse --git-path hooks/pre-commit 2>/dev/null)"
if [[ -n "$hook_file" && -f "$hook_file" ]] && grep -q lefthook "$hook_file" 2>/dev/null; then
  ok "git hooks installed (lefthook)"
else
  warn "git hooks not installed" "run 'just setup' (formats and checks before you commit)"
fi

if [[ "$(uname -s)" == "Darwin" ]]; then
  section "Apple (needed only for the Apple app and iOS targets)"
  if scripts/xcode-check.sh; then
    :
  else
    warn "Xcode does not match ops/xcode-version.txt (details above)" "install the recorded version from https://developer.apple.com/download/ or 'xcode-select -s' it"
  fi
  # The Mac mini lanes (B063): `just sim` needs a simulator runtime and xcodegen, `just ship`
  # an accepted license, Tailscale and .envrc.local. Warnings, not failures: the Rust core
  # does not need any of them.
  if command -v xcodebuild >/dev/null 2>&1; then
    if xcodebuild -checkFirstLaunchStatus >/dev/null 2>&1; then
      ok "Xcode license accepted and first launch done"
    else
      warn "Xcode license not accepted or first launch pending" "run 'sudo xcodebuild -license accept' and 'sudo xcodebuild -runFirstLaunch'"
    fi
    runtime="$(xcrun simctl list runtimes 2>/dev/null | awk '/^iOS/ { print $1, $2; exit }')"
    if [[ -n "$runtime" ]]; then
      ok "simulator runtime $runtime ('just sim')"
    else
      warn "no iOS simulator runtime installed" "Xcode → Settings → Components: add an iOS runtime, or 'xcodebuild -downloadPlatform iOS'"
    fi
  fi
  if have xcodegen; then
    ok "xcodegen $(xcodegen --version 2>/dev/null | awk '{print $NF}') (apps/apple/OpenLogos/project.yml)"
  elif [[ "$shell_name" == "apple" ]]; then
    fail "xcodegen missing" "it comes from the apple shell: 'direnv reload'"
  else
    ok "xcodegen not needed in the '$shell_name' shell (the apple shell carries it)"
  fi
  if have tailscale || [[ -x /Applications/Tailscale.app/Contents/MacOS/Tailscale ]]; then
    ts_cli="$(command -v tailscale || echo /Applications/Tailscale.app/Contents/MacOS/Tailscale)"
    ts_name="$("$ts_cli" status --json 2>/dev/null | jq -r '.Self.DNSName // empty' 2>/dev/null || true)"
    if [[ -n "$ts_name" ]]; then
      ok "tailscale $("$ts_cli" version 2>/dev/null | head -n 1), connected as ${ts_name%.}"
    else
      warn "tailscale is installed but not connected ('just ship' publishes over it)" "open Tailscale and sign in"
    fi
  else
    warn "tailscale is not installed (the shelf 'just ship' publishes to is served over it)" "https://tailscale.com/download/mac, then 'just shelf-install'"
  fi
  if [[ -f .envrc.local ]]; then
    ok ".envrc.local present (APPLE_TEAM_ID and the shelf settings for 'just ship')"
  else
    warn "no .envrc.local ('just ship' needs APPLE_TEAM_ID from it)" "cp .envrc.local.example .envrc.local and fill it in; 'just sim' works without"
  fi
fi

section "Android (needed only for the Android app)"
if [[ -n "${ANDROID_HOME:-}" && -d "${ANDROID_HOME}" ]]; then
  ok "ANDROID_HOME=$ANDROID_HOME"
else
  warn "ANDROID_HOME is not set" "install the SDK outside Nix and export ANDROID_HOME in .envrc.local (apps/android/README.md)"
fi
if [[ -n "${ANDROID_NDK_HOME:-}" && -d "${ANDROID_NDK_HOME}" ]]; then
  ok "ANDROID_NDK_HOME=$ANDROID_NDK_HOME"
else
  warn "ANDROID_NDK_HOME is not set (cargo-ndk needs it)" "install an NDK through the SDK manager and export ANDROID_NDK_HOME in .envrc.local"
fi

printf '\n'
if ((failures > 0)); then
  printf '%s%d problem(s)%s, %d warning(s). Fix the ✗ items above, then run just doctor again.\n' "$red" "$failures" "$reset" "$warnings"
  exit 1
fi
if ((warnings > 0)); then
  printf '%sReady%s, with %d suggestion(s) above. Try: just test\n' "$green" "$reset" "$warnings"
else
  printf '%sAll good.%s Try: just test · just check-wasm · just lint\n' "$green" "$reset"
fi
