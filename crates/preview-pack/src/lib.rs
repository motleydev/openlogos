//! The B071 importer (specs/23 §4, specs/22 §7, docs/evidence/B071.md): BSB, SBLGNT and WLC from
//! the NAS artifacts into one text pack per edition and one FTS5 search pack per edition, outside
//! the repository. `just preview-pack` runs it; no corpus byte ever enters the repository.
//!
//! - [`rights`]: the rights record and hash gate, before any text is parsed.
//! - [`usfm`], [`oshb`], [`sblgnt`]: one parser per source format, each listing what it keeps.
//! - [`build`]: the edition definitions, the versification reconciliation against the canon table,
//!   the publisher's second form as an oracle (BSB), the pack and search writers.
//! - [`bench`]: text search latency against the 50 ms budget of specs/22 §7.
//! - [`morphology`]: B026, the MACULA morphology layers joined to the SBLGNT and WLC tokens.

pub mod bench;
pub mod build;
pub mod model;
pub mod morphology;
pub mod oshb;
pub mod rights;
pub mod sblgnt;
pub mod usfm;
