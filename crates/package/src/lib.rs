//! Corpus packs and the device-local catalog. A pack is one read-only, content-addressed,
//! signed SQLite file per edition or work, carrying text, search indexes, token columns and
//! bitmaps, alignment and edge tables, an optional vector layer and a provenance manifest
//! (specs/19-search-and-retrieval.md, Package layers). This crate owns the safe archive
//! format, hash and signature verification before any pack is attached, separately versioned
//! index and enrichment layers, and the device-local link index and catalog database that
//! merge outbound edges across installed packs. The same code runs in the build pipeline and
//! in every reader, so a shipped index is byte-identical to the one produced at build time.
//! See specs/03-data-model.md, specs/05-sharing-security.md and specs/08-quality-governance.md.

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
