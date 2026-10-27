//! Study collections and their export contracts. Readers gather passages, notes and resources
//! for teaching and preaching into collections; this crate owns the collection model,
//! deterministic Markdown and JSON export, the export receipts that make a handoff
//! reproducible, and reimport of what was exported. Obsidian is the sole first-class external
//! writing integration, so the bounded vault handoff of specs/13-obsidian-and-ink.md and
//! specs/15-reformed-study-and-collections.md is shaped here, not in the apps.

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
