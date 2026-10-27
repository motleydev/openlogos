# apps/web

Thin TypeScript interface over semantic DOM text; the Rust core runs as WebAssembly in a worker
with OPFS persistence (docs/adr/0001-rust-core-native-ui.md). No project exists yet.

`nix develop .#web` provides Node.js 24 (LTS), `wasm-bindgen-cli`, `wasm-pack`, `binaryen`
(`wasm-opt`) and Rust with the `wasm32-unknown-unknown` target. `just check-wasm` already
type-checks `crates/core` and `crates/text` for that target; B064 adds the bindings.
