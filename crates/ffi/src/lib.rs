//! The binding surface for native and web hosts. Everything Swift, Kotlin, TypeScript and the
//! Tauri shell can call lives here and nowhere else: UniFFI exports for Apple and Android
//! and wasm-bindgen exports for the web reader and the Obsidian plugin (B061), both
//! generated from the same versioned API over crates/core and its siblings. Bindings are
//! versioned with the core; a breaking core change updates every platform in one change set
//! (docs/adr/0001-rust-core-native-ui.md). This crate contains no domain logic. The UniFFI
//! surface lives in [`native`]; the wasm-bindgen surface (B061 item 3) in its own `wasm` module.

#[cfg(not(target_arch = "wasm32"))]
pub mod native;
// uniffi's proc macros look for the scaffolding tag at the crate root.
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub use native::UniFfiTag;

/// The core API version this binding surface was built against.
#[must_use]
pub fn core_api_version() -> &'static str {
    openlogos_core::api_version()
}

#[cfg(test)]
mod tests {
    use super::core_api_version;

    #[test]
    fn binding_surface_reports_the_core_version() {
        assert_eq!(core_api_version(), openlogos_core::api_version());
    }
}
