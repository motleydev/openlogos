//! The binding surface for native and web hosts. Everything Swift, Kotlin, TypeScript and the
//! Tauri shell can call lives here and nowhere else: UniFFI exports for Apple and Android
//! (B061) and wasm-bindgen exports for the web reader and the Obsidian plugin (B064), both
//! generated from the same versioned API over crates/core and its siblings. Bindings are
//! versioned with the core; a breaking core change updates every platform in one change set
//! (docs/adr/0001-rust-core-native-ui.md). This crate contains no domain logic. Neither UniFFI
//! nor wasm-bindgen is a dependency yet: B002 only establishes the crate.

#[cfg(target_arch = "wasm32")]
pub mod wasm;

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
