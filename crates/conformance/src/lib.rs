//! The shared conformance suite of docs/adr/0001-rust-core-native-ui.md. Small licensed or
//! synthetic fixtures (a Hebrew passage with cantillation, cross-verse selection, Psalm 9/10
//! numbering, bidi isolation, large text) and their expected anchors, offsets and search
//! results are kept here and run against every binding and renderer, asserting that a
//! selection produces the same core anchor and a highlight paints the same scalar range on
//! iPad, Mac, phone, web and Android. The fixtures listed in B061 are the first content; this
//! crate currently only reserves the name.

/// The crate's package name, as Cargo records it.
///
/// Placeholder API so the skeleton has one documented item and one test per crate; the
/// real surface arrives with the tasks named in the module documentation.
#[must_use]
pub fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_is_part_of_the_openlogos_workspace() {
        assert!(crate_name().starts_with("openlogos-"));
    }
}
