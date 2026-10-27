//! Render-ready text for every platform. This crate turns stored passages into attributed runs
//! (verse and footnote markers, poetry indentation, divine-name small caps, supported
//! red-letter markup) and translates token boundaries into each platform's string-offset unit,
//! UTF-16 for Apple, Android and the DOM, so renderers paint without reinterpreting text. It
//! owns the versioned normalization modes shared with indexing (case and final-sigma folding,
//! accent and breathing stripping for Greek; points and cantillation U+0591 to U+05C7 for
//! Hebrew) while keeping display text byte-exact; Unicode NFC is never applied to display
//! text. See specs/03-data-model.md, specs/19-search-and-retrieval.md and
//! docs/adr/0001-rust-core-native-ui.md.

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
