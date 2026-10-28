//! Study collections and their export contracts. Readers gather passages, notes and resources
//! for teaching and preaching into collections; this crate owns the collection model,
//! deterministic Markdown and JSON export, the export receipts that make a handoff
//! reproducible, and reimport of what was exported. Obsidian is the sole first-class external
//! writing integration, so the bounded vault handoff of specs/13-obsidian-and-ink.md and
//! specs/15-reformed-study-and-collections.md is shaped here, not in the apps.
//!
//! This revision (B006) fixes the identity and provenance contracts: [`StudyCollection`],
//! the source-bearing and non-source [`CollectionItem`] kinds, and [`VaultExportReceipt`].
//! Export, reconciliation and reimport logic follow in B058/B060 on top of these types.
//! The JSON forms are `schemas/study-collection.schema.json` and
//! `schemas/vault-export-receipt.schema.json`.

pub mod collection;
pub mod receipt;

pub use collection::{
    CollectionError, CollectionItem, Display, EvidenceRef, Group, ITEM_ID_PATTERN, ItemBody,
    Locator, PdfLocator, Quotation, RightsSnapshot, SourceProvenance, StudyCollection, StudyTarget,
    is_item_id,
};
pub use receipt::{
    Disposition, ReceiptEntry, ReceiptError, VaultExportReceipt, Writer, is_safe_vault_path,
};

/// The crate's package name, as Cargo records it.
#[must_use]
pub fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests {
    use super::{StudyCollection, VaultExportReceipt, crate_name};

    #[test]
    fn crate_is_part_of_the_openlogos_workspace() {
        assert!(crate_name().starts_with("openlogos-"));
    }

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
    fn the_schema_examples_parse_validate_and_round_trip() {
        let (Some(collection), Some(receipt)) = (
            example("study-collection.json"),
            example("vault-export-receipt.json"),
        ) else {
            return;
        };
        let collection: StudyCollection = serde_json::from_str(&collection).unwrap();
        collection.validate().unwrap();
        assert_eq!(collection.items.len(), 7);
        assert_eq!(collection.source_items().len(), 4);
        let receipt_json = receipt;
        let receipt: VaultExportReceipt = serde_json::from_str(&receipt_json).unwrap();
        receipt.validate_against(&collection).unwrap();
        assert_eq!(
            receipt.stale_items(&collection),
            vec!["item-margin".to_owned()]
        );
        assert_eq!(
            receipt.writer.core_api_version,
            openlogos_core::versioning::ApiVersion::current()
        );
        // Serializing the parsed records gives back the example files, key for key.
        assert_eq!(
            serde_json::to_value(&receipt).unwrap(),
            serde_json::from_str::<serde_json::Value>(&receipt_json).unwrap()
        );
    }
}
