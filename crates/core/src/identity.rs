//! Identity records the pipeline mints and every reader relies on: how token ids are minted
//! for a verse, how ids of one content revision relate to the next (`id_lineage`), and the
//! standoff alignment records between two editions' tokens (specs/19-search-and-retrieval.md,
//! "Pipelines": Token identity, Alignment). Everything here is plain data plus validation;
//! the pipeline produces the records, packs carry them, and the core reads them identically
//! on every platform.

pub mod alignment;
pub mod lineage;
pub mod mint;

pub use alignment::{AlignmentError, AlignmentRecord, AlignmentSet, AlignmentStatus, OriginKind};
pub use lineage::{IdLineage, LineageEntry, LineageError, LineageKind, LineageResult};
pub use mint::{MintError, mint_verse, suffix_for};

#[cfg(test)]
mod tests {
    use super::{AlignmentSet, IdLineage};
    use crate::anchor::Anchor;
    use crate::versification::{ReferenceMapping, VerseMap};

    /// The example under `examples/` at the repository root, or `None` when the tree is
    /// filtered (the nix checks build from `crates/` alone).
    fn example(name: &str) -> Option<String> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/").to_owned() + name;
        match std::fs::read_to_string(&path) {
            Ok(text) => Some(text),
            Err(_) => {
                eprintln!("skipping {path}: not present in this source tree");
                None
            }
        }
    }

    #[test]
    fn the_schema_examples_parse_into_the_core_types_and_validate() {
        if let Some(text) = example("token-lineage.json") {
            let lineage: IdLineage = serde_json::from_str(&text).unwrap();
            lineage.validate().unwrap();
            assert_eq!(lineage.entries.len(), 5);
        }
        if let Some(text) = example("alignment.json") {
            let set: AlignmentSet = serde_json::from_str(&text).unwrap();
            set.validate().unwrap();
            assert_eq!(set.records.len(), 17);
            assert_eq!(
                serde_json::to_value(&set).unwrap(),
                serde_json::from_str::<serde_json::Value>(&text).unwrap()
            );
        }
        if let Some(text) = example("anchor.json") {
            let anchor: Anchor = serde_json::from_str(&text).unwrap();
            assert_eq!(anchor.quote.unwrap().exact, "loved the world");
            assert_eq!(anchor.tokens.len(), 3);
        }
        if let Some(text) = example("verse-map.json") {
            let map: VerseMap = serde_json::from_str(&text).unwrap();
            assert_eq!(map.entries.len(), 7);
        }
        if let Some(text) = example("reference-mapping.json") {
            let mapping: ReferenceMapping = serde_json::from_str(&text).unwrap();
            assert_eq!(mapping.to.len(), 2);
            assert_eq!(
                serde_json::to_value(&mapping).unwrap(),
                serde_json::from_str::<serde_json::Value>(&text).unwrap()
            );
        }
    }
}
