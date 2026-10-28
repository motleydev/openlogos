#!/usr/bin/env bash
# The OpenLogos app shell in the iOS Simulator, and pictures of it, with no signing at all.
#
#   scripts/sim.sh run [device]   # bindings if missing, xcodegen, xcodebuild, boot, install, launch
#   scripts/sim.sh shot [file]    # a screenshot of the simulator now, into artifacts/sim/shots/
#   scripts/sim.sh stop           # shut down the simulator `run` used
#
# The default device is an iPad (specs/18-platform-capabilities.md: iPadOS is the primary
# surface); name any simulator from `xcrun simctl list devices available` to use another.
# Optional: OPENLOGOS_SIM_TEXT_SIZE=UICTContentSizeCategoryAccessibilityL (any
# UIContentSizeCategory name) launches the app at that Dynamic Type size, for large-type
# screenshots. After sudoku-color's scripts/sim.sh (specs/20-orchestration-and-build-hosts.md).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# Everything runs inside the apple dev shell (Rust with the Apple targets, xcodegen, just);
# from anywhere else, re-enter through it. Nested `nix develop` is fine.
if [[ "${OPENLOGOS_DEVSHELL:-}" != apple ]]; then
  command -v nix >/dev/null 2>&1 || PATH="/nix/var/nix/profiles/default/bin:$PATH"
  command -v nix >/dev/null 2>&1 || { echo "Nix is not installed; see README.md, Getting started." >&2; exit 1; }
  exec nix develop "$root#apple" --command bash "$root/scripts/sim.sh" "$@"
fi

