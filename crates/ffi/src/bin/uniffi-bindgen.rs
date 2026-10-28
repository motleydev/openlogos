//! `uniffi-bindgen`, built from the same uniffi version as the scaffolding in this crate so
//! the generated Swift and Kotlin always match the library they load. Run it as
//! `cargo run -p openlogos-ffi --features bindgen --bin uniffi-bindgen -- generate --library ...`;
//! `scripts/apple-bindings.sh` and `scripts/kotlin-bindings.sh` do so.

fn main() {
    uniffi::uniffi_bindgen_main();
}
