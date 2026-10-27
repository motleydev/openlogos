//! Handwriting and margin ink as portable data. This crate owns stroke geometry, the
//! coordinate spaces that tie ink to a reading snapshot, and the snapshot contracts of
//! specs/13-obsidian-and-ink.md so that a page annotated with Apple Pencil remains readable,
//! and re-anchorable, on platforms without a stylus. The reference fixtures are
//! examples/reading-snapshot.json and examples/ink-document.json, whose coordinate-space
//! association tools/validate.py checks.

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
