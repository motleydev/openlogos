//! SQLite implementation of the retrieval boundary (ADR 0002), built and measured in B064.
//!
//! - [`tokenizer`]: the shared Greek/Hebrew normalizer (`ol-norm-1`).
//! - [`fts`]: that normalizer registered as the FTS5 tokenizer `openlogos`.
//! - [`pack`]: the read-only pack schema and its writer.
//! - [`ingest`]: Text-Fabric files into a pack, for the R6 oracle comparison.
//! - [`token_engine`]: dense feature columns, roaring postings and the slot join.
//! - [`links`]: the device-local link index merged from installed packs.
//! - [`catalog`]: the device catalog of installed packs, locator schemes and section maps
//!   (B068), which resolves cited targets and writes the packager's resolution report.
//! - [`vectors`]: sqlite-vec registration and k-nearest-neighbour queries.
//! - [`engine`]: the [`engine::Engine`] that implements the five traits over attached packs.
//! - [`edition_search`]: text search over the installed editions for the palette (B071).
//! - [`cross_references`]: the cross-reference relation pack, written and read (B057).

pub mod catalog;
pub mod cross_references;
pub mod edition_search;
pub mod engine;
pub mod fts;
pub mod ingest;
pub mod links;
pub mod pack;
pub mod token_engine;
pub mod tokenizer;
pub mod vectors;

pub use catalog::{Access, Catalog, CitationCandidate, InstalledPack};
pub use cross_references::{CrossReferencePack, write_cross_reference_pack};
pub use edition_search::EditionSearch;
pub use engine::Engine;
pub use links::{LinkIndex, UserEdge};
pub use pack::{PackBuilder, PackManifest};

/// The SQLite library version compiled into this engine (`3.54.1`).
#[must_use]
pub fn sqlite_version() -> &'static str {
    rusqlite::version()
}
