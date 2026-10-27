# nix/darwin

macOS developer and CI environment definitions for the Mac mini build host
(specs/20-orchestration-and-build-hosts.md). The `.#apple` shell in `flake.nix` already verifies
the recorded Xcode version (`ops/xcode-version.txt`); host-level configuration lands here when the
build host is provisioned (B063).
