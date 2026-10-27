# crates

The Rust workspace: one core shared by every platform (docs/adr/0001-rust-core-native-ui.md).
Each crate's `lib.rs` opens with a paragraph stating its responsibility; specs/06-nix-operations.md
lists them together.

| Crate | Responsibility |
|---|---|
| `core` | references, versification mappings, anchors, typed claims; builds for wasm |
| `text` | render-ready attributed runs, normalization, offset adapters; builds for wasm |
| `search` | corpus query engine and prose full text (specs/19) |
| `retrieval` | vector and hybrid retrieval; the ADR 0002 storage traits |
| `package` | signed content-addressed packs, hashes, index layers, link index |
| `collections` | study collections, export receipts, Markdown/JSON export |
| `workspaces` | pane trees, link groups, layout profiles |
| `ink` | portable stroke geometry and snapshot contracts |
| `notes` | app/vault note authority and merge contracts |
| `sync` | encrypted operation log and conflict resolution |
| `ffi` | UniFFI and wasm-bindgen surfaces, versioned API (B061, B064) |
| `conformance` | cross-platform fixtures and expected results |

## Build and test

Inside `nix develop .#core` (or after `direnv allow`):

```sh
just test            # cargo nextest + doc tests, whole workspace
just lint            # rustfmt, clippy -D warnings, nixfmt, shellcheck, actionlint, blueprint
just check-wasm      # cargo check --target wasm32-unknown-unknown for core and text
just ci              # lint + test + check-wasm, as CI runs them in the shell
just check           # nix flake check: the same checks as hermetic derivations
cargo test -p openlogos-core          # one crate
cargo doc --workspace --no-deps --open
```

Toolchain: Rust pinned in `flake.nix` (`rustVersion`) and mirrored by `rust-version` in
`Cargo.toml`. Cross targets: `wasm32-unknown-unknown` and `aarch64-linux-android` everywhere;
`aarch64-apple-darwin`, `aarch64-apple-ios`, `aarch64-apple-ios-sim` in the Darwin shells.
Workspace lints live in the root `Cargo.toml`; release profile is tuned for size.
