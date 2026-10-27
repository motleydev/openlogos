# ops

Runbooks, host profiles and the Apple build entry point.

- `xcode-version.txt`: the Xcode version the project builds and tests with. `nix develop .#apple`
  and `just doctor` compare it with `xcodebuild -version` through `scripts/xcode-check.sh` and warn
  on mismatch. Update it in its own commit when the project moves to a new Xcode.
- `apple-build` (not yet written): the single checked-in entry point that runs a named Apple build
  or test job, locally or over SSH on the Mac mini (specs/20-orchestration-and-build-hosts.md).

Signing identities, provisioning profiles and App Store Connect keys never live here or in any
agent-accessible checkout; see `apps/apple/README.md`.
