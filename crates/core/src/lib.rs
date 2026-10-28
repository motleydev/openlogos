//! The domain core every platform shares. This crate owns scripture references and their
//! parsing, versification schemes and the compiled maps between them (pivoting through the
//! Paratext `org` scheme), stable token anchors with reanchoring across editions, and the
//! typed claims and provenance kinds of specs/03-data-model.md. It has no I/O, no platform
//! dependencies and no allocation-heavy indexes of its own, so it compiles for every native
//! target and for `wasm32-unknown-unknown`; the search, retrieval, package and sync crates
//! build on it, and crates/ffi exposes it through a versioned API (B061). Identical answers on
//! iPad, Mac, phone, web, Android and in the Obsidian plugin are the whole point
//! (docs/adr/0001-rust-core-native-ui.md).

pub mod reference;
pub mod versification;

/// Version of the core API that bindings (UniFFI, wasm-bindgen) are generated against.
///
/// Follows the workspace package version for now; B061 decides how breaking core changes are
/// versioned independently of the crate.
#[must_use]
pub fn api_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::api_version;

    #[test]
    fn api_version_is_semver_shaped() {
        let parts: Vec<&str> = api_version().split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "expected MAJOR.MINOR.PATCH, got {}",
            api_version()
        );
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
