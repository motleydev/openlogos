#!/usr/bin/env bash
# Negative test for specs/20-orchestration-and-build-hosts.md: an agent run must not be able
# to read signing material. This reports whether the CURRENT user can, and exits non-zero
# when it can. Run it as the user agents run as; "0 problems" there is the acceptance.
#
#   scripts/signing-isolation-check.sh            # checks and a verdict; exit 1 if anything is readable
#   scripts/signing-isolation-check.sh --quiet    # the verdict line only
#
# It only reads: no keychain is created, unlocked or changed, no key is moved, no user is
# created. What it looks at:
#   1. Apple code-signing identities visible to this user's keychain search list
#      (`security find-identity -v -p codesigning`).
#   2. An App Store Connect API key named by ASC_KEY_PATH (.envrc.local), and the usual
#      ~/.private_keys and ~/.appstoreconnect/private_keys folders.
#   3. Provisioning profiles in this user's Xcode library.
# Each readable item is one problem. Hashes, key contents and names are never printed; the
# identity line is reduced to its count and the certificate type.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root" || exit 2

quiet=0
for arg in "$@"; do
  case "$arg" in
    --quiet) quiet=1 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

# direnv loads .envrc.local into your shell; a shell without direnv gets it here.
if [[ -z "${DIRENV_DIR:-}" && -f .envrc.local ]]; then
  set +u
  # shellcheck source=/dev/null
  source .envrc.local
  set -u
fi

problems=0
say() { ((quiet)) || printf '%s\n' "$*"; }
readable() { # <description>
  say "  READABLE  $1"
  problems=$((problems + 1))
}
blocked() { say "  blocked   $1"; }

say "Signing isolation check as user '$(id -un)' (uid $(id -u)) on $(hostname -s)"

say "1. Code-signing identities in the keychain search list"
if [[ "$(uname -s)" != Darwin ]]; then
  blocked "not macOS: no Apple keychain here"
else
  identities="$(security find-identity -v -p codesigning 2>/dev/null | grep -E '^ +[0-9]+\)' || true)"
  if [[ -z "$identities" ]]; then
    blocked "no valid code-signing identity is visible to this user"
  else
    while IFS= read -r line; do
      # "  1) <40 hex> "Apple Development: Name (TEAMID)"" -> the certificate type only.
      kind="$(printf '%s' "$line" | sed -n 's/.*"\([^:"]*\):.*/\1/p')"
      readable "a '${kind:-code-signing}' identity (private key usable by this user)"
    done <<<"$identities"
  fi
  say "   keychains searched: $(security list-keychains -d user 2>/dev/null | tr -d ' "' | paste -sd, - || echo unknown)"
fi

say "2. App Store Connect API keys"
found_key=0
if [[ -n "${ASC_KEY_PATH:-}" ]]; then
  if [[ -r "$ASC_KEY_PATH" ]]; then readable "ASC_KEY_PATH (.envrc.local) is readable"; found_key=1; else blocked "ASC_KEY_PATH is set but not readable by this user"; fi
fi
for dir in "$HOME/.private_keys" "$HOME/.appstoreconnect/private_keys" "$HOME/private_keys"; do
  if [[ -d "$dir" ]]; then
    count="$(find "$dir" -maxdepth 1 -name 'AuthKey_*.p8' -readable 2>/dev/null | wc -l | tr -d ' ')"
    if [[ "$count" -gt 0 ]]; then readable "$count readable AuthKey_*.p8 in $dir"; found_key=1; fi
  fi
done
((found_key)) || blocked "no App Store Connect API key readable by this user"

say "3. Provisioning profiles"
profile_dirs=("$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles" "$HOME/Library/MobileDevice/Provisioning Profiles")
profiles=0
for dir in "${profile_dirs[@]}"; do
  if [[ -d "$dir" ]]; then
    n="$(find "$dir" -maxdepth 1 -name '*.mobileprovision' -readable 2>/dev/null | wc -l | tr -d ' ')"
    profiles=$((profiles + n))
  fi
done
if ((profiles > 0)); then
  readable "$profiles provisioning profile(s) in this user's Xcode library"
else
  blocked "no provisioning profiles readable by this user"
fi

if ((problems > 0)); then
  printf 'NOT ISOLATED: user %s can read %d piece(s) of signing material. Agents must not run as this user (specs/20: separate macOS user and keychain for the release lane).\n' "$(id -un)" "$problems"
  exit 1
fi
printf 'ISOLATED: user %s can read no signing material.\n' "$(id -un)"
exit 0
