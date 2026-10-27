# Checks that only run on Linux: NixOS VM integration tests for the service modules
# (specs/06-nix-operations.md). flake.nix merges this set into `checks` on Linux and, on
# Darwin, names every entry in a warning instead of running it, so a skipped check is
# never reported as passed. Add VM tests as `name = pkgs.testers.runNixOSTest { ... };`.
{ pkgs }:
let
  # Unused until the first VM test lands; kept so additions do not change the signature.
  _ = pkgs;
in
{ }
