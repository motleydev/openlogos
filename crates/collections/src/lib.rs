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
    use super::crate_name;

    #[test]
    fn crate_is_part_of_the_openlogos_workspace() {
        assert!(crate_name().starts_with("openlogos-"));
    }
}
