//! Workspace layout as portable data. Three synchronized reading panes, the pane tree, link
//! groups that keep panes scrolling together, and the layout profiles that adapt to iPad, Mac,
//! phone, web and ultra-wide displays are all represented here so a workspace restores
//! identically on every platform (specs/12-workspace-ux.md,
//! specs/18-platform-capabilities.md). The reference fixture is examples/workspace.json,
//! validated by tools/validate.py.

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
