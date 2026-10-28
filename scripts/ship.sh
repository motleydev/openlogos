#!/usr/bin/env bash
# Signed test builds of the OpenLogos app shell from this Mac onto the shelf, a folder of
# builds with a page your iPad or iPhone installs from over Tailscale. After sudoku-color's
# scripts/ship.sh (specs/20-orchestration-and-build-hosts.md, "Interim operation").
#
#   scripts/ship.sh ios         # xcodegen, xcodebuild archive (signed), export (ad hoc), onto the shelf
#   scripts/ship.sh install     # serve the shelf from this Mac at login, shared by Tailscale Serve
#   scripts/ship.sh uninstall   # stop serving it (the builds stay in the shelf's folder)
#   scripts/ship.sh check       # what this Mac still needs, with the fix for each
#
# Settings come from .envrc.local (git-ignored; .envrc.local.example lists them):
#
#   APPLE_TEAM_ID          your Apple developer team (developer.apple.com/account → Membership)
#   ASC_KEY_PATH, ASC_KEY_ID, ASC_ISSUER_ID
#                          an App Store Connect API key, so xcodebuild can talk to the portal
#                          without an Apple ID signed into Xcode (optional)
#   OPENLOGOS_SHELF_DIR    the shelf's folder (default ~/OpenLogosShelf)
#   OPENLOGOS_SHELF_PORT   the port it listens on, on this Mac only, and the HTTPS port
#                          Tailscale Serve exposes it on (default 8767; sudoku-color has 443)
#   OPENLOGOS_SHELF_URL    the shelf's address; when unset, this Mac's tailnet name and port
#   IOS_EXPORT_METHOD      release-testing (ad hoc, the default) or debugging (development)
#
# The shelf is self-contained: /usr/bin/python3's static file server under a launchd agent,
# an index.html and one itms-services manifest.plist per build, all written by this script.
# sudoku-color's shelf is a .NET tool specific to that repository and is not reused.
#
# Every build carries a unique CFBundleVersion (UTC yyjjjHHMM) and the commit it came from;
# nothing is committed or bumped in the tree. A dirty tree is marked "+" on the shelf.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if [[ "${OPENLOGOS_DEVSHELL:-}" != apple ]]; then
  command -v nix >/dev/null 2>&1 || PATH="/nix/var/nix/profiles/default/bin:$PATH"
  command -v nix >/dev/null 2>&1 || { echo "Nix is not installed; see README.md, Getting started." >&2; exit 1; }
  exec nix develop "$root#apple" --command bash "$root/scripts/ship.sh" "$@"
fi

# direnv loads .envrc.local into your shell; a shell without direnv gets it here.
if [[ -z "${DIRENV_DIR:-}" && -f .envrc.local ]]; then
  set +eu
  # shellcheck source=/dev/null
  source .envrc.local
  set -eu
fi

