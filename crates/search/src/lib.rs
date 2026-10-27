//! The deterministic query engine described in specs/19-search-and-retrieval.md. Keyword,
//! phrase and Boolean search run on FTS5 through one shared Greek and Hebrew normalizing
//! tokenizer; lemma, morphology, proximity, agreement, syntax and alignment queries run on a
//! columnar token engine of dictionary-encoded columns and roaring bitmaps, modeled on
//! Text-Fabric's slot, feature and edge model rather than Lucene. Faceted concordances and
//! prose full text share the same query planner so results are identical on every platform and
//! well inside the specs/01 offline budgets. The engine lands with B064; this crate currently
//! only reserves the name.

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
