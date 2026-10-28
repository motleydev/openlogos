//! The domain core every platform shares. This crate owns scripture references and their
//! parsing, versification schemes and the compiled maps between them (pivoting through the
//! Paratext `org` scheme), stable token anchors with reanchoring across editions, and the
//! typed claims and provenance kinds of specs/03-data-model.md. It has no I/O, no platform
//! dependencies and no allocation-heavy indexes of its own, so it compiles for every native
//! target and for `wasm32-unknown-unknown`; the search, retrieval, package and sync crates
//! build on it, and crates/ffi exposes it through a versioned API (B061). Identical answers on
//! iPad, Mac, phone, web, Android and in the Obsidian plugin are the whole point
//! (docs/adr/0001-rust-core-native-ui.md).

pub mod anchor;
pub mod reference;
pub mod versification;

/// Version of the core API that bindings (UniFFI, wasm-bindgen) are generated against.
///
/// Independent of the crate version in `Cargo.toml`, which may move with internal changes.
/// The rule: a breaking change to any public type or function of `openlogos-core`,
/// `openlogos-text` or `openlogos-conformance` (a removed or renamed item, a changed field,
/// variant, signature or serialized form) bumps MAJOR, or MINOR while the MAJOR is 0; an
/// additive change bumps MINOR, or PATCH while the MAJOR is 0. Bindings are generated against
/// this value and must embed it, so a host can refuse a core whose API version it was not
/// built for (docs/adr/0001-rust-core-native-ui.md).
pub const API_VERSION: &str = "0.1.0";

/// [`API_VERSION`] as a function, for hosts that cannot read a constant through a binding.
#[must_use]
pub fn api_version() -> &'static str {
    API_VERSION
}

#[cfg(test)]
mod tests {
    use super::{API_VERSION, api_version};

    #[test]
    fn api_version_is_semver_shaped() {
        assert_eq!(api_version(), API_VERSION);
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