# A Nix shell on macOS may point these at Nix's SDK; Xcode needs its own (the shell hook
# already drops them; this keeps the script honest when run some other way).
for variable in DEVELOPER_DIR SDKROOT; do
  if [[ "${!variable:-}" == /nix/store/* ]]; then unset "$variable"; fi
done

if [[ -t 1 ]]; then
  green=$'\e[32m' red=$'\e[31m' dim=$'\e[2m' bold=$'\e[1m' reset=$'\e[0m'
else
  green='' red='' dim='' bold='' reset=''
fi
step() { printf '%s▸ %s%s\n' "$bold" "$1" "$reset"; }
ok() { printf '  %s✓%s %s\n' "$green" "$reset" "$1"; }
die() {
  printf '%s✗ %s%s\n' "$red" "$1" "$reset" >&2
  if [[ -n "${2:-}" ]]; then printf '  %s→ %s%s\n' "$dim" "$2" "$reset" >&2; fi
  exit 1
}

bundle_id="org.openlogos.app"
default_device="iPad Pro 11-inch (M5)"
app_dir="$root/apps/apple/OpenLogos"
project="$app_dir/OpenLogos.xcodeproj"
scheme="OpenLogos"
xcframework="$root/artifacts/apple/OpenLogosCoreFFI.xcframework"
generated_swift="$root/artifacts/apple/generated/OpenLogosCore.swift"
sim_dir="$root/artifacts/sim"
derived="$sim_dir/DerivedData"
shots="$sim_dir/shots"
device_file="$sim_dir/device" # the UDID `run` last used, for `shot` and `stop`

udid_of() { sed -n 's/.*(\([0-9A-F-]\{36\}\)).*/\1/p' | head -n 1; }

# A simulator to use: the one named, else the one `run` used last if it is still booted,
# else the default iPad, else any available iPad.
device_id() { # [name]
  local wanted="${1:-}" id
  if [[ -n "$wanted" ]]; then
    id="$(xcrun simctl list devices available | grep -F "$wanted (" | udid_of)"
    [[ -n "$id" ]] || die "No simulator called '$wanted'." "xcrun simctl list devices available"
    echo "$id"
    return
  fi
  if [[ -f "$device_file" ]]; then
    id="$(cat "$device_file")"
    if xcrun simctl list devices booted | grep -q "$id"; then
      echo "$id"
      return
    fi
  fi
  id="$(xcrun simctl list devices available | grep -F "$default_device (" | udid_of)"
  if [[ -z "$id" ]]; then
    id="$(xcrun simctl list devices available | grep -E '^ +iPad' | head -n 1 | udid_of)"
  fi
  [[ -n "$id" ]] || die "No iPad simulator is available." "Xcode → Settings → Components: add an iOS runtime"
  echo "$id"
}

device_name() { xcrun simctl list devices | grep -F "$1" | sed 's/^ *//; s/ (Shutdown)//; s/ (Booted)//'; }

run() {
  local device
  device="$(device_id "${1:-}")"
  mkdir -p "$sim_dir"
  echo "$device" >"$device_file"

  if [[ ! -d "$xcframework" || ! -f "$generated_swift" ]]; then
    step "Bindings: artifacts/apple is missing, building it (scripts/apple-bindings.sh --no-test)"
    scripts/apple-bindings.sh --no-test >"$sim_dir/bindings.log" 2>&1 \
      || { tail -n 30 "$sim_dir/bindings.log" >&2; die "Could not build the bindings." "the whole log: artifacts/sim/bindings.log"; }
    ok "artifacts/apple/OpenLogosCoreFFI.xcframework"
  fi

  step "XcodeGen: apps/apple/OpenLogos/project.yml → OpenLogos.xcodeproj"
  xcodegen generate --spec "$app_dir/project.yml" --project "$app_dir" --quiet

  step "Xcode: building $scheme for $(device_name "$device") (no signing; log: artifacts/sim/build.log)"
  mkdir -p "$derived"
  SECONDS=0
  if ! xcodebuild -project "$project" -scheme "$scheme" -configuration Debug \
    -destination "id=$device" -derivedDataPath "$derived" \
    CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY="" \
    build >"$sim_dir/build.log" 2>&1; then
    grep -E 'error:' "$sim_dir/build.log" | tail -n 15 >&2 || tail -n 30 "$sim_dir/build.log" >&2
    die "Xcode could not build for the simulator." "the whole log: artifacts/sim/build.log"
  fi
  local app
  app="$(find "$derived/Build/Products" -maxdepth 2 -name 'OpenLogos.app' -path '*iphonesimulator*' -type d | head -n 1)"
  [[ -n "$app" ]] || die "Xcode built no OpenLogos.app." "see artifacts/sim/build.log"
  ok "$app (${SECONDS}s)"

  step "Simulator: booting, installing and launching"
  xcrun simctl boot "$device" 2>/dev/null || true
  xcrun simctl bootstatus "$device" -b >/dev/null 2>&1 || true
  open -a Simulator --args -CurrentDeviceUDID "$device" 2>/dev/null || true
  xcrun simctl install "$device" "$app"
  local launch_args=()
  if [[ -n "${OPENLOGOS_SIM_TEXT_SIZE:-}" ]]; then
    launch_args+=(-UIPreferredContentSizeCategoryName "$OPENLOGOS_SIM_TEXT_SIZE")
  fi
  xcrun simctl terminate "$device" "$bundle_id" >/dev/null 2>&1 || true
  xcrun simctl launch "$device" "$bundle_id" ${launch_args[@]+"${launch_args[@]}"} >/dev/null
  ok "$(device_name "$device") is running $bundle_id."
  ok "A picture of it: just sim-shot"
}

shot() {
  local device file
  device="$(device_id)"
  mkdir -p "$shots"
  file="${1:-$shots/$(date -u +%Y%m%d-%H%M%S).png}"
  # The app has usually drawn by now; a fresh launch gets a moment.
  sleep 1
  xcrun simctl io "$device" screenshot "$file" >/dev/null 2>&1 \
    || die "Could not take a screenshot." "is the simulator booted? 'just sim' first"
  ok "$file"
}

stop() {
  local device
  if [[ -f "$device_file" ]]; then
    device="$(cat "$device_file")"
    xcrun simctl shutdown "$device" 2>/dev/null || true
    ok "$(device_name "$device") shut down."
  else
    ok "Nothing to stop: scripts/sim.sh run has not booted a simulator here."
  fi
}

case "${1:-run}" in
  run) shift; run "${1:-}" ;;
  shot) shift; shot "${1:-}" ;;
  stop) stop ;;
  *) die "Usage: scripts/sim.sh {run [device]|shot [file]|stop}" ;;
esac
