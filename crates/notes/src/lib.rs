//! First-class study notes and their relationship to an external vault. This crate decides
//! which copy of a note is authoritative when the app and an Obsidian vault both hold it, owns
//! the merge contracts that protect external writing, and keeps note anchors stable through
//! the core's reanchoring rules (specs/12-workspace-ux.md, specs/13-obsidian-and-ink.md).
//! Broader bidirectional vault editing is gated separately by B045 and B060.

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
