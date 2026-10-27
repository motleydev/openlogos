# nix/checks

Linux-only checks, mainly NixOS VM integration tests for the modules under `nix/modules`
(specs/06-nix-operations.md). `default.nix` returns an attribute set that `flake.nix` adds to
`checks` on Linux; on macOS the same names are printed as skipped. No test exists yet; the first
one arrives with the first service module.