for variable in DEVELOPER_DIR SDKROOT; do
  if [[ "${!variable:-}" == /nix/store/* ]]; then unset "$variable"; fi
done

if [[ -t 1 ]]; then
  green=$'\e[32m' yellow=$'\e[33m' red=$'\e[31m' dim=$'\e[2m' bold=$'\e[1m' reset=$'\e[0m'
else
  green='' yellow='' red='' dim='' bold='' reset=''
fi

bundle_id="org.openlogos.app"
app_dir="$root/apps/apple/OpenLogos"
project="$app_dir/OpenLogos.xcodeproj"
scheme="OpenLogos"
xcframework="$root/artifacts/apple/OpenLogosCoreFFI.xcframework"
generated_swift="$root/artifacts/apple/generated/OpenLogosCore.swift"
work="$root/artifacts/ship/ios"
shelf_dir="${OPENLOGOS_SHELF_DIR:-$HOME/OpenLogosShelf}"
port="${OPENLOGOS_SHELF_PORT:-8767}"
label="org.openlogos.shelf"
agent="$HOME/Library/LaunchAgents/$label.plist"
keep_builds=10
problems=0

step() { printf '%s▸ %s%s\n' "$bold" "$1" "$reset"; }
ok() { printf '  %s✓%s %s\n' "$green" "$reset" "$1"; }
warn() { printf '  %s!%s %s\n    %s→ %s%s\n' "$yellow" "$reset" "$1" "$dim" "$2" "$reset"; }
fail() { printf '  %s✗%s %s\n    %s→ %s%s\n' "$red" "$reset" "$1" "$dim" "$2" "$reset"; problems=$((problems + 1)); }
section() { printf '\n%s%s%s\n' "$bold" "$1" "$reset"; }
die() {
  printf '%s✗ %s%s\n' "$red" "$1" "$reset" >&2
  if [[ -n "${2:-}" ]]; then printf '  %s→ %s%s\n' "$dim" "$2" "$reset" >&2; fi
  exit 1
}
# The last lines of a tool's log, then stop.
die_with_log() { # <log> <what failed> <what to try>
  printf '%s… %s%s\n' "$dim" "$1" "$reset" >&2
  grep -E 'error:|Error |failed|FAILED' "$1" | tail -n 15 >&2 || tail -n 30 "$1" >&2
  die "$2" "$3 (the whole log: $1)"
}

on_mac() { [[ "$(uname -s)" == Darwin ]]; }

tailscale_cli() {
  if command -v tailscale >/dev/null 2>&1; then
    echo tailscale
  elif [[ -x /Applications/Tailscale.app/Contents/MacOS/Tailscale ]]; then
    echo /Applications/Tailscale.app/Contents/MacOS/Tailscale
  else
    return 1
  fi
}

# The shelf's address: OPENLOGOS_SHELF_URL, or this Mac's tailnet name on the shelf's port.
shelf_url() {
  if [[ -n "${OPENLOGOS_SHELF_URL:-}" ]]; then
    echo "${OPENLOGOS_SHELF_URL%/}"
    return
  fi
  local ts name
  ts="$(tailscale_cli)" || return 1
  name="$("$ts" status --json 2>/dev/null | jq -r '.Self.DNSName // empty')" || return 1
  [[ -n "$name" ]] || return 1
  echo "https://${name%.}:$port"
}

serving() { curl -fsS -o /dev/null --max-time 3 "http://127.0.0.1:$port/shelf.json" 2>/dev/null; }

# A minimal page over shelf.json: newest build first, one Install link each.
write_index() {
  local url="$1" rows
  rows="$(jq -r --arg url "$url" '
    .builds | sort_by(.number) | reverse | .[] |
    "<li><a class=\"install\" href=\"itms-services://?action=download-manifest&amp;url=\($url)/builds/\(.number)/manifest.plist\">Install \(.app_version) (\(.number))</a>" +
    "<span class=\"meta\">\(.date) · \(.commit)\(if .dirty then "+" else "" end) · \(.notes | gsub("&"; "&amp;") | gsub("<"; "&lt;"))</span></li>"' \
    "$shelf_dir/shelf.json")"
  cat >"$shelf_dir/index.html" <<EOF
<!doctype html>
<html lang="en">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>OpenLogos shelf</title>
<style>
  body { font: 18px/1.5 ui-serif, Georgia, serif; max-width: 36em; margin: 3em auto; padding: 0 1.5em; color: #111; background: #fff; }
  @media (prefers-color-scheme: dark) { body { color: #eee; background: #111; } a { color: #eee; } }
  h1 { font-weight: normal; font-size: 1.6em; }
  ul { list-style: none; padding: 0; }
  li { margin: 1.2em 0; }
  .install { font-size: 1.1em; }
  .meta { display: block; font-size: 0.8em; opacity: 0.7; }
  p.note { font-size: 0.85em; opacity: 0.8; }
</style>
<h1>OpenLogos test builds</h1>
<ul>
$rows
</ul>
<p class="note">Open this page in Safari on the device with Tailscale connected. The device must be
registered with the Apple developer team before the build is made (ad hoc signing). After
Install, the app appears on the Home Screen; the first launch may ask to trust the developer
under Settings → General → VPN &amp; Device Management.</p>
EOF
}

# Add an .ipa to the shelf: builds/<number>/, its manifest, shelf.json, index.html.
shelve() { # <ipa> <app_version> <number> <commit> <dirty 0|1> <notes>
  local ipa="$1" app_version="$2" number="$3" commit="$4" dirty="$5" notes="$6" url dest
  url="$(shelf_url)" || die "The shelf has no address." \
    "start Tailscale on this Mac and sign in, or set OPENLOGOS_SHELF_URL in .envrc.local"
  dest="$shelf_dir/builds/$number"
  mkdir -p "$dest"
  cp "$ipa" "$dest/OpenLogos.ipa"
  cat >"$dest/manifest.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>items</key>
  <array>
    <dict>
      <key>assets</key>
      <array>
        <dict>
          <key>kind</key>
          <string>software-package</string>
          <key>url</key>
          <string>$url/builds/$number/OpenLogos.ipa</string>
        </dict>
      </array>
      <key>metadata</key>
      <dict>
        <key>bundle-identifier</key>
        <string>$bundle_id</string>
        <key>bundle-version</key>
        <string>$app_version</string>
        <key>kind</key>
        <string>software</string>
        <key>title</key>
        <string>OpenLogos $app_version ($number)</string>
      </dict>
    </dict>
  </array>
</dict>
</plist>
EOF
  [[ -f "$shelf_dir/shelf.json" ]] || echo '{"app":"OpenLogos","builds":[]}' >"$shelf_dir/shelf.json"
  local tmp
  tmp="$(mktemp)"
  jq --arg number "$number" --arg app_version "$app_version" --arg commit "$commit" \
    --argjson dirty "$([[ "$dirty" == 1 ]] && echo true || echo false)" --arg notes "$notes" \
    --arg date "$(date -u +%Y-%m-%dT%H:%MZ)" --arg bundle_id "$bundle_id" \
    --arg size "$(wc -c <"$ipa" | tr -d ' ')" \
    '.builds = ([.builds[] | select(.number != $number)] + [{
       number: $number, app_version: $app_version, bundle_id: $bundle_id, platform: "ios",
       commit: $commit, dirty: $dirty, notes: $notes, date: $date, size: ($size | tonumber)
     }] | sort_by(.number))' "$shelf_dir/shelf.json" >"$tmp"
  mv "$tmp" "$shelf_dir/shelf.json"
  # The newest few only; the rest are deleted from the folder and the list.
  for old in $(jq -r --argjson keep "$keep_builds" '.builds | sort_by(.number) | reverse | .[$keep:] | .[].number' "$shelf_dir/shelf.json"); do
    rm -rf "${shelf_dir:?}/builds/$old"
    jq --arg n "$old" '.builds = [.builds[] | select(.number != $n)]' "$shelf_dir/shelf.json" >"$tmp"
    mv "$tmp" "$shelf_dir/shelf.json"
  done
  write_index "$url"
  ok "$dest/OpenLogos.ipa ($(du -h "$ipa" | cut -f1 | tr -d ' '))"
  ok "$url/  (install from there with Tailscale on)"
  if ! serving; then
    warn "Nothing serves the shelf on this Mac yet." "run 'just shelf-install' once"
  fi
}

ship_ios() {
  on_mac || die "iOS builds need a Mac with Xcode."
  xcodebuild -version >/dev/null 2>&1 \
    || die "Xcode is missing." "install it, open it once, then: sudo xcodebuild -license accept"
  [[ -n "${APPLE_TEAM_ID:-}" ]] || die "APPLE_TEAM_ID is not set." \
    "add 'export APPLE_TEAM_ID=…' to .envrc.local (developer.apple.com/account → Membership details → Team ID)"

  local number commit dirty=0 notes auth=() app_version
  number="$(date -u +%y%j%H%M)"
  commit="$(git rev-parse --short HEAD)"
  notes="$(git log -1 --format=%s)"
  if [[ -n "$(git status --porcelain)" ]]; then dirty=1; fi
  if [[ -n "${ASC_KEY_PATH:-}" && -n "${ASC_KEY_ID:-}" && -n "${ASC_ISSUER_ID:-}" ]]; then
    auth=(-authenticationKeyPath "$ASC_KEY_PATH" -authenticationKeyID "$ASC_KEY_ID"
      -authenticationKeyIssuerID "$ASC_ISSUER_ID")
  fi

  if [[ ! -d "$xcframework" || ! -f "$generated_swift" ]]; then
    step "Bindings: artifacts/apple is missing, building it (scripts/apple-bindings.sh --no-test)"
    mkdir -p "$work"
    scripts/apple-bindings.sh --no-test >"$work/bindings.log" 2>&1 \
      || die_with_log "$work/bindings.log" "Could not build the bindings." "see scripts/apple-bindings.sh"
  fi

  step "XcodeGen: apps/apple/OpenLogos/project.yml → OpenLogos.xcodeproj"
  xcodegen generate --spec "$app_dir/project.yml" --project "$app_dir" --quiet

  rm -rf "$work"
  mkdir -p "$work"
  step "Xcode: archiving build $number from $commit$([[ $dirty == 1 ]] && echo '+') (log: artifacts/ship/ios/archive.log)"
  SECONDS=0
  xcodebuild -project "$project" -scheme "$scheme" -configuration Release \
    -destination generic/platform=iOS -archivePath "$work/OpenLogos.xcarchive" \
    -allowProvisioningUpdates ${auth[@]+"${auth[@]}"} \
    DEVELOPMENT_TEAM="$APPLE_TEAM_ID" CODE_SIGN_STYLE=Automatic \
    CURRENT_PROJECT_VERSION="$number" archive >"$work/archive.log" 2>&1 \
    || die_with_log "$work/archive.log" "Xcode could not archive the app." \
      "signing? Xcode → Settings → Accounts must list the team, or set the ASC_* keys; 'just ship-check' looks for the rest"
  ok "archive in ${SECONDS}s"

  cat >"$work/ExportOptions.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>method</key>
  <string>${IOS_EXPORT_METHOD:-release-testing}</string>
  <key>teamID</key>
  <string>$APPLE_TEAM_ID</string>
  <key>signingStyle</key>
  <string>automatic</string>
</dict>
</plist>
EOF
  step "Xcode: exporting, signed for the devices registered with the team (log: artifacts/ship/ios/export.log)"
  SECONDS=0
  xcodebuild -exportArchive -archivePath "$work/OpenLogos.xcarchive" -exportPath "$work/export" \
    -exportOptionsPlist "$work/ExportOptions.plist" -allowProvisioningUpdates ${auth[@]+"${auth[@]}"} \
    >"$work/export.log" 2>&1 \
    || die_with_log "$work/export.log" "Xcode could not export a signed build." \
      "is at least one device registered with the team? apps/apple/README.md, Signing"
  ok "export in ${SECONDS}s"

  local ipa
  ipa="$(find "$work/export" -maxdepth 1 -name '*.ipa' | head -n 1)"
  [[ -n "$ipa" ]] || die "Xcode made no .ipa." "see $work/export.log"

  # The manifest must name what the .ipa holds, or the device refuses it.
  unzip -q -o "$ipa" 'Payload/*.app/Info.plist' -d "$work/info"
  local plists=("$work"/info/Payload/*.app/Info.plist)
  bundle_id="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "${plists[0]}")"
  app_version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "${plists[0]}")"

  step "The shelf: adding it ($shelf_dir)"
  shelve "$ipa" "$app_version" "$number" "$commit" "$dirty" "$notes"
}

install_service() {
  on_mac || die "The service is for macOS (launchd)."
  local ts address
  mkdir -p "$shelf_dir/builds" "$HOME/Library/LaunchAgents"
  [[ -f "$shelf_dir/shelf.json" ]] || echo '{"app":"OpenLogos","builds":[]}' >"$shelf_dir/shelf.json"
  address="$(shelf_url || echo "https://<this-mac>.<tailnet>.ts.net:$port")"
  write_index "$address"

  launchctl bootout "gui/$(id -u)/$label" >/dev/null 2>&1 || true
  for _ in $(seq 1 20); do
    launchctl print "gui/$(id -u)/$label" >/dev/null 2>&1 || break
    sleep 0.5
  done

  step "Serving $shelf_dir on 127.0.0.1:$port whenever you are logged in ($agent)"
  # /usr/bin/python3 is Xcode's; the shelf is static files, so its http.server is enough.
  cat >"$agent" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$label</string>
  <key>ProgramArguments</key>
  <array>
    <string>/usr/bin/python3</string>
    <string>-m</string>
    <string>http.server</string>
    <string>$port</string>
    <string>--bind</string>
    <string>127.0.0.1</string>
    <string>--directory</string>
    <string>$shelf_dir</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>$shelf_dir/.server.log</string>
  <key>StandardErrorPath</key>
  <string>$shelf_dir/.server.log</string>
</dict>
</plist>
EOF
  launchctl bootstrap "gui/$(id -u)" "$agent"
  for _ in $(seq 1 30); do
    serving && break
    sleep 0.5
  done
  serving || die "The shelf's server did not start." "see $shelf_dir/.server.log"
  ok "http://127.0.0.1:$port/ answers on this Mac"

  ts="$(tailscale_cli)" || die "Tailscale's command line is missing." \
    "install Tailscale (tailscale.com/download/mac), sign in, then 'just shelf-install' again"
  step "Tailscale Serve: HTTPS port $port on the tailnet → 127.0.0.1:$port (sudoku-color keeps 443)"
  echo "  (the first time, Tailscale may print a link to turn on HTTPS certificates: open it, then wait here)"
  "$ts" serve --bg --https="$port" "http://127.0.0.1:$port"
  if address="$(shelf_url)"; then
    ok "Open $address on the iPad or iPhone, with Tailscale on."
  fi
}

uninstall_service() {
  local ts
  if on_mac; then
    launchctl bootout "gui/$(id -u)/$label" >/dev/null 2>&1 || true
    rm -f "$agent"
  fi
  # Only this shelf's port; another app's Serve configuration on this Mac is left alone.
  if ts="$(tailscale_cli)"; then "$ts" serve --https="$port" off >/dev/null 2>&1 || true; fi
  ok "The shelf is no longer served; its builds stay in $shelf_dir."
}

check() {
  local identities sleep_after ts dns address
  section "This Mac"
  if on_mac; then
    ok "macOS $(sw_vers -productVersion)"
    sleep_after="$(pmset -g 2>/dev/null | awk '$1 == "sleep" { print $2; exit }')"
    if [[ "$sleep_after" == 0 ]]; then
      ok "never sleeps"
    else
      warn "sleeps after ${sleep_after:-some} minutes" \
        "System Settings → Energy: turn on 'Prevent automatic sleeping when the display is off'"
    fi
  else
    fail "not a Mac" "iOS builds need macOS with Xcode"
  fi

  section "Xcode and signing"
  if on_mac && xcodebuild -version >/dev/null 2>&1; then
    ok "$(xcodebuild -version | head -n 1)"
  else
    fail "Xcode is missing" "install Xcode, open it once, then: sudo xcodebuild -license accept"
  fi
  if command -v xcodegen >/dev/null 2>&1; then ok "xcodegen $(xcodegen --version | awk '{print $NF}')"; else
    fail "xcodegen missing" "it comes from the apple shell: nix develop .#apple"
  fi
  if [[ -d "$xcframework" && -f "$generated_swift" ]]; then ok "bindings in artifacts/apple"; else
    warn "no bindings in artifacts/apple yet" "'just bindings-swift' (ship builds them if missing)"
  fi
  if [[ -f .envrc.local ]]; then ok ".envrc.local present"; else
    fail ".envrc.local is missing" "cp .envrc.local.example .envrc.local and fill in APPLE_TEAM_ID"
  fi
  if [[ -n "${APPLE_TEAM_ID:-}" ]]; then ok "APPLE_TEAM_ID is $APPLE_TEAM_ID"; else
    fail "APPLE_TEAM_ID is not set" "add 'export APPLE_TEAM_ID=…' to .envrc.local"
  fi
  if [[ -n "${ASC_KEY_PATH:-}" ]]; then
    if [[ -r "$ASC_KEY_PATH" ]]; then ok "ASC API key at $ASC_KEY_PATH (id ${ASC_KEY_ID:-unset})"; else
      fail "ASC_KEY_PATH=$ASC_KEY_PATH is not readable" "check the path in .envrc.local"
    fi
  else
    ok "no ASC API key: xcodebuild uses the Apple ID signed into Xcode"
  fi
  if on_mac; then
    identities="$(security find-identity -v -p codesigning 2>/dev/null | grep -cE 'Apple (Development|Distribution)' || true)"
    if [[ "${identities:-0}" -gt 0 ]]; then
      ok "$identities Apple signing identities in this user's keychain (specs/20: agents must not have this)"
    else
      warn "no Apple signing identity in this user's keychain" \
        "Xcode → Settings → Accounts: add the Apple ID; the first 'just ship ios' makes the certificates"
    fi
  fi

  section "Tailscale and the shelf"
  if ts="$(tailscale_cli)"; then
    dns="$("$ts" status --json 2>/dev/null | jq -r '.Self.DNSName // empty' || true)"
    if [[ -n "$dns" ]]; then ok "on the tailnet as ${dns%.}"; else
      fail "Tailscale is not connected" "open Tailscale and sign in"
    fi
    if "$ts" serve status 2>/dev/null | grep -q "127.0.0.1:$port"; then
      ok "Tailscale Serve forwards HTTPS port $port to the shelf"
    else
      fail "Tailscale Serve does not forward to the shelf" "run 'just shelf-install'"
    fi
  else
    fail "Tailscale is not installed" "tailscale.com/download/mac, then sign in"
  fi
  if on_mac && launchctl print "gui/$(id -u)/$label" >/dev/null 2>&1; then
    ok "the shelf starts at login ($label)"
  else
    fail "the shelf is not installed as a service" "run 'just shelf-install'"
  fi
  if serving; then ok "the shelf answers on this Mac (port $port)"; else
    fail "nothing answers on port $port" "run 'just shelf-install'; its log is $shelf_dir/.server.log"
  fi
  if address="$(shelf_url)"; then
    if curl -fsS -o /dev/null --max-time 10 "$address/shelf.json" 2>/dev/null; then
      ok "devices on the tailnet reach it at $address"
    else
      fail "$address does not answer" \
        "Tailscale admin console → DNS: MagicDNS and HTTPS Certificates on, then 'just shelf-install'"
    fi
  fi

  printf '\n'
  if ((problems > 0)); then
    printf '%s%d thing(s) to fix%s, above.\n' "$red" "$problems" "$reset"
    exit 1
  fi
  printf '%sReady to ship.%s just ship ios\n' "$green" "$reset"
}

case "${1:-}" in
  ios) ship_ios ;;
  install) install_service ;;
  uninstall) uninstall_service ;;
  check) check ;;
  *)
    echo "usage: $0 {ios|install|uninstall|check}" >&2
    exit 2
    ;;
esac
