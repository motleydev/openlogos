//! The UniFFI surface for Swift and Kotlin hosts (B061). Every type here is a binding record
//! or enum mirroring one in `crates/core`, `crates/text` or `crates/conformance`, converted
//! with `From`/`TryFrom` at the boundary so the core crates stay free of uniffi and build for
//! wasm. Function and field names are the core's; uniffi renders them in each host's
//! convention (`parseReference`, `apiVersion`). Records that a host may store carry
//! [`openlogos_core::API_VERSION`] so a persisted value names the API it came from.
//!
//! `scripts/apple-bindings.sh` and `scripts/kotlin-bindings.sh` build the library, generate
//! the bindings from its metadata (library mode, no UDL) and run the test hosts under
//! `apps/apple/CoreHost` and `apps/android/CoreHost`.

uniffi::setup_scaffolding!();

pub mod anchor;
pub mod conformance;
pub mod ink;
pub mod reference;
#[cfg(feature = "retrieval")]
pub mod retrieval;
pub mod shell;
pub mod versification;

pub use anchor::{
    Anchor, AnchorError, OffsetUnit, Paragraph, ReanchorResult, Revision, ScalarRange, Segment,
    TextQuoteSelector, TokenSpan, Utf16Range, anchor_to_utf16, reanchor, selection_to_anchors,
};
pub use conformance::{CaseResult, ConformanceReport, conformance_fixtures_json, run_conformance};
pub use ink::{
    InkDocument, InkError, InkOutlineOptions, InkPoint, InkStroke, InkTaper, InkTool,
    InkViewportTransform, ink_document_json, ink_outline,
};
pub use reference::{
    Reference, ReferenceError, ReferenceRange, ReferenceSet, ReviewFlag, format_human, format_osis,
    format_short, parse_reference,
};
pub use shell::{
    ShellColumn, ShellCommandArgs, ShellCommandError, ShellCommandInfo, ShellHandle, ShellInk,
    ShellLayoutConstants, ShellOutcome, ShellPaletteContext, ShellPaletteGroup, ShellPaletteItem,
    ShellPaletteResult, ShellPaletteSection, ShellPlatform, ShellReading, ShellRecords,
    ShellSession, ShellShortcuts, ShellTab, ShellWorkspace, layout_constants,
    migrate_workspace_json, shell_registry,
};
pub use versification::{
    MappingResult, MappingStatus, VerseMapRow, VerseMapRowKind, VersificationError, map_verse,
};

/// The core API version the bindings were generated against: [`openlogos_core::API_VERSION`].
#[uniffi::export]
#[must_use]
pub fn api_version() -> String {
    openlogos_core::api_version().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_version_is_the_core_constant() {
        assert_eq!(api_version(), openlogos_core::API_VERSION);
        assert_eq!(run_conformance().api_version, api_version());
    }
}
