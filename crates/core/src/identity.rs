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
